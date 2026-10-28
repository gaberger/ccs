//! The library as directories under `$CCS_HOME/library` (ADR-2610261643).
//!
//! `skills/<folders…>/<skill>/SKILL.md`, `agents/<folders…>/<agent>.md`, and
//! `plugins/<plugin>/.claude-plugin/plugin.json` (no folders).
//! A skill folder is any directory holding `SKILL.md`; every other directory is
//! a folder. Removing moves to the trash; nothing is erased.

use crate::ports::{front_matter_description, front_matter_name, manifest_field, Error, ItemKind, Library, LibraryItem, PluginContents, PluginPart};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct FsLibrary {
    root: PathBuf,
    trash: PathBuf,
}

impl FsLibrary {
    pub fn new(root: PathBuf, trash: PathBuf) -> Self {
        FsLibrary { root, trash }
    }

    fn base(&self, kind: ItemKind) -> PathBuf {
        self.root.join(kind.dir())
    }

    /// Where an item or folder path lives. An agent item is its `.md` file.
    fn locate(&self, kind: ItemKind, path: &str) -> PathBuf {
        let p = self.base(kind).join(path);
        let file = p.with_extension("md");
        if kind == ItemKind::Agent && !p.is_dir() && file.is_file() {
            file
        } else {
            p
        }
    }

    fn walk(&self, kind: ItemKind, dir: &Path, folder: &str, items: &mut Vec<LibraryItem>, folders: &mut Vec<String>) {
        let mut entries: Vec<_> = fs::read_dir(dir).into_iter().flatten().flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let file = e.file_name().to_string_lossy().into_owned();
            if file.starts_with('.') {
                continue;
            }
            let p = e.path();
            let sub = if folder.is_empty() { file.clone() } else { format!("{folder}/{file}") };
            let item = |name: String, main: &Path| {
                let text = fs::read_to_string(main).ok();
                LibraryItem {
                    kind,
                    name,
                    folder: folder.into(),
                    description: text.as_deref().and_then(front_matter_description),
                    shown_name: text.as_deref().and_then(front_matter_name),
                    source: p.clone(),
                }
            };
            match kind {
                ItemKind::Plugin => {
                    let manifest = p.join(MANIFEST);
                    if let Ok(text) = fs::read_to_string(&manifest) {
                        items.push(LibraryItem { description: manifest_field(&text, "description"), ..item(file, &manifest) });
                    }
                }
                ItemKind::Skill if p.join("SKILL.md").is_file() => items.push(item(file, &p.join("SKILL.md"))),
                ItemKind::Agent if p.is_file() => {
                    if let Some(stem) = file.strip_suffix(".md") {
                        items.push(item(stem.into(), &p));
                    }
                }
                _ if p.is_dir() => {
                    folders.push(sub.clone());
                    self.walk(kind, &p, &sub, items, folders);
                }
                _ => {}
            }
        }
    }

    fn scan(&self, kind: ItemKind) -> (Vec<LibraryItem>, Vec<String>) {
        let (mut items, mut folders) = (Vec::new(), Vec::new());
        self.walk(kind, &self.base(kind), "", &mut items, &mut folders);
        items.sort_by(|a, b| (&a.folder, &a.name).cmp(&(&b.folder, &b.name)));
        folders.sort();
        (items, folders)
    }
}

/// Where a plugin keeps its manifest.
const MANIFEST: &str = ".claude-plugin/plugin.json";

fn io_err(path: &Path) -> impl Fn(io::Error) -> Error + '_ {
    move |e| Error::Io(format!("{}: {e}", path.display()))
}

fn copy_dir(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for e in fs::read_dir(src)? {
        let e = e?;
        let to = dst.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &to)?;
        } else {
            fs::copy(e.path(), to)?;
        }
    }
    Ok(())
}

impl Library for FsLibrary {
    fn items(&self, kind: ItemKind) -> Vec<LibraryItem> {
        self.scan(kind).0
    }

    fn folders(&self, kind: ItemKind) -> Vec<String> {
        self.scan(kind).1
    }

    fn add(&self, kind: ItemKind, folder: &str, name: &str, source: &Path) -> Result<(), Error> {
        let dir = self.base(kind).join(folder);
        let dst = if kind == ItemKind::Agent { dir.join(format!("{name}.md")) } else { dir.join(name) };
        if dst.exists() || dst.is_symlink() {
            return Err(Error::AssetExists(dst.display().to_string()));
        }
        fs::create_dir_all(&dir).map_err(io_err(&dir))?;
        match kind {
            ItemKind::Agent => fs::copy(source, &dst).map(|_| ()),
            _ => copy_dir(source, &dst),
        }
        .map_err(io_err(&dst))
    }

    fn classify(&self, source: &Path) -> Result<(ItemKind, PathBuf), Error> {
        if source.join(MANIFEST).is_file() {
            Ok((ItemKind::Plugin, source.to_path_buf()))
        } else if source.join("SKILL.md").is_file() {
            Ok((ItemKind::Skill, source.to_path_buf()))
        } else if source.file_name().is_some_and(|f| f == "SKILL.md") && source.is_file() {
            Ok((ItemKind::Skill, source.parent().unwrap_or(source).to_path_buf()))
        } else if source.extension().is_some_and(|e| e == "md") && source.is_file() {
            Ok((ItemKind::Agent, source.to_path_buf()))
        } else {
            Err(Error::Usage(format!(
                "{} is not a skill folder (SKILL.md), an agent .md file, or a plugin ({MANIFEST})",
                source.display()
            )))
        }
    }

    fn plugin_name(&self, dir: &Path) -> Result<String, Error> {
        let p = dir.join(MANIFEST);
        let text = fs::read_to_string(&p).map_err(|_| Error::Usage(format!("{} has no {MANIFEST}: it is not a plugin", dir.display())))?;
        manifest_field(&text, "name").ok_or_else(|| Error::Usage(format!("{} has no name", p.display())))
    }

    fn main_file(&self, item: &LibraryItem) -> PathBuf {
        match item.kind {
            ItemKind::Skill => item.source.join("SKILL.md"),
            ItemKind::Agent => item.source.clone(),
            ItemKind::Plugin => item.source.join(MANIFEST),
        }
    }

    fn read(&self, item: &LibraryItem) -> Result<String, Error> {
        let p = self.main_file(item);
        fs::read_to_string(&p).map_err(io_err(&p))
    }

    fn plugin_contents(&self, plugin: &LibraryItem) -> PluginContents {
        if plugin.kind != ItemKind::Plugin {
            return PluginContents::default();
        }
        let part = |file: String, main: &Path| {
            let text = fs::read_to_string(main).ok();
            PluginPart { file, shown_name: text.as_deref().and_then(front_matter_name), description: text.as_deref().and_then(front_matter_description) }
        };
        let listed = |dir: &str| {
            let mut entries: Vec<_> = fs::read_dir(plugin.source.join(dir)).into_iter().flatten().flatten().collect();
            entries.sort_by_key(|e| e.file_name());
            entries
        };
        let skills = listed("skills")
            .into_iter()
            .filter(|e| e.path().join("SKILL.md").is_file())
            .map(|e| part(e.file_name().to_string_lossy().into_owned(), &e.path().join("SKILL.md")))
            .collect();
        let agents = listed("agents")
            .into_iter()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let stem = name.strip_suffix(".md")?.to_string();
                e.path().is_file().then(|| part(stem, &e.path()))
            })
            .collect();
        PluginContents { skills, agents }
    }

    fn make_folder(&self, kind: ItemKind, folder: &str) -> Result<(), Error> {
        let dir = self.base(kind).join(folder);
        fs::create_dir_all(&dir).map_err(io_err(&dir))
    }

    fn move_to(&self, kind: ItemKind, from: &str, to: &str) -> Result<(), Error> {
        let src = self.locate(kind, from);
        if !src.exists() {
            return Err(Error::NoAsset(format!("{from} in the library")));
        }
        let dir = self.base(kind).join(to);
        let dst = dir.join(src.file_name().unwrap_or_default());
        if dst.exists() {
            return Err(Error::AssetExists(dst.display().to_string()));
        }
        fs::create_dir_all(&dir).map_err(io_err(&dir))?;
        fs::rename(&src, &dst).map_err(io_err(&src))
    }

    fn rename_folder(&self, kind: ItemKind, folder: &str, name: &str) -> Result<(), Error> {
        let src = self.base(kind).join(folder);
        if !src.is_dir() || src.join("SKILL.md").exists() {
            return Err(Error::NoAsset(format!("folder {folder} in the library")));
        }
        let dst = src.with_file_name(name);
        if dst.exists() {
            return Err(Error::AssetExists(dst.display().to_string()));
        }
        fs::rename(&src, &dst).map_err(io_err(&src))
    }

    fn trash(&self, kind: ItemKind, path: &str) -> Result<PathBuf, Error> {
        let src = self.locate(kind, path);
        if !src.exists() {
            return Err(Error::NoAsset(format!("{path} in the library")));
        }
        fs::create_dir_all(&self.trash).map_err(io_err(&self.trash))?;
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let base = format!("{secs}-library-{}-{}", kind.label(), path.replace('/', "_"));
        let to = (0..)
            .map(|n| if n == 0 { base.clone() } else { format!("{base}-{n}") })
            .map(|f| self.trash.join(f))
            .find(|p| !p.exists())
            .unwrap_or_else(|| self.trash.join(&base));
        fs::rename(&src, &to).map_err(io_err(&src))?;
        Ok(to)
    }
}
