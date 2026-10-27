//! Paths typed into the TUI: `~` expansion and completion (ADR-2610261933).

use crate::ports::PathInput;
use std::fs;
use std::path::PathBuf;

pub struct FsPathInput {
    home: PathBuf,
    cwd: PathBuf,
}

impl FsPathInput {
    pub fn new(home: PathBuf, cwd: PathBuf) -> Self {
        FsPathInput { home, cwd }
    }
}

impl PathInput for FsPathInput {
    fn expand(&self, typed: &str) -> PathBuf {
        match typed.strip_prefix('~') {
            Some("") => self.home.clone(),
            Some(rest) if rest.starts_with('/') => self.home.join(&rest[1..]),
            _ => self.cwd.join(typed),
        }
    }

    fn complete(&self, typed: &str) -> Vec<String> {
        // Split into the folder as typed and the start of the name in it.
        let (dir_typed, start) = match typed.rfind('/') {
            Some(i) => (&typed[..=i], &typed[i + 1..]),
            None if typed == "~" => return vec!["~/".into()],
            None => ("", typed),
        };
        let dir = if dir_typed.is_empty() { self.cwd.clone() } else { self.expand(dir_typed) };
        let mut out: Vec<String> = fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let file = e.file_name().to_string_lossy().into_owned();
                let shown = file.starts_with(start) && (start.starts_with('.') || !file.starts_with('.'));
                // `is_dir` follows links, so a linked folder completes as a folder.
                shown.then(|| format!("{dir_typed}{file}{}", if e.path().is_dir() { "/" } else { "" }))
            })
            .collect();
        out.sort();
        out
    }
}
