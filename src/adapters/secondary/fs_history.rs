//! Appends to the shell's history file (ADR-2610281500). zsh keeps
//! `: <epoch>:0;<line>`, bash a bare line; any other shell is left alone.

use crate::ports::History;
use std::io::Write;
use std::path::PathBuf;

pub enum Format {
    Zsh,
    Plain,
}

pub struct FsHistory {
    file: Option<(PathBuf, Format)>,
}

impl FsHistory {
    /// `shell` is `$SHELL`; `histfile` is `$HISTFILE` when it is exported.
    pub fn for_shell(shell: &str, home: &std::path::Path, histfile: Option<PathBuf>) -> Self {
        let name = shell.rsplit('/').next().unwrap_or("");
        let file = match name {
            "zsh" => Some((histfile.unwrap_or_else(|| home.join(".zsh_history")), Format::Zsh)),
            "bash" => Some((histfile.unwrap_or_else(|| home.join(".bash_history")), Format::Plain)),
            _ => None,
        };
        FsHistory { file }
    }
}

impl History for FsHistory {
    fn record(&self, line: &str) -> bool {
        let Some((path, format)) = &self.file else { return false };
        // One entry per call: a newline inside an argument must not start another.
        let line = line.replace(['\n', '\r'], " ");
        let entry = match format {
            Format::Zsh => {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                format!(": {now}:0;{line}\n")
            }
            Format::Plain => format!("{line}\n"),
        };
        std::fs::OpenOptions::new().create(true).append(true).open(path).and_then(|mut f| f.write_all(entry.as_bytes())).is_ok()
    }
}
