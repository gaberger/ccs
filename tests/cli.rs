//! End-to-end gate: run the real `ccs` binary. "It compiles" is not "it works".
//!
//! `CCS_CLAUDE_BIN` points at a script that runs `env` instead of `claude`, so
//! the test reads back exactly the environment and arguments `claude` would receive.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static N: AtomicUsize = AtomicUsize::new(0);

struct Env {
    root: PathBuf,
}

impl Env {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ccs-cli-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("home/.claude")).unwrap();
        fs::create_dir_all(root.join("cwd")).unwrap();
        // `env` stands in for claude; the launch's leading `--settings <json>` (its
        // status line, tested in tests/kits.rs) is dropped first, since env has no such flag.
        let claude = root.join("claude");
        fs::write(&claude, "#!/bin/sh\n[ \"$1\" = --settings ] && shift 2\nexec env \"$@\"\n").unwrap();
        fs::set_permissions(&claude, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        Env { root }
    }

    fn ccs(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ccs"));
        c.args(args)
            .current_dir(self.root.join("cwd"))
            .env("HOME", self.root.join("home"))
            .env("CCS_HOME", self.root.join("profiles"))
            .env("CCS_CLAUDE_BIN", self.root.join("claude"))
            .env_remove("CCS_PROFILE")
            .env_remove("CCS_KITS")
            .env_remove("VISUAL")
            .env_remove("EDITOR");
        // The shell running the tests may itself be on Bedrock; its provider
        // variables must not decide what a test sees.
        for v in ccs::domain::PROVIDER_VARS {
            c.env_remove(v);
        }
        c
    }

    fn run(&self, args: &[&str]) -> Output {
        self.ccs(args).output().unwrap()
    }

    fn ok(&self, args: &[&str]) -> String {
        let out = self.run(args);
        assert!(out.status.success(), "ccs {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// `env` prints NAME=value lines; find one.
fn var<'a>(env_out: &'a str, key: &str) -> Option<&'a str> {
    env_out.lines().find_map(|l| l.strip_prefix(&format!("{key}=")))
}

#[test]
fn launching_a_bedrock_profile_hands_claude_the_profile_env() {
    let e = Env::new();
    e.ok(&["new", "work"]);
    fs::write(
        e.root.join("profiles/work/profile.env"),
        "CLAUDE_CODE_USE_BEDROCK=1\nAWS_PROFILE=work-sso\nAWS_REGION=us-east-1\n",
    )
    .unwrap();

    let out = e.ok(&["work"]);
    assert_eq!(var(&out, "CLAUDE_CODE_USE_BEDROCK"), Some("1"));
    assert_eq!(var(&out, "AWS_PROFILE"), Some("work-sso"));
    assert_eq!(var(&out, "CCS_PROFILE"), Some("work"));
    let cfg = e.root.join("profiles/work/config");
    assert_eq!(var(&out, "CLAUDE_CONFIG_DIR"), Some(cfg.to_str().unwrap()));
}

#[test]
fn a_bedrock_shell_does_not_leak_into_a_personal_profile() {
    let e = Env::new();
    e.ok(&["new", "personal"]);
    let out = e
        .ccs(&["personal"])
        .env("CLAUDE_CODE_USE_BEDROCK", "1")
        .env("AWS_PROFILE", "leaky")
        .env("CLAUDE_CONFIG_DIR", "/wrong")
        .output()
        .unwrap();
    let out = String::from_utf8(out.stdout).unwrap();
    assert_eq!(var(&out, "CLAUDE_CODE_USE_BEDROCK"), None, "{out}");
    assert_eq!(var(&out, "AWS_PROFILE"), None);
    assert_ne!(var(&out, "CLAUDE_CONFIG_DIR"), Some("/wrong"));
}

#[test]
fn claude_arguments_pass_through_untouched() {
    // `env` treats trailing words as a command to run; `echo` proves the args arrived in order.
    let e = Env::new();
    e.ok(&["new", "work"]);
    let out = e.ok(&["work", "echo", "--resume", "-p", "two words"]);
    assert_eq!(out, "--resume -p two words\n");
}

#[test]
fn a_pin_file_beats_the_default() {
    let e = Env::new();
    e.ok(&["new", "lab"]);
    e.ok(&["default", "lab"]);
    e.ok(&["new", "work"]);
    fs::write(e.root.join("cwd/.claude-profile"), "work\n").unwrap();
    assert_eq!(e.ok(&["which"]), "work\n");
    assert_eq!(var(&e.ok(&[]), "CCS_PROFILE"), Some("work"));
}

#[test]
fn env_prints_evaluable_exports() {
    let e = Env::new();
    e.ok(&["new", "work"]);
    fs::write(e.root.join("profiles/work/profile.env"), "AWS_PROFILE=w\n").unwrap();
    let out = e.ok(&["env", "work"]);
    assert!(out.contains("unset CLAUDE_CODE_USE_BEDROCK\n"), "{out}");
    assert!(out.contains("export AWS_PROFILE='w'\n"), "{out}");

    // The output really is shell: eval it and read the value back.
    let sh = Command::new("sh").arg("-c").arg(format!("{out}\necho \"$AWS_PROFILE/$CCS_PROFILE\"")).output().unwrap();
    assert_eq!(String::from_utf8(sh.stdout).unwrap(), "w/work\n");
}

#[test]
fn no_profile_at_all_is_a_clear_error() {
    let e = Env::new();
    let out = e.run(&[]);
    assert!(!out.status.success());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("ccs new <name>"), "the error must say how to start: {err}");
}

#[test]
fn an_unknown_profile_name_is_an_error_not_a_launch() {
    let e = Env::new();
    e.ok(&["new", "work"]);
    let out = e.run(&["env", "ghost"]);
    assert!(!out.status.success());
    assert!(String::from_utf8(out.stderr).unwrap().contains("ghost"));
}

#[test]
fn list_shows_profiles() {
    let e = Env::new();
    e.ok(&["new", "lab"]);
    e.ok(&["new", "work"]);
    e.ok(&["default", "lab"]);
    let out = e.ok(&["list"]);
    assert!(out.contains("* lab (default)\n"), "{out}");
    assert!(out.contains("  work\n"), "{out}");
}

#[test]
fn doctor_exits_nonzero_on_a_broken_profile() {
    let e = Env::new();
    e.ok(&["new", "work"]);
    fs::write(e.root.join("profiles/work/profile.env"), "CLAUDE_CODE_USE_BEDROCK=1\n").unwrap();
    let out = e.run(&["doctor", "work"]);
    assert!(!out.status.success());
    assert!(String::from_utf8(out.stdout).unwrap().contains("AWS_REGION"));
}

// ── Status line and AWS import, through the real binary ─────────────────────

fn script(e: &Env, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let p = e.root.join(name);
    fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    p
}

fn with_stdin(mut cmd: Command, input: &str) -> Output {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn statusline_wraps_a_real_script_and_passes_stdin_through() {
    let e = Env::new();
    // The wrapped script echoes the model id it read from stdin, like a real status line.
    let sl = script(&e, "sl.sh", "sed -n 's/.*\"id\":\"\\([^\"]*\\)\".*/model \\1/p'");
    let mut cmd = e.ccs(&["statusline", "--", sl.to_str().unwrap()]);
    cmd.env("CCS_PROFILE", "work").env("CLAUDE_CODE_USE_BEDROCK", "1").env("AWS_PROFILE", "sso").env("AWS_REGION", "us-east-1");
    let out = with_stdin(cmd, "{\"model\":{\"id\":\"opus\"}}");
    assert!(out.status.success());
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "⬡ work · bedrock sso@us-east-1 │ model opus\n");
}

#[test]
fn statusline_survives_a_missing_wrapped_script() {
    let e = Env::new();
    let mut cmd = e.ccs(&["statusline", "--", "/nonexistent/sl.sh"]);
    cmd.env("CCS_PROFILE", "work");
    let out = with_stdin(cmd, "{}");
    assert!(out.status.success(), "a status line command must not crash the bar");
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "⬡ work · anthropic │ ⚠ statusline failed: /nonexistent/sl.sh\n");
}

/// The status line every launch sets: the segment, the profile's own line, then the project's.
#[test]
fn statusline_auto_puts_the_profile_before_the_projects_own_line() {
    let e = Env::new();
    let proj = e.root.join("proj");
    fs::create_dir_all(proj.join(".claude")).unwrap();
    let line = |cmd: &str| format!(r#"{{"statusLine": {{"type": "command", "command": "{cmd}"}}}}"#);
    let run = |cfg: Option<&std::path::Path>, project: &std::path::Path| {
        let mut cmd = e.ccs(&["statusline", "--auto"]);
        cmd.env("CCS_PROFILE", "work").env("CCS_KITS", "base,dev").env_remove("CLAUDE_CONFIG_DIR");
        if let Some(c) = cfg {
            cmd.env("CLAUDE_CONFIG_DIR", c);
        }
        let out = with_stdin(cmd, &format!(r#"{{"workspace": {{"project_dir": "{}"}}}}"#, project.display()));
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap()
    };
    fs::write(proj.join(".claude/settings.json"), line("echo hexa-loop")).unwrap();
    assert_eq!(run(None, &proj), "⬡ work +base +dev · anthropic │ hexa-loop\n");
    fs::write(proj.join(".claude/settings.local.json"), line("echo mine")).unwrap();
    assert_eq!(run(None, &proj), "⬡ work +base +dev · anthropic │ mine\n", "local settings come first");

    let cfg = e.root.join("cfg");
    fs::create_dir_all(&cfg).unwrap();
    fs::write(cfg.join("settings.json"), line("'/x/ccs' statusline -- echo profile-line")).unwrap();
    let bare = e.root.join("bare");
    fs::create_dir_all(&bare).unwrap();
    assert_eq!(run(Some(&cfg), &bare), "⬡ work +base +dev · anthropic │ profile-line\n", "no project line: the profile's");
    assert_eq!(run(Some(&cfg), &proj), "⬡ work +base +dev · anthropic │ profile-line\nmine\n", "both: the profile's, then the project's");
    fs::write(cfg.join("settings.json"), line("echo mine")).unwrap();
    assert_eq!(run(Some(&cfg), &proj), "⬡ work +base +dev · anthropic │ mine\n", "the same command runs once");
    assert_eq!(run(None, &bare), "⬡ work +base +dev · anthropic\n", "nothing else: the segment alone");
}

#[test]
fn a_new_profile_launches_with_the_ccs_statusline_configured() {
    let e = Env::new();
    e.ok(&["new", "work"]);
    let settings = fs::read_to_string(e.root.join("profiles/work/config/settings.json")).unwrap();
    let bin = env!("CARGO_BIN_EXE_ccs");
    assert!(settings.contains(&format!("'{bin}' statusline")), "{settings}");
}

#[test]
fn import_aws_lists_then_imports_through_the_aws_cli() {
    let e = Env::new();
    let aws = script(
        &e,
        "aws",
        r#"case "$1 $2" in
  "configure list-profiles") printf 'claude-bedrock\ndefault\nhorizon\n' ;;
  "configure get") [ "$5" = claude-bedrock ] && echo us-west-2 || exit 1 ;;
  *) exit 2 ;;
esac"#,
    );
    let run = |args: &[&str]| {
        let out = e.ccs(args).env("CCS_AWS_BIN", &aws).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    };

    let listing = run(&["import", "aws"]);
    assert!(listing.contains("claude-bedrock"), "{listing}");
    assert!(listing.contains("horizon"), "{listing}");
    assert!(!e.root.join("profiles/claude-bedrock").exists(), "listing must not import");

    let done = run(&["import", "aws", "claude-bedrock"]);
    assert!(done.contains("claude-bedrock"), "{done}");
    let launched = e.ok(&["claude-bedrock"]);
    assert_eq!(var(&launched, "AWS_PROFILE"), Some("claude-bedrock"));
    assert_eq!(var(&launched, "AWS_REGION"), Some("us-west-2"));
    assert_eq!(var(&launched, "CLAUDE_CODE_USE_BEDROCK"), Some("1"));
    assert!(run(&["import", "aws"]).contains("claude-bedrock → claude-bedrock"));
}

#[test]
fn import_aws_without_the_aws_cli_says_so() {
    let e = Env::new();
    let out = e.ccs(&["import", "aws"]).env("CCS_AWS_BIN", "/nonexistent/aws").output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8(out.stderr).unwrap().contains("aws"));
}

// ── Assets through the real binary (ADR-2610261244) ─────────────────────────

// ── Edit (ADR-2610261643 phase 1, on the library since phase 2) ───────────────────────────────────────────

/// A stand-in editor: appends its arguments to the last one (the file), and
/// leaves a marker so a test can tell it never ran.
fn fake_editor(e: &Env) -> PathBuf {
    let p = e.root.join("fake-editor");
    fs::write(&p, "#!/bin/sh\nfor f; do :; done\necho \"edited $*\" >> \"$f\"\ntouch \"$(dirname \"$0\")/editor-ran\"\n").unwrap();
    fs::set_permissions(&p, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    p
}

fn with_skill(e: &Env) {
    let src = e.root.join("src/demo");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("SKILL.md"), "---\ndescription: d\n---\nbody\n").unwrap();
    e.ok(&["library", "add", src.to_str().unwrap(), "--in", "tools"]);
}

#[test]
fn edit_opens_the_item_in_visual_with_its_arguments_and_saves_in_place() {
    let e = Env::new();
    with_skill(&e);
    let ed = fake_editor(&e);
    let out = e.ccs(&["library", "edit", "skills/demo"]).env("VISUAL", format!("{} --wait", ed.display())).env("EDITOR", "false").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = fs::read_to_string(e.root.join("profiles/library/skills/tools/demo/SKILL.md")).unwrap();
    assert!(text.starts_with("---\ndescription: d\n---\nbody\n"), "the file is edited, not replaced: {text}");
    assert!(text.contains("edited --wait "), "VISUAL wins over EDITOR and keeps its arguments: {text}");
}

#[test]
fn edit_falls_back_to_editor_and_edits_an_agent() {
    let e = Env::new();
    let agent = e.root.join("rev.md");
    fs::write(&agent, "---\ndescription: r\n---\n").unwrap();
    e.ok(&["library", "add", agent.to_str().unwrap()]);
    let out = e.ccs(&["library", "edit", "agents/rev"]).env("EDITOR", fake_editor(&e)).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(fs::read_to_string(e.root.join("profiles/library/agents/rev.md")).unwrap().contains("edited "));
}

#[test]
fn editing_a_missing_item_never_starts_the_editor() {
    let e = Env::new();
    with_skill(&e);
    let out = e.ccs(&["library", "edit", "skills/nope"]).env("EDITOR", fake_editor(&e)).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no skills/nope"), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(!e.root.join("editor-ran").exists());
}

#[test]
fn a_failing_editor_is_reported() {
    let e = Env::new();
    with_skill(&e);
    let out = e.ccs(&["library", "edit", "skills/demo"]).env("EDITOR", "false").output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("editor"), "{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn editing_kit_instructions_makes_them_when_the_kit_has_none() {
    let e = Env::new();
    e.ok(&["kit", "new", "net"]);
    let out = e.ccs(&["kit", "edit", "net"]).env("EDITOR", fake_editor(&e)).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(fs::read_to_string(e.root.join("profiles/kits/net/CLAUDE.md")).unwrap().contains("edited "));
}


#[test]
fn a_closed_pipe_ends_quietly_not_with_a_panic() {
    // `ccs library list | head -1` on a real library panicked with "Broken pipe".
    let e = Env::new();
    let mut child = e.ccs(&["help"]).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!err.contains("panicked"), "{err}");
}
