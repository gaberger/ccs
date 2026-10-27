//! Gate for ADR-2610261643 phase 2: a library in folders, kits, and a launch
//! that loads kits as session plugins. The pure rules are tested directly; the
//! rest runs the real `ccs` binary with a fake `claude` that prints what it got.

use ccs::domain::{kit_args, parse_command, resolve_kit, status_segment, Command, Kind, LibItem, Use};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as Proc, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn lib(kind: Kind, folder: &str, name: &str) -> LibItem {
    LibItem { kind, name: name.into(), folder: folder.into(), description: None, source: PathBuf::from(format!("/lib/{folder}/{name}")) }
}

// ── Pure rules ───────────────────────────────────────────────────────────────

#[test]
fn entries_parse_as_folder_or_item_and_print_back() {
    assert_eq!(Use::parse("skills/nqe/").unwrap(), Use::Folder(Kind::Skill, "nqe".into()));
    assert_eq!(Use::parse("skills/net/nqe/").unwrap(), Use::Folder(Kind::Skill, "net/nqe".into()));
    assert_eq!(Use::parse("agents/rev").unwrap(), Use::Item(Kind::Agent, "rev".into()));
    for line in ["skills/nqe/", "skills/net/nqe/", "agents/rev", "skills/find-nqe-query"] {
        assert_eq!(Use::parse(line).unwrap().line(), line);
    }
    for bad in ["", "x", "skills/", "skills", "claude-md/x", "skills/../x", "skills/a b", "skills/a/b", "skills//", "agents/.hidden"] {
        assert!(Use::parse(bad).is_err(), "{bad:?} must be refused");
    }
}

#[test]
fn a_kit_resolves_folders_deeply_items_by_name_once_and_names_what_is_missing() {
    let library = vec![
        lib(Kind::Skill, "net/nqe", "find-nqe-query"),
        lib(Kind::Skill, "net", "inspect-edge"),
        lib(Kind::Skill, "", "humanize"),
        lib(Kind::Skill, "network", "not-in-net"),
        lib(Kind::Agent, "hex", "rev"),
    ];
    let uses = vec![
        Use::Folder(Kind::Skill, "net".into()),
        Use::Item(Kind::Skill, "find-nqe-query".into()),
        Use::Item(Kind::Agent, "rev".into()),
        Use::Item(Kind::Skill, "gone".into()),
    ];
    let k = resolve_kit(&uses, &library);
    let names: Vec<&str> = k.items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["find-nqe-query", "inspect-edge", "rev"], "net/ includes net/nqe/, not network/; no duplicates");
    assert_eq!(k.missing, ["skills/gone"]);
}

#[test]
fn kits_become_plugin_dirs_and_one_prompt_file() {
    let dirs = [PathBuf::from("/h/.build/net"), PathBuf::from("/h/.build/rust")];
    assert_eq!(
        kit_args(&dirs, Some(Path::new("/h/.build/net+rust.md"))),
        args(&["--plugin-dir", "/h/.build/net", "--plugin-dir", "/h/.build/rust", "--append-system-prompt-file", "/h/.build/net+rust.md"])
    );
    assert!(kit_args(&[], None).is_empty());
}

#[test]
fn plus_words_after_the_profile_choose_kits_for_this_launch() {
    let p = |a: &[&str]| parse_command(&args(a), |s| s == "work").unwrap();
    assert_eq!(
        p(&["work", "+net", "+rust", "--resume"]),
        Command::Launch { profile: Some("work".into()), kits: Some(args(&["net", "rust"])), args: args(&["--resume"]) }
    );
    assert_eq!(p(&["+net"]), Command::Launch { profile: None, kits: Some(args(&["net"])), args: vec![] });
    assert_eq!(p(&["work", "--resume"]), Command::Launch { profile: Some("work".into()), kits: None, args: args(&["--resume"]) });
    assert_eq!(
        p(&["work", "-p", "+1"]),
        Command::Launch { profile: Some("work".into()), kits: None, args: args(&["-p", "+1"]) },
        "only leading + words are kits; later ones belong to claude"
    );
}

#[test]
fn the_status_segment_names_the_kits() {
    let v = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<Vec<_>>();
    assert_eq!(status_segment(&v(&[("CCS_PROFILE", "work"), ("CCS_KITS", "net,rust")])).unwrap(), "⬡ work +net +rust · anthropic");
    assert_eq!(status_segment(&v(&[("CCS_PROFILE", "work")])).unwrap(), "⬡ work · anthropic");
}

// ── End to end ───────────────────────────────────────────────────────────────

static N: AtomicUsize = AtomicUsize::new(0);

struct Env {
    root: PathBuf,
}

impl Env {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("ccs-kits-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("home/.claude")).unwrap();
        fs::create_dir_all(root.join("cwd")).unwrap();
        // A fake claude: one line per argument, then the environment.
        let claude = root.join("claude");
        fs::write(&claude, "#!/bin/sh\nfor a; do echo \"ARG $a\"; done\nenv\n").unwrap();
        fs::set_permissions(&claude, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        Env { root }
    }
    fn home(&self) -> PathBuf {
        self.root.join("profiles")
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
    fn skill_src(&self, name: &str) -> String {
        let d = self.root.join("src").join(name);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("SKILL.md"), format!("---\nname: {name}\ndescription: about {name}\n---\nbody\n")).unwrap();
        d.to_string_lossy().into_owned()
    }
    fn agent_src(&self, name: &str) -> String {
        let p = self.root.join("src").join(format!("{name}.md"));
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, format!("---\nname: {name}\ndescription: agent {name}\n---\nprompt\n")).unwrap();
        p.to_string_lossy().into_owned()
    }
    /// The args the fake claude received.
    fn launched(&self, a: &[&str]) -> (Vec<String>, String) {
        let out = self.ok(a);
        (without_settings(out.lines().filter_map(|l| l.strip_prefix("ARG ")).map(String::from).collect()), out)
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn var<'a>(out: &'a str, key: &str) -> Option<&'a str> {
    out.lines().find_map(|l| l.strip_prefix(&format!("{key}=")))
}

/// A library with `net/nqe/find-nqe-query`, `net/inspect-edge`, `humanize`, and agent `hex/rev`.
fn stocked(e: &Env) {
    e.ok(&["library", "add", &e.skill_src("find-nqe-query"), "--in", "net/nqe"]);
    e.ok(&["library", "add", &e.skill_src("inspect-edge"), "--in", "net"]);
    e.ok(&["library", "add", &e.skill_src("humanize")]);
    e.ok(&["library", "add", &e.agent_src("rev"), "--in", "hex"]);
}

#[test]
fn the_library_keeps_folders_and_lists_them_as_a_tree() {
    let e = Env::new();
    stocked(&e);
    assert!(e.home().join("library/skills/net/nqe/find-nqe-query/SKILL.md").is_file());
    assert!(e.home().join("library/agents/hex/rev.md").is_file());
    let out = e.ok(&["library", "list"]);
    for want in ["skills", "net/", "nqe/", "find-nqe-query", "about inspect-edge", "humanize", "agents", "hex/", "rev"] {
        assert!(out.contains(want), "missing {want:?} in:\n{out}");
    }
    assert!(!e.ok(&["library", "list", "agents"]).contains("humanize"), "a kind filter lists one kind");
}

#[test]
fn a_name_is_unique_in_the_library_whatever_its_folder() {
    let e = Env::new();
    stocked(&e);
    let err = e.err(&["library", "add", &e.skill_src("humanize"), "--in", "writing"]);
    assert!(err.contains("skills/humanize") && err.contains("already"), "{err}");
    assert!(!e.home().join("library/skills/writing/humanize").exists());
}

#[test]
fn moving_an_item_keeps_every_kit_that_picked_it_by_name() {
    let e = Env::new();
    stocked(&e);
    e.ok(&["kit", "new", "net"]);
    e.ok(&["kit", "use", "net", "skills/find-nqe-query"]);
    e.ok(&["library", "mkdir", "skills/tools"]);
    e.ok(&["library", "mv", "skills/find-nqe-query", "tools"]);
    assert!(e.home().join("library/skills/tools/find-nqe-query/SKILL.md").is_file());
    assert!(!e.home().join("library/skills/net/nqe/find-nqe-query").exists());
    assert!(e.ok(&["kit", "show", "net"]).contains("tools/find-nqe-query"));
    // A folder moves whole, into a folder that is made if needed; `/` is the top of the library.
    e.ok(&["library", "mv", "skills/tools/", "net/kit"]);
    assert!(e.home().join("library/skills/net/kit/tools/find-nqe-query/SKILL.md").is_file());
    e.ok(&["library", "mv", "skills/net/kit/tools/", "/"]);
    assert!(e.home().join("library/skills/tools/find-nqe-query/SKILL.md").is_file());
    assert!(e.ok(&["kit", "show", "net"]).contains("tools/find-nqe-query"), "still picked after two moves");
    let err = e.err(&["library", "mv", "skills/net/", "net/kit"]);
    assert!(err.contains("into itself"), "{err}");
}

#[test]
fn a_kit_uses_folders_and_items_and_refuses_what_the_library_lacks() {
    let e = Env::new();
    stocked(&e);
    e.ok(&["kit", "new", "net"]);
    e.ok(&["kit", "use", "net", "skills/net/", "agents/rev"]);
    let err = e.err(&["kit", "use", "net", "skills/nope"]);
    assert!(err.contains("skills/nope"), "{err}");
    let show = e.ok(&["kit", "show", "net"]);
    for want in ["skills/net/", "agents/rev", "find-nqe-query", "inspect-edge", "hex/rev"] {
        assert!(show.contains(want), "missing {want:?} in:\n{show}");
    }
    assert!(!show.contains("humanize"));
    e.ok(&["kit", "drop", "net", "agents/rev"]);
    assert!(!e.ok(&["kit", "show", "net"]).contains("hex/rev"));
    assert_eq!(fs::read_to_string(e.home().join("kits/net/uses")).unwrap(), "skills/net/\n");
}

#[test]
fn kit_names_are_plugin_safe() {
    let e = Env::new();
    for bad in ["Net", "net kit", "-net", "net_kit", "kits"] {
        e.err(&["kit", "new", bad]);
    }
    e.ok(&["kit", "new", "net-2"]);
    assert!(e.ok(&["kit", "list"]).contains("net-2"));
}

#[test]
fn a_new_profile_launches_with_no_kits() {
    let e = Env::new();
    e.ok(&["new", "work"]);
    let (argv, out) = e.launched(&["work"]);
    assert!(!argv.iter().any(|a| a == "--plugin-dir"), "{argv:?}");
    assert_eq!(var(&out, "CCS_KITS"), None);
}

#[test]
fn an_attached_kit_is_built_as_a_plugin_and_loaded() {
    let e = Env::new();
    stocked(&e);
    e.ok(&["new", "work"]);
    e.ok(&["kit", "new", "net"]);
    e.ok(&["kit", "use", "net", "skills/net/", "agents/rev"]);
    e.ok(&["kit", "attach", "net", "work"]);
    assert!(e.ok(&["list"]).contains("work +net"), "{}", e.ok(&["list"]));
    let (argv, out) = e.launched(&["work", "--resume"]);
    let build = e.home().join(".build/net");
    assert_eq!(argv, args(&["--plugin-dir", build.to_str().unwrap(), "--resume"]));
    assert_eq!(var(&out, "CCS_KITS"), Some("net"));
    let manifest = fs::read_to_string(build.join(".claude-plugin/plugin.json")).unwrap();
    assert!(manifest.contains("\"name\": \"net\""), "{manifest}");
    // Skills are links to the library folder (flat), agents are copies (Claude Code does not read agent links).
    let skill = build.join("skills/find-nqe-query");
    assert!(skill.is_symlink());
    assert_eq!(fs::canonicalize(&skill).unwrap(), fs::canonicalize(e.home().join("library/skills/net/nqe/find-nqe-query")).unwrap());
    assert!(build.join("skills/inspect-edge").is_symlink());
    assert!(!build.join("skills/humanize").exists());
    let agent = build.join("agents/rev.md");
    assert!(agent.is_file() && !agent.is_symlink());
    // Nothing went into the login's config dir.
    assert!(!e.home().join("work/config/skills/find-nqe-query").exists());
}

#[test]
fn plus_kits_replace_the_defaults_and_the_build_follows_the_kit() {
    let e = Env::new();
    stocked(&e);
    e.ok(&["new", "work"]);
    for (k, entry) in [("net", "skills/net/"), ("write", "skills/humanize")] {
        e.ok(&["kit", "new", k]);
        e.ok(&["kit", "use", k, entry]);
    }
    e.ok(&["kit", "attach", "net", "work"]);
    let (argv, out) = e.launched(&["work", "+write"]);
    assert_eq!(argv, args(&["--plugin-dir", e.home().join(".build/write").to_str().unwrap()]));
    assert_eq!(var(&out, "CCS_KITS"), Some("write"));
    // Dropping from the kit drops it from the next build.
    e.launched(&["work"]);
    assert!(e.home().join(".build/net/skills/inspect-edge").exists(), "built once with net/");
    e.ok(&["kit", "drop", "net", "skills/net/"]);
    e.ok(&["kit", "use", "net", "skills/humanize"]);
    e.launched(&["work"]);
    assert!(!e.home().join(".build/net/skills/inspect-edge").exists());
    assert!(e.home().join(".build/net/skills/humanize").exists());
    // An unknown kit launches nothing.
    let err = e.err(&["work", "+nope"]);
    assert!(err.contains("nope"), "{err}");
}

#[test]
fn kit_instructions_are_joined_into_one_prompt_file() {
    let e = Env::new();
    stocked(&e);
    e.ok(&["new", "work"]);
    for k in ["net", "write"] {
        e.ok(&["kit", "new", k]);
        fs::write(e.home().join(format!("kits/{k}/CLAUDE.md")), format!("RULE-{k}\n")).unwrap();
    }
    let (argv, _) = e.launched(&["work", "+net", "+write"]);
    let i = argv.iter().position(|a| a == "--append-system-prompt-file").expect("prompt flag");
    let text = fs::read_to_string(&argv[i + 1]).unwrap();
    assert!(text.contains("RULE-net") && text.contains("RULE-write"), "{text}");
}

#[test]
fn a_removed_item_is_named_at_launch_and_the_launch_goes_on() {
    let e = Env::new();
    stocked(&e);
    e.ok(&["new", "work"]);
    e.ok(&["kit", "new", "net"]);
    e.ok(&["kit", "use", "net", "skills/humanize", "skills/inspect-edge"]);
    e.ok(&["kit", "attach", "net", "work"]);
    let out = e.ok(&["library", "rm", "skills/humanize"]);
    assert!(out.contains(".trash") && out.contains("net"), "rm names the trash and the kits that used it: {out}");
    let run = e.run(&["work"]);
    assert!(run.status.success());
    assert!(String::from_utf8_lossy(&run.stderr).contains("skills/humanize"), "{}", String::from_utf8_lossy(&run.stderr));
    assert!(e.home().join(".build/net/skills/inspect-edge").exists());
}

#[test]
fn detach_and_kit_rm() {
    let e = Env::new();
    e.ok(&["new", "work"]);
    e.ok(&["kit", "new", "net"]);
    e.ok(&["kit", "attach", "net", "work"]);
    e.ok(&["kit", "detach", "net", "work"]);
    assert!(!e.ok(&["list"]).contains("+net"));
    e.ok(&["kit", "attach", "net", "work"]);
    let out = e.ok(&["kit", "rm", "net"]);
    assert!(out.contains(".trash"), "{out}");
    assert!(!e.ok(&["list"]).contains("+net"), "a removed kit is detached everywhere");
    assert!(!e.home().join("kits/net").exists());
}

#[test]
fn library_edit_opens_the_library_copy() {
    let e = Env::new();
    stocked(&e);
    let ed = e.root.join("ed");
    fs::write(&ed, "#!/bin/sh\nfor f; do :; done\necho edited >> \"$f\"\n").unwrap();
    fs::set_permissions(&ed, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let out = e.ccs(&["library", "edit", "skills/inspect-edge"]).env("EDITOR", &ed).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(fs::read_to_string(e.home().join("library/skills/net/inspect-edge/SKILL.md")).unwrap().ends_with("edited\n"));
    let out = e.ccs(&["library", "edit", "agents/rev"]).env("EDITOR", &ed).output().unwrap();
    assert!(out.status.success());
    assert!(fs::read_to_string(e.home().join("library/agents/hex/rev.md")).unwrap().ends_with("edited\n"));
    assert!(e.ok(&["library", "show", "skills/humanize"]).contains("about humanize"));
}

// ── Organizing (ADR-2610261643 §9) ───────────────────────────────────────────

#[test]
fn moving_a_folder_rewrites_every_kit_line_under_it() {
    let e = Env::new();
    stocked(&e);
    e.ok(&["new", "work"]);
    e.ok(&["kit", "new", "net"]);
    e.ok(&["kit", "use", "net", "skills/net/", "skills/humanize"]);
    e.ok(&["kit", "new", "deep"]);
    e.ok(&["kit", "use", "deep", "skills/net/nqe/"]);
    e.ok(&["kit", "attach", "net", "work"]);
    e.ok(&["library", "mv", "skills/net/", "ops"]);
    assert_eq!(fs::read_to_string(e.home().join("kits/net/uses")).unwrap(), "skills/ops/net/\nskills/humanize\n");
    assert_eq!(fs::read_to_string(e.home().join("kits/deep/uses")).unwrap(), "skills/ops/net/nqe/\n");
    let run = e.run(&["work"]);
    assert!(run.status.success());
    assert!(!String::from_utf8_lossy(&run.stderr).contains("not in the library"), "{}", String::from_utf8_lossy(&run.stderr));
    assert!(e.home().join(".build/net/skills/find-nqe-query").exists());
}

#[test]
fn renaming_a_folder_keeps_its_place_and_its_kits() {
    let e = Env::new();
    stocked(&e);
    e.ok(&["kit", "new", "net"]);
    e.ok(&["kit", "use", "net", "skills/net/nqe/"]);
    e.ok(&["library", "rename", "skills/net/nqe/", "queries"]);
    assert!(e.home().join("library/skills/net/queries/find-nqe-query/SKILL.md").is_file());
    assert_eq!(fs::read_to_string(e.home().join("kits/net/uses")).unwrap(), "skills/net/queries/\n");
    assert!(e.err(&["library", "rename", "skills/net/queries/", "../x"]).contains("folder name"));
    e.ok(&["library", "mkdir", "skills/net/taken"]);
    e.err(&["library", "rename", "skills/net/queries/", "taken"]);
    e.err(&["library", "rename", "skills/humanize", "x"]);
}

#[test]
fn renaming_a_kit_follows_it_into_every_profile() {
    let e = Env::new();
    e.ok(&["new", "work"]);
    e.ok(&["kit", "new", "net"]);
    e.ok(&["kit", "new", "eng"]);
    e.ok(&["kit", "attach", "net", "work"]);
    e.ok(&["kit", "attach", "eng", "work"]);
    e.ok(&["kit", "rename", "net", "network"]);
    assert!(e.home().join("kits/network/uses").is_file() && !e.home().join("kits/net").exists());
    assert_eq!(fs::read_to_string(e.home().join("work/kits")).unwrap(), "network\neng\n", "same place in the order");
    e.err(&["kit", "rename", "network", "eng"]);
    e.err(&["kit", "rename", "ghost", "x"]);
    e.err(&["kit", "rename", "network", "Bad"]);
}

/// The launch's leading `--settings <json>` (its status line), which every
/// launch has; `a_launch_sets_the_ccs_status_line` in tests/kits.rs checks it.
fn without_settings(argv: Vec<String>) -> Vec<String> {
    match argv.as_slice() {
        [flag, _, rest @ ..] if flag == "--settings" => rest.to_vec(),
        _ => argv,
    }
}

#[test]
fn a_launch_sets_the_ccs_status_line_above_any_projects() {
    let e = Env::new();
    e.ok(&["new", "work"]);
    let out = e.ok(&["work", "--resume"]);
    let argv: Vec<&str> = out.lines().filter_map(|l| l.strip_prefix("ARG ")).collect();
    assert_eq!(argv[0], "--settings", "{argv:?}");
    assert!(argv[1].contains("statusline --auto") && argv[1].contains(env!("CARGO_BIN_EXE_ccs")), "{argv:?}");
    assert_eq!(argv[2..], ["--resume"]);
}
