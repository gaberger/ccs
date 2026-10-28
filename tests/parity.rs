//! Gate G3 for ADR-2610281305: the TUI and the CLI agree. Each change made on
//! the column screen shows the `ccs` command that makes the same change. This
//! test takes the command the screen *displayed*, runs it with the real binary
//! against a second home that started the same, and compares the two homes. A
//! displayed command that ccs refuses, or that does something else, fails here.

use ccs::adapters::primary::tui::{draw, Key, State};
use ccs::domain::{CcsError, ProfileName};
use ccs::ports::{AwsProfiles, ProfileStore as _};
use ccs::usecases::{self, App};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static N: AtomicUsize = AtomicUsize::new(0);

struct NoAws;

impl AwsProfiles for NoAws {
    fn names(&self) -> Result<Vec<String>, CcsError> {
        Ok(vec![])
    }
    fn region(&self, _profile: &str) -> Option<String> {
        None
    }
}

fn name(s: &str) -> ProfileName {
    ProfileName::parse(s).unwrap()
}

/// A home with skills in writing/ and forward-dev/, plugin egm, kits writing
/// and base, and profiles work (kit writing) and solo (no kits).
fn home() -> PathBuf {
    let h = std::env::temp_dir().join(format!("ccs-parity-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = fs::remove_dir_all(&h);
    let write = |rel: &str, text: &str| {
        let p = h.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    };
    write("library/skills/writing/humanize-writing/SKILL.md", "---\nname: humanize-writing\ndescription: Rewrite text.\n---\n");
    write("library/skills/writing/report/SKILL.md", "---\nname: report\n---\n");
    write("library/skills/forward-dev/fn-parity/SKILL.md", "---\nname: fn-parity\n---\n");
    write("library/plugins/egm/.claude-plugin/plugin.json", "{\"name\": \"egm\"}\n");
    write("kits/writing/uses", "skills/writing/\n");
    write("kits/base/uses", "");
    fs::create_dir_all(h.join("user")).unwrap();
    let store = ccs::profile_store(h.clone());
    for p in ["work", "solo"] {
        usecases::new_profile(&store, &name(p), None).unwrap();
    }
    store.set_kits(&name("work"), &["writing".into()]).unwrap();
    h
}

/// Kits and which profiles start with them: what a change to kits can touch.
fn kits_state(h: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for e in fs::read_dir(h.join("kits")).unwrap().flatten() {
        let uses = fs::read_to_string(e.path().join("uses")).unwrap_or_default();
        out.insert(format!("kit {}", e.file_name().to_string_lossy()), uses);
    }
    for p in ["work", "solo"] {
        out.insert(format!("profile {p}"), fs::read_to_string(h.join(p).join("kits")).unwrap_or_default());
    }
    out
}

fn screen(s: &State) -> String {
    let mut t = Terminal::new(TestBackend::new(120, 36)).unwrap();
    t.draw(|f| draw(f, s)).unwrap();
    let buf = t.backend().buffer();
    let w = buf.area.width as usize;
    buf.content().chunks(w).map(|r| r.iter().map(|c| c.symbol()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

/// The commands the screen shows after a change: `✓ ccs a · ccs b   z undoes`.
fn displayed(s: &State) -> Vec<Vec<String>> {
    let out = screen(s);
    let line = out.lines().find(|l| l.contains("✓ ccs ")).unwrap_or_else(|| panic!("no command on screen:\n{out}"));
    let text = line.split("✓ ").nth(1).unwrap().split("   ").next().unwrap().trim();
    text.split(" · ").map(|c| c.split_whitespace().skip(1).map(String::from).collect()).collect()
}

fn run_cli(h: &Path, argv: &[String]) {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_ccs"))
        .args(argv)
        .env("HOME", h.join("user"))
        .env("CCS_HOME", h)
        .env_remove("CCS_PROFILE")
        .output()
        .unwrap();
    assert!(out.status.success(), "the displayed command `ccs {}` failed: {}", argv.join(" "), String::from_utf8_lossy(&out.stderr));
}

#[test]
fn every_change_on_the_screen_shows_a_command_that_makes_the_same_change() {
    let (a, b) = (home(), home());
    let (store, kits, lib) = (ccs::profile_store(a.clone()), ccs::kit_store(&a), ccs::library(&a));
    let (legacy, paths, editor, fetch) = (ccs::legacy(&a, a.join("user/.claude")), ccs::path_input(a.join("user"), a.clone()), ccs::editor(), ccs::fetcher());
    let app = App {
        store: &store,
        kits: &kits,
        lib: &lib,
        legacy: &legacy,
        paths: &paths,
        editor: &editor,
        aws: &NoAws,
        fetch: &fetch,
        ccs_bin: "ccs".into(),
        claude_available: true,
        current: Some("work".into()),
    };
    let mut s = State::new(&app);
    let mut checked = 0;
    let mut step = |s: &mut State, keys: &[Key], what: &str| {
        for k in keys {
            assert_eq!(s.handle(&app, *k), None, "{what}: {k:?} ended the TUI");
        }
        for argv in displayed(s) {
            run_cli(&b, &argv);
        }
        assert_eq!(kits_state(&a), kits_state(&b), "{what}: the screen and its displayed command did different things");
        checked += 1;
    };
    use Key::{Backspace, Char, Down, Enter, Left, Right, Up};
    // work's kits: [x] writing, + new kit, then the other kits: [ ] base.
    step(&mut s, &[Right, Down, Down, Char(' ')], "attach");
    step(&mut s, &[Char(' ')], "detach");
    step(&mut s, &[Up, Up, Right, Right, Char('/'), Char('f'), Char('n'), Char('-'), Char('p'), Enter, Char(' ')], "pick an item");
    step(&mut s, &[Char(' ')], "unpick it");
    let mut make = vec![Left, Left, Char('n')];
    make.extend("notes".chars().map(Char));
    make.push(Enter);
    step(&mut s, &make, "new kit");
    let mut rename = vec![Left, Left, Char('r')];
    rename.extend([Backspace; 5]);
    rename.extend("memo".chars().map(Char));
    rename.push(Enter);
    step(&mut s, &rename, "rename kit");
    step(&mut s, &[Char('x'), Char('y')], "delete kit");
    assert_eq!(checked, 7, "every action was checked; a gate over nothing passes vacuously");
    let _ = (fs::remove_dir_all(&a), fs::remove_dir_all(&b));
}
