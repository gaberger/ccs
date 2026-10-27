//! Finds a `.claude-profile` pin file in a directory or any parent.

use crate::ports::PinLocator;
use std::fs;
use std::path::PathBuf;

const PIN_FILE: &str = ".claude-profile";

pub struct FsPinLocator {
    start: PathBuf,
}

impl FsPinLocator {
    pub fn new(start: PathBuf) -> Self {
        FsPinLocator { start }
    }
}

impl PinLocator for FsPinLocator {
    fn pinned(&self) -> Option<String> {
        self.start.ancestors().find_map(|d| {
            let s = fs::read_to_string(d.join(PIN_FILE)).ok()?;
            let s = s.trim();
            (!s.is_empty()).then(|| s.to_string())
        })
    }
}
