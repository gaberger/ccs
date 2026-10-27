//! Reads settings files for the status line (ADR-2610261237 §2). A file that
//! is missing or unreadable is simply absent.

use crate::ports::TextFiles;
use std::path::Path;

pub struct FsText;

impl TextFiles for FsText {
    fn read(&self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }
}
