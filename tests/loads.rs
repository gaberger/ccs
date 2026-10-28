//! Gate G4 for ADR-2610281305: what a profile loads has one source. The launch
//! planned without building equals the launch `prepare_launch` makes, and
//! planning writes nothing.

use ccs::domain::{Finding, ProfileName};
use ccs::ports::ProfileStore as _;
use ccs::usecases;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static N: AtomicUsize = AtomicUsize::new(0);

struct Home(PathBuf);

impl Home {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("ccs-loads-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Home(root)
    }
    fn write(&self, rel: &str, text: &str) {
        let p = self.0.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn name(s: &str) -> ProfileName {
    ProfileName::parse(s).unwrap()
}

/// A folder of two skills, a loose skill, an agent and a plugin; two kits that
/// both pick the plugin, one with instructions and a line the library lacks;
/// one profile that starts with both kits.
fn stocked() -> Home {
    let h = Home::new();
    h.write("library/skills/writing/humanize/SKILL.md", "---\nname: humanize\ndescription: Rewrite text.\n---\nbody\n");
    h.write("library/skills/writing/report/SKILL.md", "---\nname: report\ndescription: Write a report.\n---\n");
    h.write("library/skills/net/inspect/SKILL.md", "---\nname: inspect\ndescription: Inspect.\n---\n");
    h.write("library/agents/dev/reviewer.md", "---\nname: reviewer\ndescription: Reviews.\n---\n");
    h.write("library/plugins/egm/.claude-plugin/plugin.json", "{\"name\": \"egm\", \"description\": \"EGM\"}\n");
    h.write("library/plugins/egm/skills/egm_draft/SKILL.md", "---\nname: egm_draft\ndescription: Draft.\n---\n");
    h.write("kits/research/uses", "skills/writing/\nplugins/egm\nagents/reviewer\nskills/gone\n");
    h.write("kits/research/CLAUDE.md", "Be brief.\n");
    h.write("kits/net/uses", "skills/inspect\nplugins/egm\n");
    let store = ccs::profile_store(h.0.clone());
    usecases::new_profile(&store, &name("work"), None).unwrap();
    store.set_kits(&name("work"), &["research".into(), "net".into()]).unwrap();
    h
}

/// Every path under `dir`, with what it is and its inode: a rebuild that writes
/// the same bytes still changes the inodes, so it cannot pass as "unchanged".
fn tree(dir: &Path) -> BTreeMap<PathBuf, String> {
    fn walk(base: &Path, d: &Path, out: &mut BTreeMap<PathBuf, String>) {
        let Ok(entries) = fs::read_dir(d) else { return };
        for e in entries.flatten() {
            let p = e.path();
            let rel = p.strip_prefix(base).unwrap().to_path_buf();
            let md = fs::symlink_metadata(&p).unwrap();
            let what = if md.file_type().is_symlink() {
                format!("link {} -> {}", md.ino(), fs::read_link(&p).unwrap().display())
            } else if md.is_dir() {
                walk(base, &p, out);
                format!("dir {}", md.ino())
            } else {
                format!("file {} {}", md.ino(), fs::read_to_string(&p).unwrap_or_default())
            };
            out.insert(rel, what);
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

#[test]
fn a_planned_launch_is_the_launch_that_gets_built() {
    let h = stocked();
    let (store, kits, lib) = (ccs::profile_store(h.0.clone()), ccs::kit_store(&h.0), ccs::library(&h.0));
    for chosen in [None, Some(vec!["net".to_string()])] {
        let args = vec!["--resume".to_string()];
        let planned = usecases::planned_launch(&store, &kits, &lib, &name("work"), chosen.clone(), args.clone(), "/bin/ccs").unwrap();
        let built = usecases::prepare_launch(&store, &kits, &lib, &name("work"), chosen.clone(), args, "/bin/ccs").unwrap();
        assert_eq!(planned, built, "chosen kits: {chosen:?}");
    }
    let (plan, warnings) = usecases::planned_launch(&store, &kits, &lib, &name("work"), None, vec![], "/bin/ccs").unwrap();
    assert_eq!(plan.args.iter().filter(|a| *a == "--plugin-dir").count(), 3, "two kits, and egm once: {:?}", plan.args);
    assert!(plan.args.iter().any(|a| a == "--append-system-prompt-file"), "research has instructions: {:?}", plan.args);
    assert_eq!(warnings, ["kit research: skills/gone is not in the library; left out"]);
}

/// Equal plans are not enough: the paths the launch names must be the ones the
/// build wrote, or Claude Code is pointed at nothing.
#[test]
fn the_launch_names_what_the_build_wrote() {
    let h = stocked();
    let (store, kits, lib) = (ccs::profile_store(h.0.clone()), ccs::kit_store(&h.0), ccs::library(&h.0));
    let (plan, _) = usecases::prepare_launch(&store, &kits, &lib, &name("work"), None, vec![], "/bin/ccs").unwrap();
    let after = |flag: &str| -> Vec<PathBuf> { plan.args.windows(2).filter(|w| w[0] == flag).map(|w| PathBuf::from(&w[1])).collect() };
    let dirs = after("--plugin-dir");
    assert_eq!(dirs.len(), 3, "{:?}", plan.args);
    for d in &dirs {
        assert!(d.join(".claude-plugin/plugin.json").is_file(), "{} is not a built plugin", d.display());
    }
    let prompts = after("--append-system-prompt-file");
    assert_eq!(prompts.len(), 1);
    assert_eq!(fs::read_to_string(&prompts[0]).unwrap(), "<!-- kit research -->\nBe brief.\n");
}

#[test]
fn planning_writes_nothing() {
    let h = stocked();
    let (store, kits, lib) = (ccs::profile_store(h.0.clone()), ccs::kit_store(&h.0), ccs::library(&h.0));
    usecases::load_plan(&store, &kits, &lib, &name("work"), None).unwrap();
    usecases::planned_launch(&store, &kits, &lib, &name("work"), None, vec![], "/bin/ccs").unwrap();
    assert!(!h.0.join(".build").exists(), "planning on a new home builds nothing");

    usecases::prepare_launch(&store, &kits, &lib, &name("work"), None, vec![], "/bin/ccs").unwrap();
    let before = tree(&h.0.join(".build"));
    assert!(before.len() > 3, "the launch built the kits: {before:?}");
    usecases::load_plan(&store, &kits, &lib, &name("work"), None).unwrap();
    usecases::planned_launch(&store, &kits, &lib, &name("work"), None, vec![], "/bin/ccs").unwrap();
    assert_eq!(tree(&h.0.join(".build")), before, "planning leaves .build as it was");
}

#[test]
fn each_line_says_what_it_gives_and_a_plugin_loads_once() {
    let h = stocked();
    let (store, kits, lib) = (ccs::profile_store(h.0.clone()), ccs::kit_store(&h.0), ccs::library(&h.0));
    let plan = usecases::load_plan(&store, &kits, &lib, &name("work"), None).unwrap();
    let names: Vec<&str> = plan.kits.iter().map(|k| k.kit.as_str()).collect();
    assert_eq!(names, ["research", "net"], "launch order");
    let gives: Vec<(String, Vec<String>, bool)> =
        plan.kits[0].lines.iter().map(|g| (g.line.line(), g.items.iter().map(|i| i.name.clone()).collect(), g.missing)).collect();
    assert_eq!(
        gives,
        [
            ("skills/writing/".to_string(), vec!["humanize".to_string(), "report".to_string()], false),
            ("plugins/egm".to_string(), vec!["egm".to_string()], false),
            ("agents/reviewer".to_string(), vec!["reviewer".to_string()], false),
            ("skills/gone".to_string(), vec![], true),
        ]
    );
    let plugins: Vec<&str> = plan.plugins.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(plugins, ["egm"], "two kits pick egm; it loads once");
}

// ── `ccs loads`, and two agents with one name ───────────────────────────────

fn ccs(h: &Home, a: &[&str], profile: Option<&str>) -> (bool, String) {
    let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_ccs"));
    c.args(a).env("HOME", h.0.join("user")).env("CCS_HOME", &h.0).env_remove("CCS_PROFILE");
    if let Some(p) = profile {
        c.env("CCS_PROFILE", p);
    }
    let out = c.output().unwrap();
    (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr))
}

#[test]
fn ccs_loads_prints_each_kit_line_and_item_and_builds_nothing() {
    let h = stocked();
    let (ok, out) = ccs(&h, &["loads", "work"], None);
    assert!(ok, "{out}");
    for want in [
        "work loads 2 kits: research net",
        "kit research",
        "skills/writing/",
        "humanize",
        "report",
        "plugins/egm",
        "agents/reviewer",
        "skills/gone",
        "not in the library",
        "kit net",
        "skills/inspect",
        "folder · 2 skills",
        "instructions (CLAUDE.md)",
        "3 skills · 1 agent · 1 plugin",
    ] {
        assert!(out.contains(want), "{want:?} missing from:\n{out}");
    }
    assert!(!h.0.join(".build").exists(), "looking builds nothing");
    h.write("kits/empty/uses", "");
    let (ok, empty) = ccs(&h, &["kit", "show", "empty"], None);
    assert!(ok && empty.contains("(picks nothing)"), "an empty kit says so: {empty}");
    let (ok, resolved) = ccs(&h, &["loads"], Some("work"));
    assert!(ok, "{resolved}");
    assert_eq!(resolved, out, "with no name, the profile a launch would use");
}

/// The library's real case: two claude-flow agents both declare `name: pr-manager`.
fn with_two_pr_managers(h: &Home) {
    h.write("library/agents/flow/github/pr-manager.md", "---\nname: pr-manager\n---\n");
    h.write("library/agents/flow/templates/github-pr-manager.md", "---\nname: pr-manager\n---\n");
    h.write("kits/flow/uses", "agents/flow/\n");
    ccs::profile_store(h.0.clone()).set_kits(&name("work"), &["flow".into()]).unwrap();
}

#[test]
fn two_agents_with_one_name_in_a_kit_are_named_in_the_plan_and_in_doctor() {
    let h = stocked();
    with_two_pr_managers(&h);
    let (store, kits, lib) = (ccs::profile_store(h.0.clone()), ccs::kit_store(&h.0), ccs::library(&h.0));
    let plan = usecases::load_plan(&store, &kits, &lib, &name("work"), None).unwrap();
    let warned: Vec<&String> = plan.warnings.iter().filter(|w| w.contains("both named pr-manager")).collect();
    assert_eq!(warned.len(), 1, "{:?}", plan.warnings);
    assert!(warned[0].contains("kit flow") && warned[0].contains("github-pr-manager"), "{}", warned[0]);
    let old = ccs::legacy(&h.0, h.0.join("user/.claude"));
    let found = usecases::doctor(&store, &kits, &lib, &old, &name("work"), true).unwrap();
    assert!(
        found.iter().any(|f| matches!(f, Finding::Warn(m) if m.contains("both named pr-manager"))),
        "{found:?}"
    );
}

/// One kit whose `uses` file does not parse is a doctor finding, not the end of
/// the check: the other kit is still checked, and the overview still opens.
#[test]
fn a_kit_whose_uses_file_does_not_parse_fails_in_doctor_and_the_rest_is_checked() {
    let h = stocked();
    h.write("kits/research/uses", "skills/writing/\nnot-an-entry\n");
    let (store, kits, lib) = (ccs::profile_store(h.0.clone()), ccs::kit_store(&h.0), ccs::library(&h.0));
    let old = ccs::legacy(&h.0, h.0.join("user/.claude"));
    let found = usecases::doctor(&store, &kits, &lib, &old, &name("work"), true).expect("doctor reports, not aborts");
    assert!(
        found.iter().any(|f| matches!(f, Finding::Fail(m) if m.contains("kit research") && m.contains("not-an-entry"))),
        "{found:?}"
    );
    assert!(found.iter().any(|f| matches!(f, Finding::Pass(m) if m.starts_with("kit net:"))), "{found:?}");
}
