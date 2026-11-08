//! Profiles as directories under `$CCS_HOME` (ADR-2610261212; ADR-2610261643).
//!
//! A profile is `<name>/profile.env`, `<name>/kits`, and `<name>/config/`, its
//! `CLAUDE_CONFIG_DIR`. ccs puts no skills, agents or CLAUDE.md in `config/`:
//! those reach a session through kits. A dir holding the old `native` marker is
//! not a profile; `ccs migrate` retires it.

use crate::ports::{parse_profile_env, Error, LoadedProfile, Name, ProfileStore, ToolPermissions};
use serde_json::{Map, Value};
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
        // Leave nothing half-made behind, and remove nothing that was there
        // before: a folder that is not a profile can still hold a login.
        let made = !dir.exists();
        let config_made = !self.config(name).exists();
        let ours = [dir.join(ENV_FILE), dir.join(KITS_FILE), self.config(name).join(SETTINGS_FILE)];
        let fresh: Vec<bool> = ours.iter().map(|p| !p.exists()).collect();
        self.build(name, from).map_err(|e| {
            if made {
                let _ = fs::remove_dir_all(&dir);
            } else {
                for (p, _) in ours.iter().zip(&fresh).filter(|(_, f)| **f) {
                    let _ = fs::remove_file(p);
                }
                if config_made {
                    let _ = fs::remove_dir(self.config(name));
                }
            }
            io_err("cannot create", &dir)(e)
        })
    }

    fn discard(&self, name: &Name) -> Result<(), Error> {
        let path = self.dir(name).join(ENV_FILE);
        match fs::remove_file(&path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(io_err("cannot remove", &path)(e)),
            _ => {}
        }
        // remove_dir, not remove_dir_all: a folder that still holds something keeps it.
        let _ = fs::remove_dir(self.config(name));
        let _ = fs::remove_dir(self.dir(name));
        Ok(())
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

    fn read_kits(&self, name: &Name) -> Result<Vec<String>, Error> {
        let path = self.dir(name).join(KITS_FILE);
        let text = match fs::read_to_string(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            r => r.map_err(io_err("cannot read", &path))?,
        };
        Ok(text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
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

    fn permissions(&self, name: &Name) -> Result<ToolPermissions, Error> {
        let path = self.config(name).join(SETTINGS_FILE);
        let root = read_settings(&path)?;
        let Some(perms) = root.get("permissions") else { return Ok(ToolPermissions::default()) };
        let perms = perms.as_object().ok_or_else(|| Error::Io(format!("{}: \"permissions\" is not an object; fix it by hand", path.display())))?;
        let allow = match perms.get("allow") {
            None => vec![],
            Some(Value::Array(a)) => a
                .iter()
                .map(|v| v.as_str().map(String::from).ok_or_else(|| Error::Io(format!("{}: \"allow\" holds something that is not text", path.display()))))
                .collect::<Result<_, _>>()?,
            Some(_) => return Err(Error::Io(format!("{}: \"allow\" is not a list; fix it by hand", path.display()))),
        };
        Ok(ToolPermissions { mode: perms.get("defaultMode").and_then(Value::as_str).map(String::from), allow })
    }

    fn set_permissions(&self, name: &Name, p: &ToolPermissions) -> Result<(), Error> {
        let path = self.config(name).join(SETTINGS_FILE);
        // Parsed before anything is touched: a file that does not parse is never replaced.
        let mut root = read_settings(&path)?;
        let perms = root.entry("permissions").or_insert_with(|| Value::Object(Map::new()));
        let perms = perms.as_object_mut().ok_or_else(|| Error::Io(format!("{}: \"permissions\" is not an object; fix it by hand", path.display())))?;
        if let Some(mode) = &p.mode {
            perms.insert("defaultMode".into(), Value::String(mode.clone()));
        }
        perms.insert("allow".into(), Value::Array(p.allow.iter().cloned().map(Value::String).collect()));
        let mut text = serde_json::to_string_pretty(&Value::Object(root)).map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
        text.push('\n');
        if path.is_file() {
            let bak = path.with_file_name(format!("{SETTINGS_FILE}.bak-ccs"));
            fs::copy(&path, &bak).map_err(io_err("cannot back up to", &bak))?;
        }
        // Whole or not at all: write beside it, then rename over.
        let tmp = path.with_file_name(format!(".{SETTINGS_FILE}.ccs-{}", std::process::id()));
        fs::write(&tmp, text).and_then(|()| fs::rename(&tmp, &path)).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            io_err("cannot write", &path)(e)
        })
    }

    fn write_settings_if_absent(&self, name: &Name, text: &str) -> Result<bool, Error> {
        let path = self.config(name).join(SETTINGS_FILE);
        // create_new: never replace a settings.json, even one made a moment ago.
        match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            // A half-written file is ours, made a moment ago: remove it, so a retry writes it whole.
            Ok(mut f) => io::Write::write_all(&mut f, text.as_bytes()).map(|_| true).map_err(|e| {
                let _ = fs::remove_file(&path);
                io_err("cannot write", &path)(e)
            }),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
            Err(e) => Err(io_err("cannot write", &path)(e)),
        }
    }
}

/// The top-level object of a settings file; a missing file is an empty one.
/// A file that does not parse, or is not an object, is an error and is never replaced.
fn read_settings(path: &Path) -> Result<Map<String, Value>, Error> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(e) => return Err(io_err("cannot read", path)(e)),
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(m)) => Ok(m),
        Ok(_) => Err(Error::Io(format!("{}: not a JSON object; fix it by hand", path.display()))),
        Err(e) => Err(Error::Io(format!("{}: not valid JSON ({e}); fix it by hand, ccs will not overwrite it", path.display()))),
    }
}
