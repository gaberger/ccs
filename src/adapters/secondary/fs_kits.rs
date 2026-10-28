//! Kits as directories under `$CCS_HOME/kits`, built into `$CCS_HOME/.build`
//! (ADR-2610261643).
//!
//! A built kit is a Claude Code plugin. Skills are links to their library
//! folder. Agents are copies, because Claude Code does not read an agent
//! through a link (measured; see the ADR). The build is rebuilt from scratch on
//! every launch, so it never holds anything the kit no longer picks.

use crate::ports::{Entry, Error, ItemKind, Kit, KitStore, LibraryItem};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const USES_FILE: &str = "uses";
const INSTRUCTIONS_FILE: &str = "CLAUDE.md";
const MCP_FILE: &str = ".mcp.json";

/// A name no other write in this or any process uses. The clock alone can
/// repeat (macOS counts in microseconds), so a counter keeps two writes in one
/// process out of each other's file.
fn staging_name(stem: &str) -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    format!(".{stem}.tmp-{}-{nanos}-{seq}", std::process::id())
}

pub struct FsKitStore {
    kits: PathBuf,
    build: PathBuf,
    trash: PathBuf,
    /// Each kit's `uses` as this process last read or wrote it: the base a
    /// save is measured against, so it changes only what this process changed.
    seen: Mutex<HashMap<String, Vec<Entry>>>,
}

impl FsKitStore {
    pub fn new(kits: PathBuf, build: PathBuf, trash: PathBuf) -> Self {
        FsKitStore { kits, build, trash, seen: Mutex::default() }
    }

    fn read_uses(&self, kit: &Kit) -> Result<(String, Vec<Entry>), Error> {
        let path = self.dir(kit).join(USES_FILE);
        // Only a missing file is an empty kit. Any other error read as empty
        // would let `set_uses` replace the whole file, comments and all.
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(io_err(&path)(e)),
        };
        let entries = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| Entry::parse(l).map_err(|e| Error::Io(format!("{}: {e}", path.display()))))
            .collect::<Result<_, _>>()?;
        Ok((text, entries))
    }

    fn remember(&self, kit: &Kit, entries: &[Entry]) {
        if let Ok(mut seen) = self.seen.lock() {
            seen.insert(kit.as_str().to_string(), entries.to_vec());
        }
    }

    fn dir(&self, kit: &Kit) -> PathBuf {
        self.kits.join(kit.as_str())
    }

    fn assemble(&self, kit: &Kit, dir: &Path, items: &[LibraryItem]) -> io::Result<()> {
        fs::create_dir_all(dir.join(".claude-plugin"))?;
        let manifest = format!(
            "{{\n  \"name\": \"{}\",\n  \"version\": \"0.0.0\",\n  \"description\": \"ccs kit {}; generated, edit the library instead\"\n}}\n",
            kit.as_str(),
            kit.as_str()
        );
        fs::write(dir.join(".claude-plugin/plugin.json"), manifest)?;
        // Two items of one name in different folders land on one path: a second
        // link fails with a bare "File exists", and a second copy silently
        // replaces the first. The plan keeps only the first of a name, so this
        // is a guard: refuse, and name both sources.
        let mut placed: HashMap<PathBuf, &Path> = HashMap::new();
        for i in items {
            let target = match i.kind {
                ItemKind::Skill => dir.join("skills").join(&i.name),
                _ => dir.join("agents").join(format!("{}.md", i.name)),
            };
            if let Some(first) = placed.insert(target.clone(), &i.source) {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("two {}s named {}: {} and {}", i.kind.label(), i.name, first.display(), i.source.display()),
                ));
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            match i.kind {
                ItemKind::Skill => std::os::unix::fs::symlink(&i.source, &target)?,
                _ => {
                    fs::copy(&i.source, &target)?;
                }
            }
        }
        let mcp = self.dir(kit).join(MCP_FILE);
        if mcp.is_file() {
            fs::copy(mcp, dir.join(MCP_FILE))?;
        }
        Ok(())
    }
}

/// Swap two folders in one step, so a reader of either path sees one whole
/// folder or the other, never a gap.
#[cfg(any(target_os = "macos", all(target_os = "linux", target_env = "gnu")))]
fn exchange(a: &Path, b: &Path) -> io::Result<()> {
    use std::ffi::{c_char, c_int, c_uint, CString};
    use std::os::unix::ffi::OsStrExt;
    let cstr = |p: &Path| CString::new(p.as_os_str().as_bytes()).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e));
    let (a, b) = (cstr(a)?, cstr(b)?);
    #[cfg(target_os = "macos")]
    let rc = {
        extern "C" {
            fn renamex_np(from: *const c_char, to: *const c_char, flags: c_uint) -> c_int;
        }
        const RENAME_SWAP: c_uint = 0x2;
        unsafe { renamex_np(a.as_ptr(), b.as_ptr(), RENAME_SWAP) }
    };
    #[cfg(target_os = "linux")]
    let rc = {
        extern "C" {
            fn renameat2(olddirfd: c_int, old: *const c_char, newdirfd: c_int, new: *const c_char, flags: c_uint) -> c_int;
        }
        const AT_FDCWD: c_int = -100;
        const RENAME_EXCHANGE: c_uint = 0x2;
        unsafe { renameat2(AT_FDCWD, a.as_ptr(), AT_FDCWD, b.as_ptr(), RENAME_EXCHANGE) }
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// No one-step swap here: move the old folder aside, then the new one in. The
/// gap is two renames, not a whole rebuild.
#[cfg(not(any(target_os = "macos", all(target_os = "linux", target_env = "gnu"))))]
fn exchange(a: &Path, b: &Path) -> io::Result<()> {
    let mut aside = a.as_os_str().to_owned();
    aside.push(".old");
    let aside = PathBuf::from(aside);
    fs::rename(b, &aside)?;
    fs::rename(a, b)?;
    fs::rename(&aside, a)
}

fn io_err(path: &Path) -> impl Fn(io::Error) -> Error + '_ {
    move |e| Error::Io(format!("{}: {e}", path.display()))
}

impl KitStore for FsKitStore {
    fn names(&self) -> Vec<Kit> {
        let mut out: Vec<Kit> = fs::read_dir(&self.kits)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| Kit::parse(&e.file_name().to_string_lossy()).ok())
            .collect();
        out.sort();
        out
    }

    fn exists(&self, kit: &Kit) -> bool {
        self.dir(kit).is_dir()
    }

    fn create(&self, kit: &Kit) -> Result<(), Error> {
        let dir = self.dir(kit);
        fs::create_dir_all(&dir).map_err(io_err(&dir))?;
        let uses = dir.join(USES_FILE);
        fs::write(&uses, "").map_err(io_err(&uses))
    }

    fn remove(&self, kit: &Kit) -> Result<PathBuf, Error> {
        let src = self.dir(kit);
        fs::create_dir_all(&self.trash).map_err(io_err(&self.trash))?;
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let base = format!("{secs}-kit-{}", kit.as_str());
        let to = (0..)
            .map(|n| if n == 0 { base.clone() } else { format!("{base}-{n}") })
            .map(|f| self.trash.join(f))
            .find(|p| !p.exists())
            .unwrap_or_else(|| self.trash.join(&base));
        fs::rename(&src, &to).map_err(io_err(&src))?;
        let _ = fs::remove_dir_all(self.build.join(kit.as_str()));
        Ok(to)
    }

    fn rename(&self, kit: &Kit, to: &Kit) -> Result<(), Error> {
        let (src, dst) = (self.dir(kit), self.dir(to));
        if dst.exists() {
            return Err(Error::Usage(format!("kit '{}' already exists", to.as_str())));
        }
        fs::rename(&src, &dst).map_err(io_err(&src))?;
        // The old build is named after the old kit; the next launch builds the new one.
        let _ = fs::remove_dir_all(self.build.join(kit.as_str()));
        Ok(())
    }

    fn uses(&self, kit: &Kit) -> Result<Vec<Entry>, Error> {
        let (_, entries) = self.read_uses(kit)?;
        self.remember(kit, &entries);
        Ok(entries)
    }

    /// ADR-2610281305 R3: comments and blank lines stay where they are; a line
    /// still picked stays where it is; a line that changed (a renamed folder)
    /// takes the place of the one it replaced; a new pick goes at the end. The
    /// file is replaced whole, through a temporary file beside it, or not at all.
    ///
    /// Two processes may save one kit at once (a TUI and a CLI command). Each
    /// save holds a lock on the kit, re-reads the file, and applies only the
    /// picks it added or dropped since its own last read, so neither save
    /// throws away the other's.
    fn set_uses(&self, kit: &Kit, uses: &[Entry]) -> Result<(), Error> {
        enum Slot {
            Keep(String),
            Entry(Option<usize>),
        }
        let path = self.dir(kit).join(USES_FILE);
        // The lock is on the kit's folder, so nothing is left beside `uses`.
        let dir = self.dir(kit);
        let lock = fs::File::open(&dir).map_err(io_err(&dir))?;
        lock.lock().map_err(io_err(&dir))?;
        let (old, current) = self.read_uses(kit)?;
        let base = self.seen.lock().ok().and_then(|s| s.get(kit.as_str()).cloned()).unwrap_or_else(|| current.clone());
        // Keep this save's order; drop what another save dropped; keep what another save added.
        let mut merged: Vec<Entry> = uses.iter().filter(|u| current.contains(u) || !base.contains(u)).cloned().collect();
        merged.extend(current.iter().filter(|c| !base.contains(c) && !uses.contains(c)).cloned());
        let uses = &merged[..];
        let mut taken = vec![false; uses.len()];
        let mut slots = Vec::new();
        for l in old.lines() {
            let t = l.trim();
            if t.is_empty() || t.starts_with('#') {
                slots.push(Slot::Keep(l.to_string()));
            } else {
                let i = (0..uses.len()).find(|&i| !taken[i] && uses[i].line() == t);
                if let Some(i) = i {
                    taken[i] = true;
                }
                slots.push(Slot::Entry(i));
            }
        }
        // A rename keeps its place in `uses`, so it comes before the next line
        // still picked; a new pick comes after every line still picked.
        let mut next_kept = vec![uses.len(); slots.len()];
        let mut bound = uses.len();
        for (n, s) in slots.iter().enumerate().rev() {
            next_kept[n] = bound;
            if let Slot::Entry(Some(i)) = s {
                bound = *i;
            }
        }
        let mut rest: std::collections::VecDeque<usize> = (0..uses.len()).filter(|&i| !taken[i]).collect();
        let mut lines = Vec::new();
        for (n, s) in slots.into_iter().enumerate() {
            match s {
                Slot::Keep(l) => lines.push(l),
                Slot::Entry(Some(i)) => lines.push(uses[i].line()),
                Slot::Entry(None) => {
                    if rest.front().is_some_and(|&i| i < next_kept[n]) {
                        lines.extend(rest.pop_front().map(|i| uses[i].line()));
                    }
                }
            }
        }
        lines.extend(rest.into_iter().map(|i| uses[i].line()));
        let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
        let tmp = self.dir(kit).join(format!("{USES_FILE}.tmp-{}", std::process::id()));
        if let Err(e) = fs::write(&tmp, text).and_then(|()| fs::rename(&tmp, &path)) {
            let _ = fs::remove_file(&tmp);
            return Err(io_err(&path)(e));
        }
        self.remember(kit, uses);
        Ok(())
    }

    fn instructions(&self, kit: &Kit) -> Option<String> {
        fs::read_to_string(self.dir(kit).join(INSTRUCTIONS_FILE)).ok().filter(|t| !t.trim().is_empty())
    }

    fn set_instructions(&self, kit: &Kit, text: &str) -> Result<(), Error> {
        let path = self.instructions_file(kit);
        fs::write(&path, text).map_err(io_err(&path))
    }

    fn instructions_file(&self, kit: &Kit) -> PathBuf {
        self.dir(kit).join(INSTRUCTIONS_FILE)
    }

    /// A session already running on this kit reads `dir` for its whole life, so
    /// the new build is assembled in a fresh folder beside it and then takes
    /// `dir`'s place in one step. `dir` is never empty or half built.
    fn build(&self, kit: &Kit, items: &[LibraryItem]) -> Result<PathBuf, Error> {
        let dir = self.build_dir(kit);
        let fresh = self.build.join(staging_name(kit.as_str()));
        let placed = self.assemble(kit, &fresh, items).and_then(|()| {
            // Onto an empty or absent `dir` a rename is enough; onto a build it fails.
            fs::rename(&fresh, &dir).or_else(|_| exchange(&fresh, &dir))
        });
        // After an exchange `fresh` holds the old build; after a failure, the new one.
        let _ = fs::remove_dir_all(&fresh);
        placed.map_err(io_err(&dir))?;
        Ok(dir)
    }

    /// Every launch of these kits shares one prompt file, and a session already
    /// started may still be reading it. So the text is written beside it and
    /// then renamed over it: a reader sees the old file whole or the new one.
    fn write_prompt(&self, kits: &[Kit], text: &str) -> Result<PathBuf, Error> {
        let path = self.prompt_path(kits);
        fs::create_dir_all(&self.build).map_err(io_err(&self.build))?;
        let stem = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let tmp = self.build.join(staging_name(&stem));
        if let Err(e) = fs::write(&tmp, text).and_then(|()| fs::rename(&tmp, &path)) {
            let _ = fs::remove_file(&tmp);
            return Err(io_err(&path)(e));
        }
        Ok(path)
    }

    fn build_dir(&self, kit: &Kit) -> PathBuf {
        self.build.join(kit.as_str())
    }

    fn prompt_path(&self, kits: &[Kit]) -> PathBuf {
        let names: Vec<&str> = kits.iter().map(Kit::as_str).collect();
        self.build.join(format!("{}.md", names.join("+")))
    }
}
