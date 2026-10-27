//! Opens a file in `$VISUAL` / `$EDITOR` and waits (ADR-2610261643).
//!
//! The command is split on whitespace and run directly, never through a shell,
//! so `code --wait` works and nothing in it is evaluated.

use crate::ports::{Editor, Error};
use std::path::Path;
use std::process::Command;

pub struct ProcessEditor {
    command: Vec<String>,
}

impl ProcessEditor {
    /// `command` is the editor and its arguments; the file goes last.
    pub fn new(command: &str) -> Self {
        ProcessEditor { command: command.split_whitespace().map(String::from).collect() }
    }
}

impl Editor for ProcessEditor {
    fn edit(&self, path: &Path) -> Result<(), Error> {
        let (program, args) = self.command.split_first().ok_or_else(|| Error::Usage("no editor: set VISUAL or EDITOR".into()))?;
        // A linked item is edited at its source: an editor that saves by renaming
        // a new file into place would otherwise replace the link with a copy.
        let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let shown = self.command.join(" ");
        match Command::new(program).args(args).arg(&target).status() {
            Ok(s) if s.success() => Ok(()),
            Ok(s) => Err(Error::Io(format!("editor '{shown}' exited with {s}; {} may be unsaved", target.display()))),
            Err(e) => Err(Error::Io(format!("cannot start editor '{shown}': {e}"))),
        }
    }
}
