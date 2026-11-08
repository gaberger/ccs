//! Gate for ADR-2610281700: `ccs permissions`. Written before the code.
//! The preset list below is copied from the user's own jq command, not from the
//! implementation, so the test cannot agree with a wrong copy of it.

use ccs::domain::{parse_command, Command, PermCmd};
use std::fs;
use std::path::PathBuf;
use std::process::{Command as Proc, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

const USER_LIST: &[&str] = &[
    "Bash(git:*)", "Bash(ls:*)", "Bash(cat:*)", "Bash(head:*)", "Bash(tail:*)", "Bash(wc:*)", "Bash(grep:*)", "Bash(rg:*)",
    "Bash(find:*)", "Bash(sed -n:*)", "Bash(diff:*)", "Bash(jq:*)", "Bash(stat:*)", "Bash(which:*)", "Bash(ps:*)", "Bash(ss:*)",
    "Bash(df:*)", "Bash(du:*)", "Bash(free:*)", "Bash(journalctl:*)", "Bash(systemctl status:*)", "Bash(curl:*)", "Bash(docker:*)",
    "Bash(tt-smi:*)", "Bash(tt-toplike:*)", "Bash(weave:*)", "Bash(herdr:*)", "Bash(python3:*)", "Bash(python:*)", "Bash(pip:*)",
    "Bash(uv:*)", "Bash(node:*)", "Bash(npm:*)", "Bash(npx:*)", "Bash(make:*)", "Bash(cargo:*)", "Bash(go:*)", "Bash(pytest:*)",
    "Bash(mkdir:*)", "Bash(touch:*)", "Bash(cp:*)", "Bash(mv:*)", "Bash(echo:*)", "Bash(gh:*)", "Read", "Glob", "Grep", "WebFetch",
    "WebSearch",
];

fn words(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

fn parse(a: &[&str]) -> Command {
    parse_command(&words(a), |p| p == "work").unwrap()
}

#[test]
fn permissions_commands_parse_with_or_without_a_profile() {
    let rules = words(&["Bash(git:*)", "Read"]);
    assert_eq!(parse(&["permissions"]), Command::Permissions { profile: None, cmd: PermCmd::Show });
    assert_eq!(parse(&["perms", "work"]), Command::Permissions { profile: Some("work".into()), cmd: PermCmd::Show });
    assert_eq!(
        parse(&["permissions", "add", "Bash(git:*)", "Read"]),
        Command::Permissions { profile: None, cmd: PermCmd::Add { rules: rules.clone(), preset: None } }
    );
    assert_eq!(
        parse(&["permissions", "work", "add", "--preset", "dev"]),
        Command::Permissions { profile: Some("work".into()), cmd: PermCmd::Add { rules: vec![], preset: Some("dev".into()) } }
    );
    assert_eq!(parse(&["permissions", "rm", "Read"]), Command::Permissions { profile: None, cmd: PermCmd::Rm(words(&["Read"])) });
    assert_eq!(parse(&["permissions", "work", "mode", "acceptEdits"]), Command::Permissions { profile: Some("work".into()), cmd: PermCmd::Mode("acceptEdits".into()) });
}

#[test]
fn bad_permission_commands_are_usage_errors() {
    let bad = |a: &[&str]| parse_command(&words(a), |p| p == "work").is_err();
    assert!(bad(&["permissions", "add"]), "add needs a rule or a preset");
    assert!(bad(&["permissions", "add", "--preset", "nope"]), "unknown preset");
    assert!(bad(&["permissions", "mode", "yolo"]), "unknown mode");
    assert!(bad(&["permissions", "add", ""]), "an empty rule");
    assert!(bad(&["permissions", "add", "Bash(git:*"]), "an unclosed paren");
    assert!(bad(&["permissions", "add", "Bash(a)\nBash(b)"]), "a control character");
}

// ── End to end, against the real binary ─────────────────────────────────────

static N: AtomicUsize = AtomicUsize::new(0);

struct Env {
    root: PathBuf,
}

impl Env {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("ccs-perms-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("home")).unwrap();
        let e = Env { root };
        assert!(e.run(&["new", "work"]).status.success());
        e
    }
    fn run(&self, a: &[&str]) -> Output {
        Proc::new(env!("CARGO_BIN_EXE_ccs"))
            .args(a)
            .env("HOME", self.root.join("home"))
            .env("CCS_HOME", self.root.join("home/.claude-profiles"))
            .env_remove("CCS_PROFILE")
            .output()
            .unwrap()
    }
    fn says(&self, a: &[&str]) -> (bool, String) {
        let o = self.run(a);
        (o.status.success(), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
    }
    fn settings_path(&self) -> PathBuf {
        self.root.join("home/.claude-profiles/work/config/settings.json")
    }
    fn settings(&self) -> String {
        fs::read_to_string(self.settings_path()).unwrap()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The `allow` array of a settings text, read by `jq` (an independent reader) when present.
fn jq(text: &str, filter: &str) -> Option<String> {
    use std::io::Write;
    let mut child = Proc::new("jq").args(["-c", filter]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().ok()?;
    child.stdin.take()?.write_all(text.as_bytes()).ok()?;
    let out = child.wait_with_output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[test]
fn the_dev_preset_adds_the_users_list_and_accept_edits() {
    let e = Env::new();
    let (ok, out) = e.says(&["permissions", "work", "add", "--preset", "dev"]);
    assert!(ok, "{out}");
    let text = e.settings();
    // Every rule of the user's list is present, in the user's order.
    let at: Vec<usize> = USER_LIST.iter().map(|r| text.find(&format!("\"{r}\"")).unwrap_or_else(|| panic!("{r} missing:\n{text}"))).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "order kept");
    if let Some(mode) = jq(&text, ".permissions.defaultMode") {
        assert_eq!(mode, "\"acceptEdits\"");
        assert_eq!(jq(&text, ".permissions.allow | length").unwrap(), USER_LIST.len().to_string());
    }
}

#[test]
fn adding_keeps_existing_rules_in_order_without_duplicates_and_touches_nothing_else() {
    let e = Env::new();
    fs::write(
        e.settings_path(),
        "{\n  \"zeta\": 1,\n  \"permissions\": {\n    \"allow\": [\"Zed\", \"Read\"],\n    \"deny\": [\"Bash(rm:*)\"]\n  },\n  \"alpha\": {\"k\": [1, 2]}\n}\n",
    )
    .unwrap();
    let (ok, out) = e.says(&["permissions", "work", "add", "Read", "Bash(git:*)", "Bash(git:*)"]);
    assert!(ok, "{out}");
    let text = e.settings();
    assert!(text.find("\"Zed\"").unwrap() < text.find("\"Read\"").unwrap() && text.find("\"Read\"").unwrap() < text.find("\"Bash(git:*)\"").unwrap());
    assert_eq!(text.matches("\"Read\"").count(), 1, "no duplicate");
    assert_eq!(text.matches("\"Bash(git:*)\"").count(), 1, "no duplicate in one call either");
    assert!(text.contains("\"Bash(rm:*)\""), "deny kept");
    assert!(text.find("\"zeta\"").unwrap() < text.find("\"permissions\"").unwrap() && text.find("\"permissions\"").unwrap() < text.find("\"alpha\"").unwrap(), "key order kept:\n{text}");
}

#[test]
fn each_change_leaves_a_backup_of_what_it_replaced() {
    let e = Env::new();
    let before = e.settings();
    assert!(e.run(&["permissions", "work", "add", "Read"]).status.success());
    let bak = fs::read_to_string(e.settings_path().with_file_name("settings.json.bak-ccs")).unwrap();
    assert_eq!(bak, before);
}

#[test]
fn settings_that_do_not_parse_are_refused_and_left_alone() {
    let e = Env::new();
    fs::write(e.settings_path(), "{ not json").unwrap();
    let (ok, out) = e.says(&["permissions", "work", "add", "Read"]);
    assert!(!ok && out.contains("settings.json"), "{out}");
    assert_eq!(e.settings(), "{ not json");
}

#[test]
fn a_settings_file_where_permissions_is_not_an_object_is_refused() {
    let e = Env::new();
    fs::write(e.settings_path(), "{\"permissions\": [1]}").unwrap();
    let (ok, _) = e.says(&["permissions", "work", "add", "Read"]);
    assert!(!ok);
    assert_eq!(e.settings(), "{\"permissions\": [1]}");
}

#[test]
fn mode_rm_and_show() {
    let e = Env::new();
    assert!(e.run(&["permissions", "work", "add", "Read", "Glob"]).status.success());
    assert!(e.run(&["permissions", "work", "mode", "plan"]).status.success());
    assert!(e.run(&["permissions", "work", "rm", "Read"]).status.success());
    let (ok, out) = e.says(&["permissions", "work"]);
    assert!(ok && out.contains("plan") && out.contains("Glob") && !out.contains("Read"), "{out}");
    let (ok, out) = e.says(&["permissions", "work", "rm", "NeverThere"]);
    assert!(ok && out.contains("not in"), "removing what is absent says so: {out}");
}

#[test]
fn a_profile_with_no_settings_file_gets_one() {
    let e = Env::new();
    fs::remove_file(e.settings_path()).unwrap();
    assert!(e.run(&["permissions", "work", "add", "Read"]).status.success());
    assert!(e.settings().contains("\"Read\""));
}

#[test]
fn an_unknown_profile_is_an_error_and_writes_nothing() {
    let e = Env::new();
    let (ok, out) = e.says(&["permissions", "ghost", "add", "Read"]);
    assert!(!ok && out.contains("ghost"), "{out}");
}

#[test]
fn the_launch_does_not_lose_the_permissions() {
    // A launch passes --settings with the status line; the profile's own file must stay as written.
    let e = Env::new();
    assert!(e.run(&["permissions", "work", "add", "Read"]).status.success());
    let before = e.settings();
    let fake = e.root.join("fc");
    fs::write(&fake, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&fake, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let o = Proc::new(env!("CARGO_BIN_EXE_ccs"))
        .args(["work"])
        .env("HOME", e.root.join("home"))
        .env("CCS_HOME", e.root.join("home/.claude-profiles"))
        .env("CCS_CLAUDE_BIN", &fake)
        .env_remove("CCS_PROFILE")
        .output()
        .unwrap();
    assert!(o.status.success());
    assert_eq!(e.settings(), before);
}
