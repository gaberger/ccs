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
    LibItem { kind, name: name.into(), folder: folder.into(), description: None, shown_name: None, source: PathBuf::from(format!("/lib/{folder}/{name}")) }
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
fn a_kit_drop_naming_an_entry_twice_drops_it_once_and_still_refuses_strangers() {
    let e = Env::new();
    stocked(&e);
    e.ok(&["kit", "new", "net"]);
    e.ok(&["kit", "use", "net", "skills/net/", "agents/rev"]);
    // kit use takes a repeated entry quietly; kit drop must too.
    e.ok(&["kit", "drop", "net", "agents/rev", "agents/rev"]);
    assert_eq!(fs::read_to_string(e.home().join("kits/net/uses")).unwrap(), "skills/net/\n");
    // An entry the kit never had is still refused, and nothing is dropped.
    let err = e.err(&["kit", "drop", "net", "skills/net/", "agents/rev"]);
    assert!(err.contains("agents/rev in kit net"), "{err}");
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

/// A hand-made library can hold two skills, or two agents, of one name in
/// different folders. The kit builds the first of each, names the second, and
/// the launch goes on.
#[test]
fn two_items_of_one_name_in_a_kit_keep_the_first_and_the_launch_goes_on() {
    let e = Env::new();
    let lib = e.home().join("library");
    for (rel, text) in [
        ("skills/net/foo/SKILL.md", "---\nname: foo\n---\nNET\n"),
        ("skills/web/foo/SKILL.md", "---\nname: foo\n---\nWEB\n"),
        ("agents/a/x.md", "---\nname: x\n---\nAGENT-A\n"),
        ("agents/b/x.md", "---\nname: x\n---\nAGENT-B\n"),
    ] {
        fs::create_dir_all(lib.join(rel).parent().unwrap()).unwrap();
        fs::write(lib.join(rel), text).unwrap();
    }
    e.ok(&["new", "work"]);
    e.ok(&["kit", "new", "all"]);
    fs::write(e.home().join("kits/all/uses"), "skills/net/\nskills/web/\nagents/a/\nagents/b/\n").unwrap();
    e.ok(&["kit", "attach", "all", "work"]);
    let run = e.run(&["work"]);
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(run.status.success(), "{stderr}");
    assert!(stderr.contains("skills/web/foo") && stderr.contains("skills/net/foo"), "{stderr}");
    assert!(stderr.contains("agents/b/x") && stderr.contains("agents/a/x"), "{stderr}");
    let build = e.home().join(".build/all");
    assert_eq!(fs::canonicalize(build.join("skills/foo")).unwrap(), fs::canonicalize(lib.join("skills/net/foo")).unwrap());
    assert!(fs::read_to_string(build.join("agents/x.md")).unwrap().contains("AGENT-A"));
}

#[test]
fn attaching_to_an_unreadable_kits_file_fails_and_keeps_the_file() {
    let e = Env::new();
    e.ok(&["new", "work"]);
    e.ok(&["kit", "new", "net"]);
    e.ok(&["kit", "new", "eng"]);
    // A byte that is not UTF-8: the list cannot be read, so it must not be replaced.
    let file = e.home().join("work/kits");
    let before = b"net\n\xff\n".to_vec();
    fs::write(&file, &before).unwrap();
    for verb in ["attach", "detach"] {
        let err = e.err(&["kit", verb, "eng", "work"]);
        assert!(err.contains("kits"), "{verb}: {err}");
        assert_eq!(fs::read(&file).unwrap(), before, "{verb} must not overwrite a kits file it could not read");
    }
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
fn a_kit_rm_that_cannot_reach_the_trash_changes_nothing() {
    let e = Env::new();
    e.ok(&["new", "alpha"]);
    e.ok(&["new", "beta"]);
    e.ok(&["kit", "new", "eng"]);
    e.ok(&["kit", "new", "net"]);
    e.ok(&["kit", "attach", "eng", "alpha"]);
    e.ok(&["kit", "attach", "net", "alpha"]);
    e.ok(&["kit", "attach", "net", "beta"]);
    // A file where the trash folder goes: the kit cannot be moved there.
    let trash = e.home().join(".trash");
    fs::write(&trash, "").unwrap();
    e.err(&["kit", "rm", "net"]);
    assert!(e.home().join("kits/net").is_dir(), "the kit stays");
    assert_eq!(fs::read_to_string(e.home().join("alpha/kits")).unwrap(), "eng\nnet\n", "every profile keeps the kit, in its place");
    assert_eq!(fs::read_to_string(e.home().join("beta/kits")).unwrap(), "net\n");
    fs::remove_file(&trash).unwrap();
    e.ok(&["kit", "rm", "net"]);
    assert_eq!(fs::read_to_string(e.home().join("alpha/kits")).unwrap(), "eng\n", "a second try works");
}

#[test]
fn a_kit_removed_twice_in_one_second_goes_to_the_trash_both_times() {
    use ccs::ports::KitStore;
    let e = Env::new();
    let kits = ccs::kit_store(&e.home());
    let net = ccs::ports::Kit::parse("net").unwrap();
    // An earlier `kit rm net` left its kit in the trash. Fill the slot for this
    // second and the next, so the remove below collides wherever the clock is.
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    for s in [secs, secs + 1] {
        let old = e.home().join(format!(".trash/{s}-kit-net"));
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("uses"), "skills/old\n").unwrap();
    }
    kits.create(&net).unwrap();
    let to = kits.remove(&net).expect("a second remove in the same second goes to the trash");
    assert!(!e.home().join("kits/net").exists(), "the kit leaves the kits folder");
    assert_eq!(fs::read_to_string(to.join("uses")).unwrap(), "", "the trash holds the new kit, beside the old one");
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
fn an_unreadable_kit_stops_a_folder_rename_before_the_folder_moves() {
    let e = Env::new();
    stocked(&e);
    e.ok(&["kit", "new", "a-broken"]);
    e.ok(&["kit", "new", "net"]);
    e.ok(&["kit", "use", "net", "skills/net/nqe/"]);
    fs::write(e.home().join("kits/a-broken/uses"), "not-an-entry\n").unwrap();
    e.err(&["library", "rename", "skills/net/nqe/", "queries"]);
    e.err(&["library", "mv", "skills/net/nqe/", "ops"]);
    // Nothing moved, so every kit line still points at a folder that exists.
    assert!(e.home().join("library/skills/net/nqe/find-nqe-query/SKILL.md").is_file());
    assert_eq!(fs::read_to_string(e.home().join("kits/net/uses")).unwrap(), "skills/net/nqe/\n");
    // Fix the broken kit and the same rename goes through.
    fs::write(e.home().join("kits/a-broken/uses"), "").unwrap();
    e.ok(&["library", "rename", "skills/net/nqe/", "queries"]);
    assert_eq!(fs::read_to_string(e.home().join("kits/net/uses")).unwrap(), "skills/net/queries/\n");
}

#[test]
fn a_kit_that_cannot_be_saved_puts_the_folder_and_every_kit_back() {
    use std::os::unix::fs::PermissionsExt;
    let e = Env::new();
    stocked(&e);
    // `a-net` saves; `z-net` is readable but no save can be made beside its file.
    e.ok(&["kit", "new", "a-net"]);
    e.ok(&["kit", "use", "a-net", "skills/net/nqe/"]);
    e.ok(&["kit", "new", "z-net"]);
    e.ok(&["kit", "use", "z-net", "skills/net/nqe/"]);
    let locked = e.home().join("kits/z-net");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
    let renamed = e.err(&["library", "rename", "skills/net/nqe/", "queries"]);
    let moved = e.err(&["library", "mv", "skills/net/nqe/", "ops"]);
    let torn = (
        e.home().join("library/skills/net/nqe/find-nqe-query/SKILL.md").is_file(),
        fs::read_to_string(e.home().join("kits/a-net/uses")).unwrap(),
        fs::read_to_string(locked.join("uses")).unwrap(),
    );
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!renamed.is_empty() && !moved.is_empty());
    assert_eq!(torn, (true, "skills/net/nqe/\n".into(), "skills/net/nqe/\n".into()), "the folder and both kits are where they were");
    // Unlock the kit and the same rename goes through for both.
    e.ok(&["library", "rename", "skills/net/nqe/", "queries"]);
    assert_eq!(fs::read_to_string(e.home().join("kits/a-net/uses")).unwrap(), "skills/net/queries/\n");
    assert_eq!(fs::read_to_string(locked.join("uses")).unwrap(), "skills/net/queries/\n");
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

#[test]
fn a_kit_rename_that_cannot_rewrite_a_profile_changes_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let e = Env::new();
    e.ok(&["new", "alpha"]);
    e.ok(&["new", "beta"]);
    e.ok(&["kit", "new", "net"]);
    e.ok(&["kit", "attach", "net", "alpha"]);
    e.ok(&["kit", "attach", "net", "beta"]);
    let locked = e.home().join("beta/kits");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o444)).unwrap();
    let out = e.run(&["kit", "rename", "net", "network"]);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(!out.status.success(), "a profile that cannot be rewritten fails the rename");
    assert!(e.home().join("kits/net").is_dir() && !e.home().join("kits/network").exists(), "the kit keeps its name");
    assert_eq!(fs::read_to_string(e.home().join("alpha/kits")).unwrap(), "net\n", "a profile already rewritten is put back");
    assert_eq!(fs::read_to_string(&locked).unwrap(), "net\n");
    e.ok(&["kit", "rename", "net", "network"]);
    assert_eq!(fs::read_to_string(e.home().join("beta/kits")).unwrap(), "network\n", "a second try works");
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

// ── ADR-2610281305: safe saves (G5) and kits that cannot take a plugin's name (G8) ──

/// A kit written by hand, with comments and a blank line, saved through the port.
fn hand_written_kit(e: &Env) -> (PathBuf, impl ccs::ports::KitStore) {
    let kits = ccs::kit_store(&e.home());
    let dir = e.home().join("kits/research");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("uses"), "# picks for research\nskills/humanize\n\n# plugins\nplugins/egm\n").unwrap();
    (dir, kits)
}

#[test]
fn a_save_keeps_comments_and_blank_lines_where_they_were() {
    use ccs::ports::KitStore as _;
    let e = Env::new();
    let (dir, kits) = hand_written_kit(&e);
    let k = ccs::domain::KitName::parse("research").unwrap();
    let mut uses = kits.uses(&k).unwrap();
    uses.retain(|u| u.line() != "plugins/egm");
    uses.push(Use::Item(Kind::Agent, "rev".into()));
    kits.set_uses(&k, &uses).unwrap();
    assert_eq!(
        fs::read_to_string(dir.join("uses")).unwrap(),
        "# picks for research\nskills/humanize\n\n# plugins\nagents/rev\n",
        "the dropped line goes, the comments and the blank line stay, the new line is added at the end"
    );
    let left: Vec<_> = fs::read_dir(&dir).unwrap().flatten().map(|f| f.file_name()).collect();
    assert_eq!(left, ["uses"], "no temporary file is left behind");
}

#[test]
fn a_new_pick_goes_at_the_end_when_a_middle_line_is_dropped() {
    use ccs::ports::KitStore as _;
    let e = Env::new();
    let kits = ccs::kit_store(&e.home());
    let dir = e.home().join("kits/research");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("uses"), "skills/a\nskills/b\nskills/c\n").unwrap();
    let k = ccs::domain::KitName::parse("research").unwrap();
    let uses = [Use::Item(Kind::Skill, "a".into()), Use::Item(Kind::Skill, "c".into()), Use::Item(Kind::Agent, "d".into())];
    kits.set_uses(&k, &uses).unwrap();
    assert_eq!(
        fs::read_to_string(dir.join("uses")).unwrap(),
        "skills/a\nskills/c\nagents/d\n",
        "ADR-2610281305 R3: the new pick goes at the end, not into the dropped line's place"
    );
    let renamed = [Use::Item(Kind::Skill, "a".into()), Use::Item(Kind::Skill, "c2".into()), Use::Item(Kind::Agent, "d".into())];
    kits.set_uses(&k, &renamed).unwrap();
    assert_eq!(
        fs::read_to_string(dir.join("uses")).unwrap(),
        "skills/a\nskills/c2\nagents/d\n",
        "a renamed middle line still takes the place of the one it replaced"
    );
}

#[test]
fn a_save_that_cannot_be_made_safely_changes_nothing() {
    use ccs::ports::KitStore as _;
    use std::os::unix::fs::PermissionsExt;
    let e = Env::new();
    let (dir, kits) = hand_written_kit(&e);
    let before = fs::read_to_string(dir.join("uses")).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    let k = ccs::domain::KitName::parse("research").unwrap();
    let saved = kits.set_uses(&k, &[Use::Item(Kind::Skill, "other".into())]);
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(saved.is_err(), "no temporary file can be made beside it, so the save is refused");
    assert_eq!(fs::read_to_string(dir.join("uses")).unwrap(), before, "the old file is whole");
}

#[test]
fn a_uses_file_that_cannot_be_read_is_an_error_not_an_empty_kit() {
    use ccs::ports::KitStore as _;
    let e = Env::new();
    let kits = ccs::kit_store(&e.home());
    let dir = e.home().join("kits/research");
    fs::create_dir_all(&dir).unwrap();
    let before: &[u8] = b"# picks\nskills/humanize\n# \xff not utf-8\n";
    fs::write(dir.join("uses"), before).unwrap();
    let k = ccs::domain::KitName::parse("research").unwrap();
    assert!(kits.uses(&k).is_err(), "a read error other than a missing file is reported");
    let saved = kits.set_uses(&k, &[Use::Item(Kind::Skill, "other".into())]);
    assert!(saved.is_err(), "a save over a file it cannot read is refused");
    assert_eq!(fs::read(dir.join("uses")).unwrap(), before, "the comments and picks are still there");
}

#[test]
fn a_kit_cannot_take_the_name_of_a_plugin_in_the_library() {
    let e = Env::new();
    let manifest = e.home().join("library/plugins/egm/.claude-plugin/plugin.json");
    fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    fs::write(&manifest, "{\"name\": \"egm\"}\n").unwrap();
    let err = e.err(&["kit", "new", "egm"]);
    assert!(err.contains("egm is a plugin in the library"), "{err}");
    e.ok(&["kit", "new", "research"]);
    let err = e.err(&["kit", "rename", "research", "egm"]);
    assert!(err.contains("egm is a plugin in the library"), "{err}");
    assert!(e.home().join("kits/research").is_dir(), "the refused rename changed nothing");
}

#[test]
fn a_second_build_of_a_kit_never_leaves_a_running_session_without_its_plugin() {
    use ccs::adapters::secondary::fs_kits::FsKitStore;
    use ccs::domain::KitName;
    use ccs::ports::KitStore;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    let e = Env::new();
    let home = e.home();
    let store = FsKitStore::new(home.join("kits"), home.join(".build"), home.join(".trash"));
    let kit = KitName::parse("net").unwrap();
    store.create(&kit).unwrap();
    fs::write(home.join("kits/net/.mcp.json"), "{}\n").unwrap();
    let agent = e.root.join("src/rev.md");
    fs::create_dir_all(e.root.join("src/skill")).unwrap();
    fs::write(&agent, "---\nname: rev\n---\n").unwrap();
    let items = vec![
        LibItem { source: e.root.join("src/skill"), ..lib(Kind::Skill, "", "skill") },
        LibItem { source: agent, ..lib(Kind::Agent, "", "rev") },
    ];
    // Terminal 1's session is running on this build.
    let dir = store.build(&kit, &items).unwrap();
    let wanted = [".claude-plugin/plugin.json", "skills/skill", "agents/rev.md", ".mcp.json"].map(|p| dir.join(p));
    // It keeps reading its plugin folder while terminal 2 launches the same kit, again and again.
    let stop = Arc::new(AtomicBool::new(false));
    let reader = {
        let stop = stop.clone();
        std::thread::spawn(move || {
            let mut missing = Vec::new();
            while !stop.load(Ordering::SeqCst) {
                missing.extend(wanted.iter().filter(|p| !p.exists()).cloned());
            }
            missing
        })
    };
    for _ in 0..300 {
        assert_eq!(store.build(&kit, &items).unwrap(), dir);
    }
    stop.store(true, Ordering::SeqCst);
    let missing = reader.join().unwrap();
    assert!(missing.is_empty(), "the running session lost {} reads, first {:?}", missing.len(), missing.first());
    // Each build leaves only the kit's folder behind.
    let left: Vec<_> = fs::read_dir(home.join(".build")).unwrap().map(|d| d.unwrap().file_name()).collect();
    assert_eq!(left, ["net"]);
}

#[test]
fn two_launches_that_build_the_same_kit_at_once_both_succeed_with_one_whole_build() {
    use ccs::adapters::secondary::fs_kits::FsKitStore;
    use ccs::domain::KitName;
    use ccs::ports::KitStore;
    use std::sync::{Arc, Barrier};
    let e = Env::new();
    let home = e.home();
    let store = Arc::new(FsKitStore::new(home.join("kits"), home.join(".build"), home.join(".trash")));
    let kit = KitName::parse("net").unwrap();
    store.create(&kit).unwrap();
    let names: Vec<String> = (0..20).map(|n| format!("skill-{n}")).collect();
    let items: Vec<LibItem> = names
        .iter()
        .map(|n| {
            fs::create_dir_all(e.root.join("src").join(n)).unwrap();
            LibItem { source: e.root.join("src").join(n), ..lib(Kind::Skill, "", n) }
        })
        .collect();
    let items = Arc::new(items);
    // Eight launches start the same kit at the same moment, again and again.
    let gate = Arc::new(Barrier::new(8));
    let launches: Vec<_> = (0..8)
        .map(|_| {
            let (store, kit, items, gate) = (store.clone(), kit.clone(), items.clone(), gate.clone());
            std::thread::spawn(move || {
                (0..50)
                    .map(|_| {
                        gate.wait();
                        store.build(&kit, &items)
                    })
                    .filter_map(Result::err)
                    .map(|err| format!("{err:?}"))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let failed: Vec<String> = launches.into_iter().flat_map(|t| t.join().unwrap()).collect();
    assert!(failed.is_empty(), "{} launches failed, first: {:?}", failed.len(), failed.first());
    let mut built: Vec<String> =
        fs::read_dir(home.join(".build/net/skills")).unwrap().map(|d| d.unwrap().file_name().to_string_lossy().into_owned()).collect();
    built.sort();
    let mut wanted = names.clone();
    wanted.sort();
    assert_eq!(built, wanted, "the build holds exactly one kit's worth of skills");
    let left: Vec<_> = fs::read_dir(home.join(".build")).unwrap().map(|d| d.unwrap().file_name()).collect();
    assert_eq!(left, ["net"], "no staging folder is left behind");
}

#[test]
fn a_second_launch_never_truncates_the_prompt_a_first_session_is_reading() {
    use ccs::adapters::secondary::fs_kits::FsKitStore;
    use ccs::domain::KitName;
    use ccs::ports::KitStore;
    use std::io::Read;
    let e = Env::new();
    let home = e.home();
    let store = FsKitStore::new(home.join("kits"), home.join(".build"), home.join(".trash"));
    let kits = [KitName::parse("net").unwrap(), KitName::parse("sec").unwrap()];
    let first = "first launch's instructions\n".repeat(200);
    let path = store.write_prompt(&kits, &first).unwrap();
    // The first session has opened its prompt and not yet read it.
    let mut reading = fs::File::open(&path).unwrap();
    store.write_prompt(&kits, "second\n").unwrap();
    let mut got = String::new();
    reading.read_to_string(&mut got).unwrap();
    assert_eq!(got, first, "the first session must read its whole prompt");
    assert_eq!(fs::read_to_string(&path).unwrap(), "second\n");
    let left: Vec<_> = fs::read_dir(home.join(".build")).unwrap().map(|d| d.unwrap().file_name()).collect();
    assert_eq!(left, ["net+sec.md"], "no staging file is left behind");
}

#[test]
fn two_processes_that_save_one_kit_at_once_keep_each_others_picks() {
    use ccs::adapters::secondary::fs_kits::FsKitStore;
    use ccs::domain::KitName;
    use ccs::ports::KitStore;
    let e = Env::new();
    let home = e.home();
    // Two stores are two ccs processes: a TUI and a CLI command.
    let open = || FsKitStore::new(home.join("kits"), home.join(".build"), home.join(".trash"));
    let (tui, cli) = (open(), open());
    let kit = KitName::parse("net").unwrap();
    tui.create(&kit).unwrap();
    let keep = Use::Item(Kind::Skill, "keep".into());
    let old = Use::Item(Kind::Skill, "old".into());
    tui.set_uses(&kit, &[keep.clone(), old.clone()]).unwrap();
    // Both read the same file before either saves.
    let mut a = tui.uses(&kit).unwrap();
    let mut b = cli.uses(&kit).unwrap();
    let added = Use::Item(Kind::Agent, "rev".into());
    a.push(added.clone());
    tui.set_uses(&kit, &a).unwrap();
    b.retain(|u| *u != old);
    let also = Use::Folder(Kind::Skill, "nqe".into());
    b.push(also.clone());
    cli.set_uses(&kit, &b).unwrap();
    // R3 may put `also` where `old` was; what matters is which picks survive.
    let got = open().uses(&kit).unwrap();
    assert_eq!(got.len(), 3, "{got:?}");
    for u in [&keep, &added, &also] {
        assert!(got.contains(u), "{} is lost: {got:?}", u.line());
    }
    assert!(!got.contains(&old), "the drop did not hold: {got:?}");
}

#[test]
fn two_agents_with_one_name_fail_the_build_and_name_both_sources() {
    use ccs::adapters::secondary::fs_kits::FsKitStore;
    use ccs::domain::KitName;
    use ccs::ports::KitStore;
    let e = Env::new();
    let home = e.home();
    let store = FsKitStore::new(home.join("kits"), home.join(".build"), home.join(".trash"));
    let kit = KitName::parse("net").unwrap();
    store.create(&kit).unwrap();
    let (a, b) = (e.root.join("src/a/x.md"), e.root.join("src/b/x.md"));
    for p in [&a, &b] {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, format!("---\nname: x\n---\n{}\n", p.display())).unwrap();
    }
    let items = vec![
        LibItem { source: a.clone(), ..lib(Kind::Agent, "a", "x") },
        LibItem { source: b.clone(), ..lib(Kind::Agent, "b", "x") },
    ];
    let err = store.build(&kit, &items).expect_err("a second agents/x.md must not replace the first").to_string();
    for p in [&a, &b] {
        assert!(err.contains(&p.display().to_string()), "the error does not name {}: {err}", p.display());
    }
    assert!(!home.join(".build/net").exists(), "a failed build left a kit behind");
}
