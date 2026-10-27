//! Fetches plugins with the `git` command (ADR-2610261643 §15).
//!
//! A clone goes to a fresh temporary folder; the library copies it in, `.git`
//! and all, so `pull` can update it in place later.

use crate::ports::{Error, Fetcher};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct GitFetcher {
    program: String,
}

impl GitFetcher {
    pub fn new(program: String) -> Self {
        GitFetcher { program }
    }

    fn git(&self, dir: Option<&Path>, args: &[&str]) -> Result<String, Error> {
        let mut cmd = Command::new(&self.program);
        if let Some(d) = dir {
            cmd.arg("-C").arg(d);
        }
        let out = cmd
            .args(args)
            .stdin(Stdio::null())
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .map_err(|e| Error::Io(format!("cannot run {}: {e}", self.program)))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            Err(Error::Io(format!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim())))
        }
    }
}

impl Fetcher for GitFetcher {
    fn clone_temp(&self, url: &str) -> Result<PathBuf, Error> {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("ccs-clone-{}-{nanos}", std::process::id()));
        self.git(None, &["clone", "--quiet", "--depth", "1", url, &dir.to_string_lossy()])?;
        Ok(dir)
    }

    fn discard(&self, dir: &Path) {
        let _ = std::fs::remove_dir_all(dir);
    }

    fn pull(&self, dir: &Path) -> Result<Option<String>, Error> {
        if !dir.join(".git").exists() {
            return Ok(None);
        }
        let before = self.git(Some(dir), &["rev-parse", "--short", "HEAD"])?;
        self.git(Some(dir), &["pull", "--quiet", "--ff-only"])?;
        let after = self.git(Some(dir), &["rev-parse", "--short", "HEAD"])?;
        Ok(Some(if before == after { format!("up to date at {after}") } else { format!("updated {before}..{after}") }))
    }
}
