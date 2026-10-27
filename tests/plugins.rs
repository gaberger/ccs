//! Gate for ADR-2610261643 §15: plugins in the library, picked by kits, loaded
//! with `--plugin-dir`. The pure rules are tested directly; the rest runs the
//! real `ccs` binary, with local git repositories standing in for GitHub.

use ccs::domain::{git_source, manifest_field, Kind, Use};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as Proc, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

// ── Pure rules ───────────────────────────────────────────────────────────────

#[test]
fn a_plugin_entry_is_an_item_and_has_no_folders() {
    assert_eq!(Use::parse("plugins/egm").unwrap(), Use::Item(Kind::Plugin, "egm".into()));
    assert_eq!(Use::parse("plugins/egm").unwrap().line(), "plugins/egm");
    for bad in ["plugins/", "plugins/net/", "plugins/a/b", "plugins/.x"] {
        assert!(Use::parse(bad).is_err(), "{bad:?} must be refused");
    }
}

#[test]
fn the_manifest_name_is_the_top_level_one() {
    let json = r#"{ "author": { "name": "Google LLC" }, "name": "egm", "description": "Say \"hi\"", "version": "0.1.0" }"#;
    assert_eq!(manifest_field(json, "name").as_deref(), Some("egm"), "not the author's name");
    assert_eq!(manifest_field(json, "description").as_deref(), Some("Say \"hi\""));
    assert_eq!(manifest_field(json, "version").as_deref(), Some("0.1.0"));
    assert_eq!(manifest_field(json, "homepage"), None);
    assert_eq!(manifest_field(r#"{"agents": ["name"], "name": "x"}"#, "name").as_deref(), Some("x"), "a string in a list is not a key");
    assert_eq!(manifest_field("not json", "name"), None);
    let nested = r#"{"repository": {"url": "x"}, "name": "egm"}"#;
    assert_eq!(manifest_field(nested, "repository"), None, "an object is not a string field");
    assert_eq!(manifest_field(nested, "name").as_deref(), Some("egm"));
}

#[test]
fn a_source_reads_as_git_only_when_it_looks_like_one() {
    assert_eq!(git_source("google/elephant-goldfish-model").as_deref(), Some("https://github.com/google/elephant-goldfish-model.git"));
    for url in ["https://example.com/a/b.git", "https://github.com/a/b", "git@github.com:a/b.git", "file:///srv/mirror/egm", "/srv/x/egm.git"] {
        assert_eq!(git_source(url).as_deref(), Some(url), "{url}");
    }
    for not in ["egm", "./a/b", "../a/b", "~/a/b", "/abs/path", "a/b/c", "a b/c", ".x/y"] {
        assert_eq!(git_source(not), None, "{not}");
    }
}

// ── End to end ───────────────────────────────────────────────────────────────

static N: AtomicUsize = AtomicUsize::new(0);

struct Env {
    root: PathBuf,
}

impl Env {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("ccs-plugins-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("home/.claude")).unwrap();
        fs::create_dir_all(root.join("cwd")).unwrap();
        let claude = root.join("claude");
        fs::write(&claude, "#!/bin/sh\nfor a; do echo \"ARG $a\"; done\nenv\n").unwrap();
        fs::set_permissions(&claude, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        Env { root }
    }
    fn home(&self) -> PathBuf {
        self.root.join("profiles")
    }
    fn run(&self, a: &[&str]) -> Output {
        let mut c = Proc::new(env!("CARGO_BIN_EXE_ccs"));
        c.args(a)
            .current_dir(self.root.join("cwd"))
            .env("HOME", self.root.join("home"))
            .env("CCS_HOME", self.home())
            .env("CCS_CLAUDE_BIN", self.root.join("claude"))
            .env_remove("CCS_PROFILE");
        for v in ccs::domain::PROVIDER_VARS {
            c.env_remove(v);
        }
        c.output().unwrap()
    }
    fn ok(&self, a: &[&str]) -> String {
        let out = self.run(a);
        assert!(out.status.success(), "ccs {a:?} failed: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap() + &String::from_utf8_lossy(&out.stderr)
    }
    fn err(&self, a: &[&str]) -> String {
        let out = self.run(a);
        assert!(!out.status.success(), "ccs {a:?} should fail: {}", String::from_utf8_lossy(&out.stdout));
        String::from_utf8(out.stderr).unwrap()
    }
    /// A plugin folder named `dir`, whose manifest calls it `name`.
    fn plugin_src(&self, dir: &str, name: &str) -> PathBuf {
        let d = self.root.join("src").join(dir);
        fs::create_dir_all(d.join(".claude-plugin")).unwrap();
        fs::create_dir_all(d.join("skills/egm_review")).unwrap();
        fs::write(
            d.join(".claude-plugin/plugin.json"),
            format!(r#"{{ "author": {{ "name": "Google LLC" }}, "name": "{name}", "description": "Goldfish reviews", "version": "0.1.0" }}"#),
        )
        .unwrap();
        fs::write(d.join("skills/egm_review/SKILL.md"), "---\nname: egm_review\n---\nv1\n").unwrap();
        d
    }
    /// `plugin_src` made a git repository with one commit.
    fn plugin_repo(&self, name: &str) -> PathBuf {
        let d = self.plugin_src(&format!("{name}-repo"), name);
        git(&d, &["init", "-q", "-b", "main"]);
        git(&d, &["add", "."]);
        git(&d, &["commit", "-q", "-m", "v1"]);
        d
    }
    fn launched(&self, a: &[&str]) -> (Vec<String>, String) {
        let out = self.ok(a);
        (out.lines().filter_map(|l| l.strip_prefix("ARG ")).map(String::from).collect(), out)
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn git(dir: &Path, a: &[&str]) {
    let out = Proc::new("git")
        .args(a)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {a:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn a_local_plugin_folder_is_added_under_its_manifest_name() {
    let e = Env::new();
    let src = e.plugin_src("elephant-goldfish-model", "egm");
    let out = e.ok(&["library", "add", s(&src)]);
    assert!(out.contains("plugins/egm"), "{out}");
    assert!(e.home().join("library/plugins/egm/.claude-plugin/plugin.json").is_file());
    let list = e.ok(&["library", "list", "plugins"]);
    assert!(list.contains("egm") && list.contains("Goldfish reviews"), "{list}");
    assert!(e.err(&["library", "add", s(&src)]).contains("egm"), "a name is unique");
}

#[test]
fn plugins_have_no_folders() {
    let e = Env::new();
    let src = e.plugin_src("egm", "egm");
    assert!(e.err(&["library", "add", s(&src), "--in", "x"]).contains("no folders"));
    e.ok(&["library", "add", s(&src)]);
    assert!(e.err(&["library", "mv", "plugins/egm", "x"]).contains("no folders"));
    assert!(e.err(&["library", "mkdir", "plugins/x"]).contains("no folders") || e.err(&["library", "mkdir", "plugins/x"]).contains("plugins"));
}

#[test]
fn a_git_source_is_cloned_and_update_pulls_it() {
    let e = Env::new();
    let repo = e.plugin_repo("egm");
    let url = format!("file://{}", s(&repo));
    let out = e.ok(&["library", "add", &url]);
    assert!(out.contains("plugins/egm"), "{out}");
    let skill = e.home().join("library/plugins/egm/skills/egm_review/SKILL.md");
    assert!(fs::read_to_string(&skill).unwrap().contains("v1"));
    assert!(e.home().join("library/plugins/egm/.git").is_dir(), "kept as a clone, so it can update");

    fs::write(repo.join("skills/egm_review/SKILL.md"), "---\nname: egm_review\n---\nv2\n").unwrap();
    git(&repo, &["commit", "-q", "-am", "v2"]);
    let out = e.ok(&["library", "update"]);
    assert!(out.contains("egm"), "{out}");
    assert!(fs::read_to_string(&skill).unwrap().contains("v2"), "update pulled the new commit");
    e.ok(&["library", "add", &format!("file://{}", s(&e.plugin_repo("other")))]);
    let out = e.ok(&["library", "update", "plugins/egm"]);
    assert!(out.contains("plugins/egm") && !out.contains("plugins/other"), "only the one named: {out}");
    let out = e.ok(&["library", "update"]);
    assert!(out.contains("plugins/egm") && out.contains("plugins/other"), "{out}");
}

#[test]
fn a_local_path_wins_over_reading_it_as_owner_repo() {
    let e = Env::new();
    let src = e.plugin_src("egm", "egm");
    let vendor = e.root.join("cwd/vendor");
    fs::create_dir_all(&vendor).unwrap();
    fs::rename(&src, vendor.join("egm")).unwrap();
    // `vendor/egm` also reads as a GitHub owner/repo; git must not be asked.
    let mut c = Proc::new(env!("CARGO_BIN_EXE_ccs"));
    let out = c
        .args(["library", "add", "vendor/egm"])
        .current_dir(e.root.join("cwd"))
        .env("HOME", e.root.join("home"))
        .env("CCS_HOME", e.home())
        .env("CCS_GIT_BIN", "/bin/false")
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(e.home().join("library/plugins/egm").is_dir());
}

#[test]
fn update_says_which_plugins_are_not_clones() {
    let e = Env::new();
    e.ok(&["library", "add", s(&e.plugin_src("egm", "egm"))]);
    assert!(e.ok(&["library", "update"]).contains("not a git clone"));
}

#[test]
fn a_git_source_that_is_not_a_plugin_adds_nothing() {
    let e = Env::new();
    let d = e.root.join("src/plain");
    fs::create_dir_all(&d).unwrap();
    fs::write(d.join("README.md"), "hi").unwrap();
    git(&d, &["init", "-q", "-b", "main"]);
    git(&d, &["add", "."]);
    git(&d, &["commit", "-q", "-m", "x"]);
    let err = e.err(&["library", "add", &format!("file://{}", s(&d))]);
    assert!(err.contains(".claude-plugin/plugin.json"), "{err}");
    assert!(!e.home().join("library/plugins").exists() || fs::read_dir(e.home().join("library/plugins")).unwrap().next().is_none());
}

#[test]
fn a_kit_loads_its_plugins_once_after_the_kits() {
    let e = Env::new();
    e.ok(&["library", "add", s(&e.plugin_src("egm", "egm"))]);
    e.ok(&["new", "work"]);
    for k in ["dev", "net"] {
        e.ok(&["kit", "new", k]);
        e.ok(&["kit", "use", k, "plugins/egm"]);
    }
    e.ok(&["kit", "attach", "dev", "work"]);
    e.ok(&["kit", "attach", "net", "work"]);
    let (argv, _) = e.launched(&["work", "--resume"]);
    let (dev, net, egm) = (e.home().join(".build/dev"), e.home().join(".build/net"), e.home().join("library/plugins/egm"));
    assert_eq!(argv, ["--plugin-dir", s(&dev), "--plugin-dir", s(&net), "--plugin-dir", s(&egm), "--resume"], "egm once, after the kits");
    assert!(!dev.join("plugins").exists(), "nothing of the plugin is built into the kit");
    assert!(e.ok(&["kit", "show", "dev"]).contains("plugins/egm"));
}

#[test]
fn a_kit_and_a_plugin_with_one_name_warn_at_launch() {
    let e = Env::new();
    e.ok(&["library", "add", s(&e.plugin_src("egm", "egm"))]);
    e.ok(&["new", "work"]);
    e.ok(&["kit", "new", "egm"]);
    e.ok(&["kit", "use", "egm", "plugins/egm"]);
    e.ok(&["kit", "attach", "egm", "work"]);
    let out = e.ok(&["work"]);
    assert!(out.contains("egm:"), "the clash is named: {out}");
}

#[test]
fn removing_a_plugin_names_its_kits_and_a_launch_goes_on_without_it() {
    let e = Env::new();
    e.ok(&["library", "add", s(&e.plugin_src("egm", "egm"))]);
    e.ok(&["new", "work"]);
    e.ok(&["kit", "new", "dev"]);
    e.ok(&["kit", "use", "dev", "plugins/egm"]);
    e.ok(&["kit", "attach", "dev", "work"]);
    let out = e.ok(&["library", "rm", "plugins/egm"]);
    assert!(out.contains("dev"), "{out}");
    assert!(e.home().join(".trash").is_dir());
    let (argv, out) = e.launched(&["work"]);
    assert!(!argv.iter().any(|a| a.contains("library/plugins")), "{argv:?}");
    assert!(out.contains("plugins/egm"), "the launch names what is gone: {out}");
}
