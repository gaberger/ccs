//! What earlier versions of ccs left under `$CCS_HOME`, and the stock
//! `~/.claude` (ADR-2610261643 §5, §8). Everything here reads, except
//! `retire`, which moves a path ccs made to the trash. `~/.claude` is never
//! written.

use crate::ports::{Error, ItemKind, Legacy, Name, OldItem, OldLayout, OldProfile};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SHARED: &str = "shared";
const NATIVE_MARKER: &str = "native";
const UNSHARED_FILE: &str = "unshared";

pub struct FsLegacy {
    home: PathBuf,
    claude_home: PathBuf,
    trash: PathBuf,
}

impl FsLegacy {
    /// `home` is `$CCS_HOME`; `claude_home` is the stock `~/.claude`.
    pub fn new(home: PathBuf, claude_home: PathBuf, trash: PathBuf) -> Self {
        FsLegacy { home, claude_home, trash }
    }

    fn shared(&self) -> PathBuf {
        self.home.join(SHARED)
    }

    fn points_into_shared(&self, link: &Path) -> bool {
        fs::read_link(link).is_ok_and(|t| t.starts_with(self.shared()))
    }

    fn profile(&self, name: Name, dir: &Path) -> OldProfile {
        let cfg = dir.join("config");
        let (mut own, mut links) = (Vec::new(), Vec::new());
        for e in sorted(&cfg.join("skills")) {
            let p = e.path();
            if p.is_symlink() {
                if self.points_into_shared(&p) {
                    links.push(p);
                }
            } else if p.join("SKILL.md").is_file() {
                own.push(OldItem { kind: ItemKind::Skill, name: file_name(&p), folder: String::new(), source: p });
            }
        }
        self.agents(&cfg.join("agents"), "", &mut own, &mut links);
        let md = cfg.join("CLAUDE.md");
        let claude_md = if md.is_symlink() {
            if self.points_into_shared(&md) {
                links.push(md);
            }
            None
        } else {
            md.is_file().then_some(md)
        };
        let unshared = fs::read_to_string(dir.join(UNSHARED_FILE))
            .map(|t| t.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
            .unwrap_or_default();
        OldProfile { name, dir: dir.to_path_buf(), own, claude_md, links, unshared }
    }

    /// Real agent files, by subfolder; links into shared/ (file or folder) go to `links`.
    fn agents(&self, dir: &Path, folder: &str, own: &mut Vec<OldItem>, links: &mut Vec<PathBuf>) {
        for e in sorted(dir) {
            let p = e.path();
            let file = file_name(&p);
            if p.is_symlink() {
                if self.points_into_shared(&p) {
                    links.push(p);
                }
            } else if p.is_dir() {
                let sub = if folder.is_empty() { file } else { format!("{folder}/{file}") };
                self.agents(&p, &sub, own, links);
            } else if let Some(stem) = file.strip_suffix(".md") {
                own.push(OldItem { kind: ItemKind::Agent, name: stem.into(), folder: folder.into(), source: p });
            }
        }
    }

    /// Skills (one level) and agents (any depth) in a config-dir-shaped tree.
    fn items(dir: &Path) -> Vec<OldItem> {
        let mut out: Vec<OldItem> = sorted(&dir.join("skills"))
            .into_iter()
            .map(|e| e.path())
            .filter(|p| p.join("SKILL.md").is_file())
            .map(|p| OldItem { kind: ItemKind::Skill, name: file_name(&p), folder: String::new(), source: p })
            .collect();
        fn walk(dir: &Path, folder: &str, out: &mut Vec<OldItem>) {
            for e in sorted(dir) {
                let p = e.path();
                let file = file_name(&p);
                if p.is_dir() {
                    walk(&p, &if folder.is_empty() { file } else { format!("{folder}/{file}") }, out);
                } else if let Some(stem) = file.strip_suffix(".md") {
                    out.push(OldItem { kind: ItemKind::Agent, name: stem.into(), folder: folder.into(), source: p });
                }
            }
        }
        walk(&dir.join("agents"), "", &mut out);
        out
    }
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Entries of a dir, sorted, without dotfiles.
fn sorted(dir: &Path) -> Vec<fs::DirEntry> {
    let mut v: Vec<_> = fs::read_dir(dir).into_iter().flatten().flatten().filter(|e| !e.file_name().to_string_lossy().starts_with('.')).collect();
    v.sort_by_key(|e| e.file_name());
    v
}

fn io_err(path: &Path) -> impl Fn(io::Error) -> Error + '_ {
    move |e| Error::Io(format!("{}: {e}", path.display()))
}

impl Legacy for FsLegacy {
    fn layout(&self) -> OldLayout {
        let shared = self.shared();
        let shared = shared.is_dir().then(|| {
            let md = shared.join("CLAUDE.md");
            (shared.clone(), Self::items(&shared), md.is_file().then_some(md))
        });
        let (mut profiles, mut native) = (Vec::new(), None);
        for e in sorted(&self.home) {
            let dir = e.path();
            let Ok(name) = Name::parse(&file_name(&dir)) else { continue };
            if !dir.join("profile.env").is_file() {
                continue;
            }
            if dir.join(NATIVE_MARKER).exists() {
                native = Some(dir);
            } else {
                profiles.push(self.profile(name, &dir));
            }
        }
        OldLayout { shared, profiles, native }
    }

    fn stock(&self) -> (Vec<OldItem>, Option<PathBuf>) {
        let md = self.claude_home.join("CLAUDE.md");
        (Self::items(&self.claude_home), md.is_file().then_some(md))
    }

    fn read(&self, path: &Path) -> Result<String, Error> {
        fs::read_to_string(path).map_err(io_err(path))
    }

    fn retire(&self, path: &Path) -> Result<Option<PathBuf>, Error> {
        if path.starts_with(&self.claude_home) {
            return Err(Error::Io(format!("refusing to change {}: ccs never writes ~/.claude", path.display())));
        }
        if path.is_symlink() {
            fs::remove_file(path).map_err(io_err(path))?;
            return Ok(None);
        }
        fs::create_dir_all(&self.trash).map_err(io_err(&self.trash))?;
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let rel = path.strip_prefix(&self.home).unwrap_or(path).to_string_lossy().replace('/', "_");
        let base = format!("{secs}-migrated-{rel}");
        let to = (0..)
            .map(|n| if n == 0 { base.clone() } else { format!("{base}-{n}") })
            .map(|f| self.trash.join(f))
            .find(|p| !p.exists())
            .unwrap_or_else(|| self.trash.join(&base));
        fs::rename(path, &to).map_err(io_err(path))?;
        Ok(Some(to))
    }
}
