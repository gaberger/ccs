//! Gate for ADR-2610261643 phase 4: start with nothing. No `native`, no
//! `shared/`; `ccs import` copies ~/.claude into the library; `ccs migrate`
//! converts the layout earlier versions left. Runs the real binary.

use ccs::domain::{kit_name_for_profile, parse_command, Command};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command as Proc, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static N: AtomicUsize = AtomicUsize::new(0);

struct Env {
    root: PathBuf,
}

impl Env {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("ccs-migrate-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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
    fn dotclaude(&self) -> PathBuf {
        self.root.join("home/.claude")
    }
    fn ccs(&self, a: &[&str]) -> Proc {
        let mut c = Proc::new(env!("CARGO_BIN_EXE_ccs"));
        c.args(a)
            .current_dir(self.root.join("cwd"))
            .env("HOME", self.root.join("home"))
            .env("CCS_HOME", self.home())
            .env("CCS_CLAUDE_BIN", self.root.join("claude"))
            .env_remove("CCS_PROFILE")
            .env_remove("VISUAL")
            .env_remove("EDITOR");
        for v in ccs::domain::PROVIDER_VARS {
            c.env_remove(v);
        }
        c
    }
    fn run(&self, a: &[&str]) -> Output {
        self.ccs(a).output().unwrap()
    }
    fn ok(&self, a: &[&str]) -> String {
        let out = self.run(a);
        assert!(out.status.success(), "ccs {a:?} failed: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    }
    fn err(&self, a: &[&str]) -> String {
        let out = self.run(a);
        assert!(!out.status.success(), "ccs {a:?} should fail: {}", String::from_utf8_lossy(&out.stdout));
        String::from_utf8(out.stderr).unwrap()
    }
    fn write(&self, rel: &str, text: &str) -> PathBuf {
        let p = self.root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, text).unwrap();
        p
    }
    fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
    }
    fn exists(&self, rel: &str) -> bool {
        let p = self.root.join(rel);
        p.exists() || p.is_symlink()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn skill(name: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: {name} skill\n---\n{body}\n")
}

fn link(target: &Path, at: &Path) {
    fs::create_dir_all(at.parent().unwrap()).unwrap();
    symlink(target, at).unwrap();
}

/// The layout the previous version of ccs leaves behind.
fn old_layout(e: &Env) {
    let h = e.home();
    e.write("profiles/shared/skills/common/SKILL.md", &skill("common", "shared body"));
    e.write("profiles/shared/agents/hex/rev.md", "---\ndescription: rev\n---\n");
    e.write("profiles/shared/CLAUDE.md", "SHARED RULES\n");

    e.write("profiles/work/profile.env", "CLAUDE_CODE_USE_BEDROCK=1\nAWS_REGION=us-east-1\n");
    link(&h.join("shared/skills/common"), &h.join("work/config/skills/common"));
    e.write("profiles/work/config/skills/own/SKILL.md", &skill("own", "work's own"));
    link(&h.join("shared/agents/hex"), &h.join("work/config/agents/hex"));
    e.write("profiles/work/config/agents/mine.md", "---\ndescription: mine\n---\n");
    link(&h.join("shared/CLAUDE.md"), &h.join("work/config/CLAUDE.md"));
    e.write("profiles/work/config/.claude.json", "{\"login\":true}");
    e.write("profiles/work/config/settings.json", "{\"model\":\"x\"}");

    // lab keeps its own copy of a shared skill: a clash.
    e.write("profiles/lab/profile.env", "");
    e.write("profiles/lab/config/skills/common/SKILL.md", &skill("common", "lab's copy"));
    e.write("profiles/lab/config/CLAUDE.md", "LAB RULES\n");
    e.write("profiles/lab/unshared", "agents/hex\n");

    e.write("profiles/My_Prof/profile.env", "");
    e.write("profiles/My_Prof/config/skills/odd/SKILL.md", &skill("odd", "x"));

    e.write("profiles/native/native", "");
    e.write("profiles/native/profile.env", "# native\n");
    e.write("profiles/.default", "native\n");

    e.write("home/.claude/skills/stock/SKILL.md", &skill("stock", "stock body"));
    e.write("home/.claude/agents/sa.md", "---\ndescription: sa\n---\n");
    e.write("home/.claude/CLAUDE.md", "STOCK RULES\n");
}

/// Every file under a dir with its bytes, to prove nothing changed.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(snapshot(&p));
        } else {
            out.push((p.clone(), fs::read(&p).unwrap()));
        }
    }
    out.sort();
    out
}

// ── Pure rules ───────────────────────────────────────────────────────────────

#[test]
fn a_profile_name_becomes_a_kit_name() {
    let k = |p: &str| kit_name_for_profile(p).as_str().to_string();
    assert_eq!(k("work"), "work");
    assert_eq!(k("My_Prof"), "my-prof");
    assert_eq!(k("claude.bedrock"), "claude-bedrock");
    assert_eq!(k("9lives"), "p-9lives");
    assert_eq!(k("kits"), "p-kits", "a reserved word is not a kit name");
}

#[test]
fn migrate_and_kit_edit_parse_and_old_words_are_still_reserved() {
    let p = |a: &[&str]| parse_command(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>(), |_| false);
    assert_eq!(p(&["migrate"]).unwrap(), Command::Migrate);
    for w in ["skill", "agent", "claude-md", "link", "shared", "migrate"] {
        assert!(ccs::domain::ProfileName::parse(w).is_err(), "{w}");
    }
}

// ── Start with nothing ───────────────────────────────────────────────────────

#[test]
fn nothing_chosen_launches_nothing() {
    let e = Env::new();
    e.ok(&["new", "work"]);
    let err = e.err(&[]);
    assert!(err.contains("no profile chosen") && err.contains("ccs ui"), "{err}");
    assert!(!e.exists("profiles/.default"), "ccs sets no default by itself");
    e.ok(&["default", "work"]);
    assert!(e.ok(&[]).contains("CCS_PROFILE=work"));
}

#[test]
fn a_new_profile_holds_no_assets() {
    let e = Env::new();
    e.ok(&["new", "work"]);
    let cfg = e.home().join("work/config");
    let entries: Vec<String> = fs::read_dir(&cfg).unwrap().flatten().map(|d| d.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(entries, ["settings.json"], "only the status line setting");
}

#[test]
fn cloning_copies_env_kits_and_settings_never_login() {
    let e = Env::new();
    e.ok(&["new", "work"]);
    e.ok(&["kit", "new", "net"]);
    e.ok(&["kit", "attach", "net", "work"]);
    e.write("profiles/work/profile.env", "AWS_REGION=us-east-1\n");
    e.write("profiles/work/config/settings.json", "{\"model\":\"x\"}");
    e.write("profiles/work/config/.claude.json", "{\"login\":true}");
    e.ok(&["new", "prod", "--from", "work"]);
    assert_eq!(e.read("profiles/prod/profile.env"), "AWS_REGION=us-east-1\n");
    assert_eq!(e.read("profiles/prod/kits"), "net\n");
    assert_eq!(e.read("profiles/prod/config/settings.json"), "{\"model\":\"x\"}");
    assert!(!e.exists("profiles/prod/config/.claude.json"));
}

#[test]
fn the_old_commands_name_their_replacements() {
    let e = Env::new();
    for (cmd, want) in [(&["skill", "list"][..], "ccs library"), (&["agent", "add", "x"][..], "ccs library"), (&["claude-md", "show"][..], "ccs kit edit"), (&["link"][..], "built at launch")] {
        let err = e.err(cmd);
        assert!(err.contains(want), "{cmd:?}: {err}");
    }
}

#[test]
fn kit_edit_opens_the_kit_instructions() {
    let e = Env::new();
    e.ok(&["kit", "new", "net"]);
    let ed = e.write("ed", "#!/bin/sh\nfor f; do :; done\necho RULE >> \"$f\"\n");
    fs::set_permissions(&ed, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let out = e.ccs(&["kit", "edit", "net"]).env("EDITOR", &ed).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(e.read("profiles/kits/net/CLAUDE.md"), "RULE\n");
}

#[test]
fn doctor_names_a_missing_kit_and_a_missing_entry() {
    let e = Env::new();
    e.ok(&["new", "work"]);
    e.ok(&["kit", "new", "net"]);
    e.write("profiles/kits/net/uses", "skills/gone\n");
    e.write("profiles/work/kits", "net\nghost\n");
    let out = e.run(&["doctor", "work"]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!out.status.success());
    assert!(text.contains("✗ kit 'ghost'"), "{text}");
    assert!(text.contains("! kit net: skills/gone"), "{text}");
}

// ── ccs import ───────────────────────────────────────────────────────────────

#[test]
fn import_copies_dotclaude_into_the_library_and_changes_nothing_there() {
    let e = Env::new();
    e.write("home/.claude/skills/stock/SKILL.md", &skill("stock", "b"));
    e.write("home/.claude/skills/not-a-skill/readme.txt", "x");
    e.write("home/.claude/agents/team/sa.md", "---\ndescription: sa\n---\n");
    e.write("home/.claude/CLAUDE.md", "STOCK RULES\n");
    e.write("home/.claude/settings.json", "{}");
    let before = snapshot(&e.dotclaude());

    let out = e.ok(&["import"]);
    assert!(out.contains("skills/stock") && out.contains("kit imported"), "{out}");
    assert!(e.exists("profiles/library/skills/imported/stock/SKILL.md"));
    assert!(e.exists("profiles/library/agents/imported/team/sa.md"), "agent folders are kept");
    assert!(!e.exists("profiles/library/skills/imported/not-a-skill"));
    assert_eq!(e.read("profiles/kits/imported/uses"), "skills/imported/\nagents/imported/\n");
    assert_eq!(e.read("profiles/kits/imported/CLAUDE.md"), "STOCK RULES\n");
    assert_eq!(snapshot(&e.dotclaude()), before, "~/.claude is read, never written");
    assert!(!e.exists("profiles/.default") && e.ok(&["list"]).trim().starts_with("No profiles"), "import makes no profile");

    let again = e.ok(&["import"]);
    assert!(again.contains("already in the library"), "{again}");
}

// ── ccs migrate ──────────────────────────────────────────────────────────────

#[test]
fn an_old_layout_is_pointed_at_migrate() {
    let e = Env::new();
    old_layout(&e);
    let out = e.run(&["list"]);
    assert!(String::from_utf8_lossy(&out.stderr).contains("ccs migrate"), "{}", String::from_utf8_lossy(&out.stderr));
    let list = String::from_utf8_lossy(&out.stdout);
    assert!(!list.contains("native"), "native is no longer a profile: {list}");
}

#[test]
fn migrate_moves_shared_and_own_items_into_the_library_and_kits() {
    let e = Env::new();
    old_layout(&e);
    let before = snapshot(&e.dotclaude());
    let out = e.ok(&["migrate"]);

    // shared/ → library/…/shared/ and kit shared, attached to every profile.
    assert_eq!(e.read("profiles/library/skills/shared/common/SKILL.md"), skill("common", "shared body"));
    assert!(e.exists("profiles/library/agents/shared/hex/rev.md"));
    assert_eq!(e.read("profiles/kits/shared/uses"), "skills/shared/\nagents/shared/\n");
    assert_eq!(e.read("profiles/kits/shared/CLAUDE.md"), "SHARED RULES\n");
    assert!(!e.exists("profiles/shared"), "shared/ goes to the trash");

    // work's own items → library/…/work/ and kit work; ccs links into shared are gone.
    assert!(e.exists("profiles/library/skills/work/own/SKILL.md"));
    assert!(e.exists("profiles/library/agents/work/mine.md"));
    assert_eq!(e.read("profiles/kits/work/uses"), "skills/work/\nagents/work/\n");
    assert_eq!(e.read("profiles/work/kits"), "shared\nwork\n");
    for gone in ["skills/common", "skills/own", "agents/hex", "agents/mine.md", "CLAUDE.md"] {
        assert!(!e.exists(&format!("profiles/work/config/{gone}")), "{gone} left in work's config");
    }
    assert_eq!(e.read("profiles/work/config/.claude.json"), "{\"login\":true}", "login state stays");
    assert_eq!(e.read("profiles/work/config/settings.json"), "{\"model\":\"x\"}");
    assert!(!e.exists("profiles/lab/unshared") && !e.exists("profiles/work/unshared"));

    // lab's own copy of `common` clashes: it stays put and is named.
    assert_eq!(e.read("profiles/lab/config/skills/common/SKILL.md"), skill("common", "lab's copy"));
    assert!(out.contains("lab") && out.contains("skills/common") && out.contains("left in place"), "{out}");
    assert_eq!(e.read("profiles/kits/lab/CLAUDE.md"), "LAB RULES\n");
    assert_eq!(e.read("profiles/lab/kits"), "shared\nlab\n");
    assert!(out.contains("agents/hex"), "lab had opted out of agents/hex; the shared kit now gives it: {out}");

    // A profile name that is not a kit name.
    assert!(e.exists("profiles/library/skills/my-prof/odd/SKILL.md"));
    assert_eq!(e.read("profiles/My_Prof/kits"), "shared\nmy-prof\n");

    // native: no longer a profile; ~/.claude imported, untouched; its default cleared.
    assert!(!e.exists("profiles/native"));
    assert!(!e.exists("profiles/.default"));
    assert!(e.exists("profiles/library/skills/imported/stock/SKILL.md"));
    assert_eq!(e.read("profiles/kits/imported/CLAUDE.md"), "STOCK RULES\n");
    assert_eq!(snapshot(&e.dotclaude()), before);

    // The result launches.
    let launch = e.ok(&["work"]);
    assert!(launch.contains(&format!("ARG {}", e.home().join(".build/shared").display())), "{launch}");
    assert!(e.home().join(".build/shared/skills/common").is_symlink());

    // Idempotent, and quiet about it.
    assert!(e.ok(&["migrate"]).contains("Nothing to migrate"));
    let list = e.run(&["list"]);
    assert!(!String::from_utf8_lossy(&list.stderr).contains("migrate"));
}

#[test]
fn migrate_on_a_new_layout_does_nothing() {
    let e = Env::new();
    e.ok(&["new", "work"]);
    assert!(e.ok(&["migrate"]).contains("Nothing to migrate"));
}

#[test]
fn an_empty_shared_dir_makes_no_kit() {
    // Seen on a real machine: earlier versions always made shared/, often empty.
    let e = Env::new();
    e.write("profiles/work/profile.env", "");
    fs::create_dir_all(e.home().join("shared")).unwrap();
    e.ok(&["migrate"]);
    assert!(!e.exists("profiles/kits/shared"), "no empty kit");
    assert!(!e.exists("profiles/work/kits") || e.read("profiles/work/kits").is_empty());
    assert!(!e.exists("profiles/shared"));
}

#[test]
fn a_name_twice_in_dotclaude_is_imported_once_and_the_other_is_named() {
    // Seen on a real machine: ~/.claude/agents held the same agent name in two folders.
    let e = Env::new();
    e.write("home/.claude/agents/analysis/review.md", "---\ndescription: one\n---\n");
    e.write("home/.claude/agents/core/review.md", "---\ndescription: two\n---\n");
    let out = e.ok(&["import"]);
    assert!(e.exists("profiles/library/agents/imported/analysis/review.md"));
    assert!(!e.exists("profiles/library/agents/imported/core/review.md"));
    let line = out.lines().find(|l| l.contains("core/review")).unwrap_or_else(|| panic!("{out}"));
    assert!(line.contains("not imported") && line.contains("analysis/review"), "{line}");
    assert!(!out.contains("left in place"), "nothing moves out of ~/.claude: {out}");
}
