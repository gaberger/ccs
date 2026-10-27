//! Profiles as directories under `$CCS_HOME` (ADR-2610261212; ADR-2610261643).
//!
//! A profile is `<name>/profile.env`, `<name>/kits`, and `<name>/config/`, its
//! `CLAUDE_CONFIG_DIR`. ccs puts no skills, agents or CLAUDE.md in `config/`:
//! those reach a session through kits. A dir holding the old `native` marker is
//! not a profile; `ccs migrate` retires it.

use crate::ports::{parse_profile_env, Error, LoadedProfile, Name, ProfileStore};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const ENV_FILE: &str = "profile.env";
const KITS_FILE: &str = "kits";
const SETTINGS_FILE: &str = "settings.json";
const OLD_NATIVE_MARKER: &str = "native";
const DEFAULT_FILE: &str = ".default";

const ENV_TEMPLATE: &str = "\
# Variables for this profile, one KEY=VALUE per line. Parsed, never run by a shell.
# No secrets here: AWS credentials come from ~/.aws or SSO; a claude.ai login
# comes from /login inside the profile.
#
# Bedrock example:
# CLAUDE_CODE_USE_BEDROCK=1
# AWS_PROFILE=my-sso-profile
# AWS_REGION=us-east-1
# ANTHROPIC_MODEL=us.anthropic.claude-opus-5-5-v1:0
";

pub struct FsProfileStore {
    home: PathBuf,
}

impl FsProfileStore {
    /// `home` is `$CCS_HOME`.
    pub fn new(home: PathBuf) -> Self {
        FsProfileStore { home }
    }

    fn dir(&self, name: &Name) -> PathBuf {
        self.home.join(name.as_str())
    }

    fn config(&self, name: &Name) -> PathBuf {
        self.dir(name).join("config")
    }

    fn build(&self, name: &Name, from: Option<&Name>) -> io::Result<()> {
        fs::create_dir_all(self.config(name))?;
        let Some(src) = from else {
            return fs::write(self.dir(name).join(ENV_FILE), ENV_TEMPLATE);
        };
        fs::copy(self.dir(src).join(ENV_FILE), self.dir(name).join(ENV_FILE))?;
        // Variables, kits and settings are chosen; `.claude.json` and the rest of
        // `config/` are login state and history, and are never copied.
        for (from, to) in [(self.dir(src).join(KITS_FILE), self.dir(name).join(KITS_FILE)), (self.config(src).join(SETTINGS_FILE), self.config(name).join(SETTINGS_FILE))] {
            if from.is_file() {
                fs::copy(from, to)?;
            }
        }
        Ok(())
    }
}

fn io_err<'a>(what: &str, path: &'a Path) -> impl Fn(io::Error) -> Error + 'a {
    let what = what.to_string();
    move |e| Error::Io(format!("{what} {}: {e}", path.display()))
}

impl ProfileStore for FsProfileStore {
    fn folder(&self, name: &Name) -> PathBuf {
        self.dir(name)
    }

    fn env_file(&self, name: &Name) -> PathBuf {
        self.dir(name).join(ENV_FILE)
    }

    fn names(&self) -> Vec<Name> {
        let Ok(entries) = fs::read_dir(&self.home) else { return vec![] };
        let mut out: Vec<Name> = entries
            .flatten()
            .filter_map(|e| Name::parse(&e.file_name().to_string_lossy()).ok())
            .filter(|n| self.exists(n))
            .collect();
        out.sort();
        out
    }

    fn exists(&self, name: &Name) -> bool {
        self.dir(name).join(ENV_FILE).is_file() && !self.dir(name).join(OLD_NATIVE_MARKER).exists()
    }

    fn load(&self, name: &Name) -> Result<LoadedProfile, Error> {
        if !self.exists(name) {
            return Err(Error::NoProfile(name.as_str().into()));
        }
        let path = self.dir(name).join(ENV_FILE);
        let text = fs::read_to_string(&path).map_err(io_err("cannot read", &path))?;
        Ok(LoadedProfile { name: name.clone(), env: parse_profile_env(&text)?, config_dir: self.config(name) })
    }

    fn create(&self, name: &Name, from: Option<&Name>) -> Result<(), Error> {
        let dir = self.dir(name);
        self.build(name, from).map_err(|e| {
            // Leave nothing half-made behind.
            let _ = fs::remove_dir_all(&dir);
            io_err("cannot create", &dir)(e)
        })
    }

    fn default_name(&self) -> Option<String> {
        let s = fs::read_to_string(self.home.join(DEFAULT_FILE)).ok()?;
        let s = s.trim();
        (!s.is_empty()).then(|| s.to_string())
    }

    fn set_default(&self, name: &Name) -> Result<(), Error> {
        let path = self.home.join(DEFAULT_FILE);
        fs::write(&path, format!("{}\n", name.as_str())).map_err(io_err("cannot write", &path))
    }

    fn clear_default(&self) -> Result<(), Error> {
        let path = self.home.join(DEFAULT_FILE);
        match fs::remove_file(&path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(io_err("cannot remove", &path)(e)),
            _ => Ok(()),
        }
    }

    fn kits(&self, name: &Name) -> Vec<String> {
        let text = fs::read_to_string(self.dir(name).join(KITS_FILE)).unwrap_or_default();
        text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()
    }

    fn set_kits(&self, name: &Name, kits: &[String]) -> Result<(), Error> {
        let path = self.dir(name).join(KITS_FILE);
        let text: String = kits.iter().map(|k| format!("{k}\n")).collect();
        fs::write(&path, text).map_err(io_err("cannot write", &path))
    }

    fn write_env(&self, name: &Name, text: &str) -> Result<(), Error> {
        let path = self.dir(name).join(ENV_FILE);
        fs::write(&path, text).map_err(io_err("cannot write", &path))
    }

    fn write_settings_if_absent(&self, name: &Name, text: &str) -> Result<bool, Error> {
        let path = self.config(name).join(SETTINGS_FILE);
        // create_new: never replace a settings.json, even one made a moment ago.
        match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => io::Write::write_all(&mut f, text.as_bytes()).map(|_| true).map_err(io_err("cannot write", &path)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
            Err(e) => Err(io_err("cannot write", &path)(e)),
        }
    }
}
