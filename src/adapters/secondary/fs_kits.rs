//! Kits as directories under `$CCS_HOME/kits`, built into `$CCS_HOME/.build`
//! (ADR-2610261643).
//!
//! A built kit is a Claude Code plugin. Skills are links to their library
//! folder. Agents are copies, because Claude Code does not read an agent
//! through a link (measured; see the ADR). The build is rebuilt from scratch on
//! every launch, so it never holds anything the kit no longer picks.

use crate::ports::{Entry, Error, ItemKind, Kit, KitStore, LibraryItem};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const USES_FILE: &str = "uses";
const INSTRUCTIONS_FILE: &str = "CLAUDE.md";
const MCP_FILE: &str = ".mcp.json";

pub struct FsKitStore {
    kits: PathBuf,
    build: PathBuf,
    trash: PathBuf,
}

impl FsKitStore {
    pub fn new(kits: PathBuf, build: PathBuf, trash: PathBuf) -> Self {
        FsKitStore { kits, build, trash }
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
        for i in items {
            match i.kind {
                ItemKind::Skill => {
                    fs::create_dir_all(dir.join("skills"))?;
                    std::os::unix::fs::symlink(&i.source, dir.join("skills").join(&i.name))?;
                }
                _ => {
                    fs::create_dir_all(dir.join("agents"))?;
                    fs::copy(&i.source, dir.join("agents").join(format!("{}.md", i.name)))?;
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
        let to = self.trash.join(format!("{secs}-kit-{}", kit.as_str()));
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
        let path = self.dir(kit).join(USES_FILE);
        let text = fs::read_to_string(&path).unwrap_or_default();
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| Entry::parse(l).map_err(|e| Error::Io(format!("{}: {e}", path.display()))))
            .collect()
    }

    fn set_uses(&self, kit: &Kit, uses: &[Entry]) -> Result<(), Error> {
        let path = self.dir(kit).join(USES_FILE);
        let text: String = uses.iter().map(|u| format!("{}\n", u.line())).collect();
        fs::write(&path, text).map_err(io_err(&path))
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

    fn build(&self, kit: &Kit, items: &[LibraryItem]) -> Result<PathBuf, Error> {
        let dir = self.build.join(kit.as_str());
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(io_err(&dir))?;
        }
        self.assemble(kit, &dir, items).map_err(io_err(&dir))?;
        Ok(dir)
    }

    fn write_prompt(&self, kits: &[Kit], text: &str) -> Result<PathBuf, Error> {
        let names: Vec<&str> = kits.iter().map(Kit::as_str).collect();
        let path = self.build.join(format!("{}.md", names.join("+")));
        fs::create_dir_all(&self.build).map_err(io_err(&self.build))?;
        fs::write(&path, text).map_err(io_err(&path))?;
        Ok(path)
    }
}
