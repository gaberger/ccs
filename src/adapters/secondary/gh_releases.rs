//! Release downloads and self-install for `ccs upgrade` (ADR-2610271000).
//!
//! `gh` downloads, because the repository is private and `gh` holds the login.
//! `tar` unpacks; `shasum -a 256` (macOS) or `sha256sum` (Linux) hashes.

use crate::ports::{Error, Releases};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct GhReleases {
    gh: String,
    repo: String,
}

impl GhReleases {
    pub fn new(gh: String, repo: String) -> Self {
        GhReleases { gh, repo }
    }

    fn gh(&self, args: &[&str]) -> Result<String, Error> {
        let out = Command::new(&self.gh).args(args).stdin(Stdio::null()).output().map_err(|_| {
            Error::Usage(format!(
                "ccs upgrade needs the GitHub CLI, because {} is private: install gh, then run: gh auth login",
                self.repo
            ))
        })?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            Err(Error::Io(format!("gh {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim())))
        }
    }
}

fn temp_dir() -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    std::env::temp_dir().join(format!("ccs-upgrade-{}-{nanos}", std::process::id()))
}

fn io(path: &Path) -> impl Fn(std::io::Error) -> Error + '_ {
    move |e| Error::Io(format!("{}: {e}", path.display()))
}

impl Releases for GhReleases {
    fn latest(&self) -> Result<String, Error> {
        self.gh(&["release", "view", "--repo", &self.repo, "--json", "tagName", "-q", ".tagName"])
    }

    fn download(&self, tag: &str, assets: &[String]) -> Result<PathBuf, Error> {
        let dir = temp_dir();
        let d = dir.to_string_lossy().into_owned();
        let mut args = vec!["release", "download", tag, "--repo", &self.repo, "-D", &d];
        for a in assets {
            args.extend(["-p", a.as_str()]);
        }
        // gh may make the folder before it fails; a failed download leaves nothing.
        self.gh(&args).inspect_err(|_| self.discard(&dir))?;
        Ok(dir)
    }

    fn read(&self, file: &Path) -> Result<String, Error> {
        fs::read_to_string(file).map_err(io(file))
    }

    fn sha256(&self, file: &Path) -> Result<String, Error> {
        let run = |cmd: &str, args: &[&str]| Command::new(cmd).args(args).arg(file).output().ok().filter(|o| o.status.success());
        let out = run("shasum", &["-a", "256"]).or_else(|| run("sha256sum", &[])).ok_or_else(|| Error::Io("neither shasum nor sha256sum ran".into()))?;
        String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .next()
            .map(str::to_lowercase)
            .ok_or_else(|| Error::Io(format!("no checksum for {}", file.display())))
    }

    fn unpack(&self, archive: &Path) -> Result<PathBuf, Error> {
        let dir = archive.parent().unwrap_or(Path::new("."));
        let ok = Command::new("tar").arg("xzf").arg(archive).arg("-C").arg(dir).arg("ccs").status().map_err(io(archive))?;
        let bin = dir.join("ccs");
        if ok.success() && bin.is_file() {
            Ok(bin)
        } else {
            Err(Error::Io(format!("{} holds no ccs binary", archive.display())))
        }
    }

    fn probe(&self, binary: &Path) -> Result<String, Error> {
        let out = Command::new(binary).arg("version").stdin(Stdio::null()).output().map_err(io(binary))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            Err(Error::Io(format!("the downloaded ccs did not run ({})", out.status)))
        }
    }

    fn install(&self, binary: &Path) -> Result<PathBuf, Error> {
        let current = std::env::current_exe().map_err(|e| Error::Io(format!("cannot find the running ccs: {e}")))?;
        let current = fs::canonicalize(&current).unwrap_or(current);
        // Beside the running binary, then renamed over it: never a half-written ccs.
        let next = current.with_file_name(format!(".ccs-upgrade-{}", std::process::id()));
        if let Err(e) = fs::copy(binary, &next) {
            let _ = fs::remove_file(&next);
            return Err(io(&next)(e));
        }
        let placed = fs::set_permissions(&next, std::os::unix::fs::PermissionsExt::from_mode(0o755)).and_then(|()| fs::rename(&next, &current));
        if let Err(e) = placed {
            let _ = fs::remove_file(&next);
            return Err(Error::Io(format!("{}: {e}", current.display())));
        }
        Ok(current)
    }

    fn discard(&self, dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }
}
