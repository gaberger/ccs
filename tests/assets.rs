//! Gate for what items are made of (front matter, names), typed paths
//! (ADR-2610261933), and the real `App` behind the TUI's port (ADR-2610261643).
//! Every test works in its own temp directory and never touches the real ~/.claude.

use ccs::domain::{default_asset_name, front_matter_description, item_name, CcsError, Kind, ProfileName, Use};
use ccs::usecases;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static N: AtomicUsize = AtomicUsize::new(0);

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("ccs-assets-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("home/.claude")).unwrap();
        fs::create_dir_all(root.join("cwd")).unwrap();
        Sandbox { root }
    }
    fn home(&self) -> PathBuf {
        self.root.join("profiles")
    }
    fn write(&self, rel: &str, text: &str) -> PathBuf {
        let p = self.root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, text).unwrap();
        p
    }
    /// `~` is the sandbox's `home/`; relative paths start in `cwd/`.
    fn paths(&self) -> impl ccs::ports::PathInput {
        ccs::path_input(self.root.join("home"), self.root.join("cwd"))
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// An editor that records the file it was asked to open.
#[derive(Default)]
struct Recorder(std::cell::RefCell<Vec<PathBuf>>);
impl ccs::ports::Editor for Recorder {
    fn edit(&self, path: &std::path::Path) -> Result<(), CcsError> {
        self.0.borrow_mut().push(path.to_path_buf());
        Ok(())
    }
}

const SKILL: &str = "---\nname: demo\ndescription: \"Finds the edge of the network.\"\n---\nbody\n";
const AGENT: &str = "---\nname: rev\ndescription: |\n  Reviews a design\n  adversarially.\nmodel: opus\n---\nprompt\n";

#[test]
fn descriptions_are_read_plain_quoted_and_as_blocks() {
    assert_eq!(front_matter_description(SKILL).as_deref(), Some("Finds the edge of the network."));
    assert_eq!(front_matter_description(AGENT).as_deref(), Some("Reviews a design adversarially."));
    assert_eq!(front_matter_description("---\ndescription: plain words\n---\n").as_deref(), Some("plain words"));
    assert_eq!(front_matter_description("---\ndescription: >\n  folded\n  text\nnext: 1\n---\n").as_deref(), Some("folded text"));
    assert_eq!(front_matter_description("no front matter\ndescription: x\n"), None);
    assert_eq!(front_matter_description("---\nname: x\n---\ndescription: after the fence\n"), None);
}

#[test]
fn default_names_come_from_the_source_path() {
    assert_eq!(default_asset_name(Kind::Skill, "/x/my-skill").as_deref(), Some("my-skill"));
    assert_eq!(default_asset_name(Kind::Skill, "/x/my-skill/").as_deref(), Some("my-skill"));
    assert_eq!(default_asset_name(Kind::Skill, "/x/my-skill/SKILL.md").as_deref(), Some("my-skill"));
    assert_eq!(default_asset_name(Kind::Agent, "./rev.md").as_deref(), Some("rev"));
}

#[test]
fn item_names_are_one_path_safe_segment() {
    for ok in ["demo", "find-nqe-query", "v1.2_x"] {
        assert!(item_name(Kind::Skill, ok).is_ok(), "{ok}");
    }
    for bad in ["", "a/b", "..", ".hidden", "-x", "a b"] {
        assert!(item_name(Kind::Skill, bad).is_err(), "{bad}");
    }
}

#[test]
fn path_input_expands_home_and_relative_paths() {
    use ccs::ports::PathInput;
    let sb = Sandbox::new();
    let paths = sb.paths();
    assert_eq!(paths.expand("~/x"), sb.root.join("home/x"));
    assert_eq!(paths.expand("~"), sb.root.join("home"));
    assert_eq!(paths.expand("rel/y"), sb.root.join("cwd/rel/y"));
    assert_eq!(paths.expand("/abs"), PathBuf::from("/abs"));
}

#[test]
fn path_input_completes_as_typed_with_folders_ending_in_a_slash() {
    use ccs::ports::PathInput;
    let sb = Sandbox::new();
    sb.write("home/skills-a/SKILL.md", SKILL);
    sb.write("home/skills-b.md", "x");
    sb.write("home/.hidden-skill/SKILL.md", SKILL);
    sb.write("cwd/local/SKILL.md", SKILL);
    let paths = sb.paths();
    assert_eq!(paths.complete("~/sk"), ["~/skills-a/", "~/skills-b.md"]);
    assert_eq!(paths.complete("~/skills-a/"), ["~/skills-a/SKILL.md"]);
    assert_eq!(paths.complete("~/"), ["~/skills-a/", "~/skills-b.md"], "dotfiles only when asked for");
    assert_eq!(paths.complete("~/.h"), ["~/.hidden-skill/"]);
    assert_eq!(paths.complete("lo"), ["local/"], "relative to where ccs started");
    assert!(paths.complete("~/nothing").is_empty());
    assert!(paths.complete("~/skills-b.md/").is_empty(), "a file has no children");
}

/// The fake in tests/tui.rs proves the screen; this proves the real `App` behind the port.
#[test]
fn the_app_drives_the_real_library_kits_and_profiles_through_the_port() {
    use ccs::ports::Browser;
    let sb = Sandbox::new();
    sb.write("home/src/demo/SKILL.md", SKILL);
    sb.write("home/src/rev.md", AGENT);
    let (store, lib, kits) = (ccs::profile_store(sb.home()), ccs::library(&sb.home()), ccs::kit_store(&sb.home()));
    let old = ccs::legacy(&sb.home(), sb.root.join("home/.claude"));
    let (paths, editor) = (sb.paths(), Recorder::default());
    usecases::new_profile(&store, &ProfileName::parse("work").unwrap(), None).unwrap();
    let app = usecases::App { store: &store, kits: &kits, lib: &lib, legacy: &old, paths: &paths, editor: &editor, aws: &NoAws, fetch: &ccs::fetcher(), ccs_bin: "ccs".into(), claude_available: true, current: None };
    let work = ProfileName::parse("work").unwrap();

    // Library: add by typed path into a folder, make and move folders.
    assert_eq!(app.add("~/src/demo", "net").unwrap(), (Kind::Skill, "demo".to_string()));
    assert_eq!(app.add("~/src/rev.md", "").unwrap(), (Kind::Agent, "rev".to_string()));
    app.mkdir(Kind::Skill, "net/edge").unwrap();
    let (items, folders) = app.library(Kind::Skill);
    assert_eq!(items[0].path(), "net/demo");
    assert_eq!(folders, ["net", "net/edge"]);
    assert_eq!(app.read(&Use::Item(Kind::Skill, "demo".into())).unwrap(), SKILL);

    // Kits: make, pick, attach.
    app.new_kit("net").unwrap();
    assert!(app.new_kit("Net").is_err());
    app.set_use("net", &Use::Folder(Kind::Skill, "net".into()), true).unwrap();
    assert!(app.set_use("net", &Use::Item(Kind::Skill, "ghost".into()), true).is_err(), "only what the library has");
    assert_eq!(app.kit_uses("net").unwrap(), [Use::Folder(Kind::Skill, "net".into())]);
    app.attach("net", &work, true).unwrap();
    assert_eq!(app.profiles()[0].kits, ["net"]);
    assert_eq!(app.overview(&work).unwrap().kits, ["net"]);
    app.set_use("net", &Use::Folder(Kind::Skill, "net".into()), false).unwrap();
    assert!(app.kit_uses("net").unwrap().is_empty());

    // Edit hands the editor the library file, or the kit's CLAUDE.md.
    app.edit(&Use::Item(Kind::Skill, "demo".into())).unwrap();
    app.edit_kit("net").unwrap();
    let opened = editor.0.borrow().clone();
    assert_eq!(opened[0], sb.home().join("library/skills/net/demo/SKILL.md"));
    assert_eq!(opened[1], sb.home().join("kits/net/CLAUDE.md"));

    // Move, delete; deleting a kit detaches it.
    app.move_entry(&Use::Item(Kind::Skill, "demo".into()), "").unwrap();
    assert!(sb.home().join("library/skills/demo/SKILL.md").is_file());
    let removed = app.remove(&Use::Item(Kind::Agent, "rev".into())).unwrap();
    assert!(removed.trashed.starts_with(sb.home().join(".trash")));
    app.remove_kit("net").unwrap();
    assert!(app.profiles()[0].kits.is_empty());
    assert_eq!(app.complete_path("~/src/d"), ["~/src/demo/"]);
}

/// One AWS CLI profile, `dev`, in us-west-2.
struct NoAws;
impl ccs::ports::AwsProfiles for NoAws {
    fn names(&self) -> Result<Vec<String>, CcsError> {
        Ok(vec!["dev".into()])
    }
    fn region(&self, _: &str) -> Option<String> {
        Some("us-west-2".into())
    }
}

/// Everything the CLI does to profiles, through the TUI's port (ADR-2610261643 §12).
#[test]
fn the_app_makes_copies_edits_and_imports_profiles_through_the_port() {
    use ccs::ports::Browser;
    let sb = Sandbox::new();
    sb.write("home/.claude/skills/demo/SKILL.md", SKILL);
    let (store, lib, kits) = (ccs::profile_store(sb.home()), ccs::library(&sb.home()), ccs::kit_store(&sb.home()));
    let old = ccs::legacy(&sb.home(), sb.root.join("home/.claude"));
    let (paths, editor) = (sb.paths(), Recorder::default());
    let app = usecases::App { store: &store, kits: &kits, lib: &lib, legacy: &old, paths: &paths, editor: &editor, aws: &NoAws, fetch: &ccs::fetcher(), ccs_bin: "/bin/ccs".into(), claude_available: true, current: None };
    let work = ProfileName::parse("work").unwrap();

    app.new_profile("work", None).unwrap();
    let settings = std::fs::read_to_string(sb.home().join("work/config/settings.json")).unwrap();
    assert!(settings.contains("'/bin/ccs' statusline"), "a new profile gets the status line, as ccs new does: {settings}");
    assert!(app.new_profile("work", None).is_err(), "never overwrites");
    assert!(app.new_profile("bad name", None).is_err());
    app.new_kit("net").unwrap();
    app.attach("net", &work, true).unwrap();
    app.new_profile("copy", Some(&work)).unwrap();
    assert_eq!(app.profiles().iter().find(|p| p.name.as_str() == "copy").unwrap().kits, ["net"]);

    app.edit_profile(&work).unwrap();
    assert_eq!(editor.0.borrow()[0], sb.home().join("work/profile.env"));
    assert!(app.edit_profile(&ProfileName::parse("ghost").unwrap()).is_err());

    assert_eq!(app.aws_profiles().unwrap()[0].imported_as, None);
    let out = app.import_aws(&["dev".into()], false).unwrap();
    assert_eq!(out[0].to_string(), "✓ dev (bedrock, us-west-2)");
    assert_eq!(app.aws_profiles().unwrap()[0].imported_as.as_deref(), Some("dev"));
    assert!(app.import_aws(&[], true).unwrap()[0].to_string().starts_with("- dev:"), "--all skips what exists");

    assert!(!app.needs_migrate());
    assert!(app.migrate().unwrap().is_empty());
    app.import_claude().unwrap();
    assert!(sb.home().join("library/skills/imported/demo/SKILL.md").is_file() || app.library(Kind::Skill).0.iter().any(|i| i.name == "demo"));
    assert!(app.kits().contains(&"imported".to_string()));
}

/// A kit whose `uses` file cannot be read stops the remove: read as "uses
/// nothing", it would be left out of the report with a dangling line.
#[test]
fn removing_an_item_refuses_when_a_kit_cannot_be_read() {
    let sb = Sandbox::new();
    sb.write("home/src/rev.md", AGENT);
    let (lib, kits) = (ccs::library(&sb.home()), ccs::kit_store(&sb.home()));
    usecases::library_add_source(&lib, &ccs::fetcher(), "rev.md", &sb.root.join("home/src/rev.md"), "").unwrap();
    usecases::kit_new(&kits, &lib, "net").unwrap();
    sb.write("profiles/kits/net/uses", "agents/rev\nnot a line ccs can read !!\n");

    let rev = Use::Item(Kind::Agent, "rev".into());
    assert!(usecases::library_remove(&lib, &kits, &rev).is_err(), "an unreadable kit is an error, not a kit that uses nothing");
    assert!(sb.home().join("library/agents/rev.md").is_file(), "nothing is trashed");
}
