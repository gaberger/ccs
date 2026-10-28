//! Gate for the column screen (ADR-2610281305, slice 1): 1 profile → 2 its
//! kits → 3 what is in the kit → 4 add from the library. The TUI is driven with
//! `State::handle` and painted with `draw` into a test terminal; no real
//! terminal is needed. Tests named `columns_*` are the slice 1 gate.

use ccs::adapters::primary::tui::{draw, Exit, Key, State};
use ccs::domain::{
    load_plan, resolve_lines, AwsRow, CcsError, Finding, ImportOutcome, Kind, KitName, LibItem, LibraryRemoved, LineGives, ListEntry, LoadPlan,
    Overview, PluginContents, PluginPart, ProfileName, Use,
};
use ccs::ports::Browser;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn name(s: &str) -> ProfileName {
    ProfileName::parse(s).unwrap()
}

fn item(kind: Kind, folder: &str, n: &str, about: &str) -> LibItem {
    LibItem { kind, name: n.into(), folder: folder.into(), description: Some(about.into()), shown_name: None, source: PathBuf::from(format!("/lib/{folder}/{n}")) }
}

struct Fake {
    profiles: RefCell<Vec<String>>,
    old_layout: RefCell<bool>,
    current: RefCell<Option<String>>,
    default: RefCell<Option<String>>,
    attached: RefCell<BTreeMap<String, Vec<String>>>,
    kits: RefCell<BTreeMap<String, Vec<Use>>>,
    instructions: RefCell<BTreeMap<String, String>>,
    items: RefCell<Vec<LibItem>>,
    folders: RefCell<Vec<(Kind, String)>>,
    plugins: BTreeMap<String, PluginContents>,
    calls: RefCell<Vec<String>>,
    attach_fails: RefCell<bool>,
    seed_fails: RefCell<bool>,
}

fn part(file: &str) -> PluginPart {
    PluginPart { file: file.into(), shown_name: Some(file.into()), description: Some(format!("about {file}")) }
}

impl Fake {
    /// Shaped like the real library on 2026-10-28: 41 skills, 96 agents (76 in
    /// claude-flow), plugins egm (7 skills, 9 agents) and forward-decks; 8 kits;
    /// profiles claude-bedrock-gary (current, 4 kits) and sticksandstrings (none).
    fn real() -> Self {
        let mut items = Vec::new();
        let mut folders = Vec::new();
        let skill_folders: [(&str, Vec<String>); 7] = [
            ("forward-dev", (0..10).map(|i| format!("fwd-{i:02}")).chain(["analyze-apidiff-report".into(), "fn-parity".into()]).collect()),
            ("horizon", vec!["design-system-review".into(), "horizon-worktree-dev".into()]),
            ("network/inspect", (0..10).map(|i| format!("inspect-{i:02}")).collect()),
            ("network/nqe", vec!["validate-nqe-query".into(), "find-nqe-query".into()]),
            ("network/plan", (0..12).map(|i| format!("plan-{i:02}")).chain(["plan-segmentation-check".into()]).collect()),
            ("methods", vec![]),
            ("writing", vec!["humanize-writing".into(), "forward-generate-report".into()]),
        ];
        for (f, names) in &skill_folders {
            folders.push((Kind::Skill, f.to_string()));
            for n in names {
                let about = match n.as_str() {
                    "validate-nqe-query" => "Checks a query compiles; use after writing one".to_string(),
                    "humanize-writing" => "Rewrite AI-generated text into natural prose in your voice".to_string(),
                    "plan-segmentation-check" => "Check that network segments are kept apart".to_string(),
                    _ => format!("about {n}"),
                };
                items.push(item(Kind::Skill, f, n, &about));
            }
        }
        folders.push((Kind::Skill, "network".into()));
        folders.push((Kind::Agent, "claude-flow".into()));
        for i in 0..76 {
            items.push(item(Kind::Agent, "claude-flow", &format!("flow-{i:02}"), "a claude-flow agent"));
        }
        folders.push((Kind::Agent, "dev".into()));
        for i in 0..15 {
            items.push(item(Kind::Agent, "dev", &format!("dev-{i:02}"), "a dev agent"));
        }
        folders.push((Kind::Agent, "network".into()));
        for i in 0..5 {
            items.push(item(Kind::Agent, "network", &format!("net-{i:02}"), "a network agent"));
        }
        items.push(item(Kind::Plugin, "", "egm", "Elephant and Goldfish reviews"));
        items.push(item(Kind::Plugin, "", "forward-decks", "Forward slide decks"));
        let mut plugins = BTreeMap::new();
        plugins.insert(
            "egm".to_string(),
            PluginContents {
                skills: ["egm_bootstrap", "egm_brainstorm", "egm_draft", "egm_drift_report", "egm_resolve", "egm_review", "egm_review_loop"].map(part).to_vec(),
                agents: (0..9).map(|i| part(&format!("egm-agent-{i}"))).collect(),
            },
        );
        plugins.insert("forward-decks".to_string(), PluginContents { skills: ["deck", "deck-style"].map(part).to_vec(), agents: vec![] });
        let f = |k: Kind, x: &str| Use::Folder(k, x.into());
        let kits: BTreeMap<String, Vec<Use>> = [
            ("base", vec![]),
            ("claude-flow", vec![f(Kind::Agent, "claude-flow")]),
            ("dev", vec![f(Kind::Agent, "dev"), Use::Item(Kind::Plugin, "egm".into())]),
            ("forward-dev", vec![f(Kind::Skill, "forward-dev"), f(Kind::Skill, "horizon")]),
            ("network", vec![f(Kind::Skill, "network"), f(Kind::Agent, "network")]),
            ("slides", vec![Use::Item(Kind::Plugin, "forward-decks".into())]),
            ("test", vec![f(Kind::Skill, "forward-dev"), f(Kind::Skill, "horizon")]),
            ("writing", vec![f(Kind::Skill, "writing")]),
        ]
        .into_iter()
        .map(|(k, u)| (k.to_string(), u))
        .collect();
        let attached: BTreeMap<String, Vec<String>> =
            [("claude-bedrock-gary".to_string(), ["base", "writing", "slides", "test"].map(String::from).to_vec())].into_iter().collect();
        Fake {
            profiles: RefCell::new(vec!["claude-bedrock-gary".into(), "sticksandstrings".into()]),
            old_layout: RefCell::new(false),
            current: RefCell::new(Some("claude-bedrock-gary".into())),
            default: RefCell::new(Some("claude-bedrock-gary".into())),
            attached: RefCell::new(attached),
            kits: RefCell::new(kits),
            instructions: RefCell::new([("base".to_string(), "Be brief.".to_string())].into_iter().collect()),
            items: RefCell::new(items),
            folders: RefCell::new(folders),
            plugins,
            calls: RefCell::new(vec![]),
            attach_fails: RefCell::new(false),
            seed_fails: RefCell::new(false),
        }
    }
    fn old_layout(self) -> Self {
        *self.old_layout.borrow_mut() = true;
        self
    }
    /// Kit `writing` on both profiles, so a change to it changes both.
    fn writing_shared(self) -> Self {
        self.attached.borrow_mut().insert("sticksandstrings".into(), vec!["writing".into()]);
        self
    }
    fn called(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
    fn log(&self, s: String) {
        self.calls.borrow_mut().push(s);
    }
    fn uses(&self, kit: &str) -> Vec<String> {
        self.kits.borrow().get(kit).map(|u| u.iter().map(Use::line).collect()).unwrap_or_default()
    }
    fn kits_of(&self, p: &str) -> Vec<String> {
        self.attached.borrow().get(p).cloned().unwrap_or_default()
    }
}

impl Browser for Fake {
    /// Sorted by name, as `fs_store` lists them.
    fn profiles(&self) -> Vec<ListEntry> {
        let mut names = self.profiles.borrow().clone();
        names.sort();
        names
            .iter()
            .map(|p| ListEntry {
                name: name(p),
                current: self.current.borrow().as_deref() == Some(p.as_str()),
                is_default: self.default.borrow().as_deref() == Some(p.as_str()),
                kits: self.kits_of(p),
                provider: if p == "sticksandstrings" { "anthropic".into() } else { "bedrock sso@us-east-1".into() },
            })
            .collect()
    }
    fn overview(&self, n: &ProfileName) -> Result<Overview, CcsError> {
        Ok(Overview {
            name: n.clone(),
            segment: format!("⬡ {}", n.as_str()),
            env: vec![],
            findings: vec![Finding::Pass("ok".into())],
            is_default: false,
            kits: self.kits_of(n.as_str()),
            dir: PathBuf::from(format!("/profiles/{}", n.as_str())),
        })
    }
    fn set_default(&self, n: &ProfileName) -> Result<(), CcsError> {
        self.log(format!("set_default {}", n.as_str()));
        *self.default.borrow_mut() = Some(n.as_str().into());
        Ok(())
    }
    fn kits(&self) -> Vec<String> {
        self.kits.borrow().keys().cloned().collect()
    }
    fn attach(&self, kit: &str, profile: &ProfileName, on: bool) -> Result<(), CcsError> {
        self.log(format!("attach {kit} {} {on}", profile.as_str()));
        if *self.attach_fails.borrow() {
            return Err(CcsError::Usage(format!("cannot attach {kit}")));
        }
        let mut a = self.attached.borrow_mut();
        let list = a.entry(profile.as_str().into()).or_default();
        list.retain(|k| k != kit);
        if on {
            list.push(kit.into());
        }
        Ok(())
    }
    fn new_kit(&self, kit: &str) -> Result<(), CcsError> {
        if KitName::parse(kit).is_err() {
            return Err(CcsError::Usage(format!("bad kit name '{kit}'")));
        }
        if self.items.borrow().iter().any(|i| i.kind == Kind::Plugin && i.name == kit) {
            return Err(CcsError::Usage(format!("{kit} is a plugin in the library; two plugins would load with one name. Choose another name.")));
        }
        self.log(format!("new_kit {kit}"));
        self.kits.borrow_mut().insert(kit.into(), vec![]);
        Ok(())
    }
    fn remove_kit(&self, kit: &str) -> Result<PathBuf, CcsError> {
        self.log(format!("remove_kit {kit}"));
        self.kits.borrow_mut().remove(kit);
        for list in self.attached.borrow_mut().values_mut() {
            list.retain(|k| k != kit);
        }
        Ok(PathBuf::from(format!("/t/.trash/1-kit-{kit}")))
    }
    fn rename_kit(&self, old: &str, new: &str) -> Result<(), CcsError> {
        self.log(format!("rename_kit {old} {new}"));
        let uses = self.kits.borrow_mut().remove(old).unwrap_or_default();
        self.kits.borrow_mut().insert(new.into(), uses);
        for list in self.attached.borrow_mut().values_mut() {
            for k in list.iter_mut().filter(|k| *k == old) {
                *k = new.into();
            }
        }
        Ok(())
    }
    fn kit_uses(&self, kit: &str) -> Result<Vec<Use>, CcsError> {
        Ok(self.kits.borrow().get(kit).cloned().unwrap_or_default())
    }
    fn kit_lines(&self, kit: &str) -> Result<Vec<LineGives>, CcsError> {
        Ok(resolve_lines(&self.kit_uses(kit)?, &self.items.borrow()))
    }
    fn load_plan(&self, profile: &ProfileName) -> Result<LoadPlan, CcsError> {
        let kits = self
            .kits_of(profile.as_str())
            .into_iter()
            .map(|k| (KitName::parse(&k).unwrap(), self.kit_uses(&k).unwrap(), self.instructions.borrow().get(&k).cloned()))
            .collect();
        Ok(load_plan(kits, &self.items.borrow()))
    }
    fn plugin_contents(&self, plugin: &str) -> Result<PluginContents, CcsError> {
        self.plugins.get(plugin).cloned().ok_or_else(|| CcsError::NoAsset(format!("plugins/{plugin}")))
    }
    fn set_use(&self, kit: &str, entry: &Use, on: bool) -> Result<(), CcsError> {
        self.log(format!("set_use {kit} {} {on}", entry.line()));
        let mut k = self.kits.borrow_mut();
        let uses = k.entry(kit.into()).or_default();
        uses.retain(|u| u != entry);
        if on {
            uses.push(entry.clone());
        }
        Ok(())
    }
    fn kit_instructions(&self, kit: &str) -> Option<String> {
        self.instructions.borrow().get(kit).cloned()
    }
    fn edit_kit(&self, kit: &str) -> Result<(), CcsError> {
        self.log(format!("edit_kit {kit}"));
        Ok(())
    }
    fn library(&self, kind: Kind) -> (Vec<LibItem>, Vec<String>) {
        let items = self.items.borrow().iter().filter(|i| i.kind == kind).cloned().collect();
        let folders = self.folders.borrow().iter().filter(|f| f.0 == kind).map(|f| f.1.clone()).collect();
        (items, folders)
    }
    fn read(&self, entry: &Use) -> Result<String, CcsError> {
        if entry.line() == "skills/fn-parity" {
            return Ok((1..=200).map(|i| format!("line {i}\n")).collect());
        }
        Ok(format!("---\ndescription: x\n---\nBody of {}\n", entry.line()))
    }
    fn edit(&self, entry: &Use) -> Result<(), CcsError> {
        self.log(format!("edit {}", entry.line()));
        Ok(())
    }
    fn add(&self, path: &str, folder: &str) -> Result<(Kind, String), CcsError> {
        self.log(format!("add {path} in '{folder}'"));
        let n = path.trim_end_matches('/').rsplit('/').next().unwrap_or(path).to_string();
        self.items.borrow_mut().push(item(Kind::Skill, folder, &n, "added"));
        Ok((Kind::Skill, n))
    }
    fn update_plugins(&self, which: Option<&str>) -> Result<Vec<String>, CcsError> {
        self.log(format!("update_plugins {}", which.unwrap_or("*")));
        Ok(vec!["plugins/egm: up to date at 3d0b348".into()])
    }
    fn mkdir(&self, kind: Kind, folder: &str) -> Result<(), CcsError> {
        self.log(format!("mkdir {} {folder}", kind.dir()));
        self.folders.borrow_mut().push((kind, folder.into()));
        Ok(())
    }
    fn move_entry(&self, entry: &Use, to: &str) -> Result<(), CcsError> {
        self.log(format!("move {} to '{to}'", entry.line()));
        Ok(())
    }
    fn rename_folder(&self, kind: Kind, folder: &str, new: &str) -> Result<(), CcsError> {
        self.log(format!("rename_folder {} {folder} {new}", kind.dir()));
        Ok(())
    }
    fn remove(&self, entry: &Use) -> Result<LibraryRemoved, CcsError> {
        self.log(format!("remove {}", entry.line()));
        if let Use::Item(_, n) = entry {
            self.items.borrow_mut().retain(|i| i.name != *n);
        }
        Ok(LibraryRemoved { trashed: PathBuf::from("/t/.trash/1-library-x"), kits: vec![] })
    }
    fn set_kits(&self, profile: &ProfileName, kits: &[String]) -> Result<(), CcsError> {
        self.log(format!("set_kits {} {}", profile.as_str(), kits.join(",")));
        self.attached.borrow_mut().insert(profile.as_str().into(), kits.to_vec());
        Ok(())
    }
    fn complete_path(&self, typed: &str) -> Vec<String> {
        ["~/skills/", "~/src/a/", "~/src/b/", "~/notes.md"].iter().filter(|c| c.starts_with(typed)).map(|c| c.to_string()).collect()
    }
    fn new_profile(&self, n: &str, from: Option<&ProfileName>) -> Result<(), CcsError> {
        if ProfileName::parse(n).is_err() {
            return Err(CcsError::Usage(format!("bad profile name '{n}'")));
        }
        self.log(format!("new_profile {n} {}", from.map_or("-", |f| f.as_str())));
        let mut all = self.profiles.borrow_mut();
        all.push(n.into());
        all.sort();
        // `make_profile` writes the profile, then the status line: the second write can fail.
        if *self.seed_fails.borrow() {
            return Err(CcsError::Usage(format!("cannot write the status line for {n}")));
        }
        Ok(())
    }
    fn edit_profile(&self, n: &ProfileName) -> Result<(), CcsError> {
        self.log(format!("edit_profile {}", n.as_str()));
        Ok(())
    }
    fn needs_migrate(&self) -> bool {
        *self.old_layout.borrow()
    }
    fn migrate(&self) -> Result<Vec<String>, CcsError> {
        self.log("migrate".into());
        *self.old_layout.borrow_mut() = false;
        Ok(vec!["shared/ is now kit shared".into()])
    }
    fn import_claude(&self) -> Result<Vec<String>, CcsError> {
        self.log("import_claude".into());
        Ok(vec!["skills/humanize: already in the library".into()])
    }
    fn aws_profiles(&self) -> Result<Vec<AwsRow>, CcsError> {
        Ok(vec![AwsRow { aws: "dev".into(), imported_as: None }, AwsRow { aws: "sso".into(), imported_as: Some("claude-bedrock-gary".into()) }])
    }
    fn import_aws(&self, names: &[String], all: bool) -> Result<Vec<ImportOutcome>, CcsError> {
        self.log(format!("import_aws {} {all}", names.join(",")));
        let mut out = vec![];
        for n in if all { vec!["dev".to_string()] } else { names.to_vec() } {
            self.profiles.borrow_mut().push(n.clone());
            out.push(ImportOutcome::Created { name: n, region: Some("us-east-1".into()) });
        }
        Ok(out)
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn screen_at(state: &State, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| draw(f, state)).unwrap();
    let buf = t.backend().buffer();
    let w = buf.area.width as usize;
    buf.content().chunks(w).map(|row| row.iter().map(|c| c.symbol()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

fn screen(state: &State) -> String {
    screen_at(state, 120, 36)
}

fn press(state: &mut State, b: &dyn Browser, keys: &[Key]) -> Option<Exit> {
    let mut last = None;
    for k in keys {
        last = state.handle(b, *k);
        if last.is_some() {
            break;
        }
    }
    last
}

fn typed(state: &mut State, b: &dyn Browser, text: &str) {
    for c in text.chars() {
        state.handle(b, Key::Char(c));
    }
}

use Key::{Char, Down, End, Enter, Esc, Left, PageDown, Right, Up, WheelDown};

fn line_with<'a>(screen: &'a str, text: &str) -> &'a str {
    screen.lines().find(|l| l.contains(text)).unwrap_or_else(|| panic!("no line with {text:?} in:\n{screen}"))
}

/// The step line: the screen's first line.
fn steps(out: &str) -> String {
    out.lines().next().unwrap_or("").to_string()
}

/// The key line: the screen's last line.
fn keys(out: &str) -> String {
    out.lines().last().unwrap_or("").to_string()
}

/// Opens on the resolved profile, then goes to sticksandstrings' kits.
fn at_sticksandstrings_kits(b: &Fake) -> State {
    let mut s = State::new(b);
    press(&mut s, b, &[Down, Right]);
    s
}

/// Kit `writing` open in the library column, from claude-bedrock-gary.
fn writing_in_library(b: &Fake) -> State {
    let mut s = State::new(b);
    press(&mut s, b, &[Right]);
    // claude-bedrock-gary's kits in launch order: base, writing, slides, test.
    press(&mut s, b, &[Down, Right, Right]);
    s
}

// ── G1 and G2: the success test, with every key on screen before it is pressed ─

/// What must be on screen for each scripted key (G2); typed text needs none.
fn shown_for(key: Key, typing: bool) -> Option<&'static str> {
    if typing {
        return match key {
            Enter => Some("enter"),
            _ => None,
        };
    }
    Some(match key {
        Down => "↑↓",
        Right => "→",
        Left => "←",
        Char('n') => "n new kit",
        Char('/') => "/ finds",
        Char(' ') => "space adds",
        Enter => "enter",
        other => panic!("no G2 token for {other:?}"),
    })
}

/// Make kit research, put plugin egm and folder writing/ in it, attach it to
/// sticksandstrings, and come back to that profile's load list.
fn success_script(w: u16, h: u16) {
    let b = Fake::real();
    let mut s = State::new(&b);
    // (key, typing?) — typing is true while text goes into a box.
    let mut script: Vec<(Key, bool)> = vec![(Down, false), (Right, false), (Char('n'), false)];
    script.extend("research".chars().map(|c| (Char(c), true)));
    script.push((Enter, true));
    script.push((Char('/'), false));
    script.extend("egm".chars().map(|c| (Char(c), true)));
    script.push((Enter, true));
    script.push((Char(' '), false));
    script.push((Char('/'), false));
    script.extend("writing".chars().map(|c| (Char(c), true)));
    script.push((Enter, true));
    script.push((Char(' '), false));
    script.extend([(Left, false), (Left, false), (Left, false)]);
    assert!(script.len() <= 40, "G1: {} keys", script.len());
    for (n, (key, typing)) in script.iter().enumerate() {
        let out = screen_at(&s, w, h);
        if let Some(token) = shown_for(*key, *typing) {
            assert!(out.contains(token), "G2 at {w}x{h}, key {n} ({key:?}): {token:?} is not on screen:\n{out}");
        }
        assert_eq!(s.handle(&b, *key), None, "key {n} ({key:?}) ended the TUI");
    }
    assert_eq!(b.uses("research"), ["plugins/egm", "skills/writing/"], "names stay name-only (R1)");
    assert_eq!(b.kits_of("sticksandstrings"), ["research"]);
    let out = screen_at(&s, w, h);
    assert!(steps(&out).contains("sticksandstrings"), "{out}");
    for want in ["research", "egm", "humanize-writing"] {
        assert!(out.contains(want), "{want:?}: the load list of sticksandstrings is on screen at {w}x{h}:\n{out}");
    }
}

#[test]
fn columns_g1_the_success_test_at_120_by_36() {
    success_script(120, 36);
}

#[test]
fn columns_g1_the_success_test_at_80_by_24() {
    success_script(80, 24);
}

// ── G1b: search finds what you typed first ───────────────────────────────────

/// G2 checks only the keys the script sends. This walks every row of
/// columns 2-4 at 80 columns: each key line must show the way back.
#[test]
fn columns_every_key_line_shows_the_way_back_at_80_columns() {
    let b = Fake::real();
    let mut s = writing_in_library(&b);
    // Open a folder and a plugin, and pick one item, so every kind of row and mark is on screen.
    press(&mut s, &b, &[Char('/')]);
    typed(&mut s, &b, "fn-parity");
    press(&mut s, &b, &[Enter, Char(' '), Esc]);
    press(&mut s, &b, &[Char('/')]);
    typed(&mut s, &b, "egm");
    press(&mut s, &b, &[Enter, Esc, Enter]);
    let mut seen = 0;
    for col in [3, 2, 1] {
        press(&mut s, &b, &[Key::Home]);
        loop {
            let k = keys(&screen_at(&s, 80, 24));
            assert!(k.contains('←'), "column {}: the key line has no way back at 80 columns: {k}", col + 1);
            seen += 1;
            let before = screen_at(&s, 80, 24);
            press(&mut s, &b, &[Down]);
            if screen_at(&s, 80, 24) == before {
                break;
            }
        }
        press(&mut s, &b, &[Left]);
    }
    assert!(seen > 40, "every row was checked, {seen} in all");
}

#[test]
fn columns_search_puts_the_plugin_and_the_folder_first() {
    let b = Fake::real();
    let mut s = writing_in_library(&b);
    press(&mut s, &b, &[Char('/')]);
    typed(&mut s, &b, "egm");
    press(&mut s, &b, &[Enter]);
    let out = screen(&s);
    assert!(keys(&out).contains("plugins/egm"), "the first result is the plugin:\n{out}");
    press(&mut s, &b, &[Char('/')]);
    typed(&mut s, &b, "writing");
    press(&mut s, &b, &[Enter]);
    let out = screen(&s);
    assert!(keys(&out).contains("skills/writing/"), "the first result is the folder:\n{out}");
}

// ── Column 1: profiles ───────────────────────────────────────────────────────

#[test]
fn columns_it_opens_on_the_resolved_profile_and_shows_what_it_loads() {
    let b = Fake::real();
    let s = State::new(&b);
    let out = screen(&s);
    assert!(steps(&out).contains("1 claude-bedrock-gary"), "{out}");
    assert!(out.contains("claude-bedrock-gary loads"), "column 3 names the profile:\n{out}");
    for want in ["kit writing", "humanize-writing", "kit slides", "forward-decks", "kit base", "instructions (CLAUDE.md)"] {
        assert!(out.contains(want), "{want:?} missing:\n{out}");
    }
    let k = keys(&out);
    assert!(k.contains("enter starts claude") && k.contains("→ its kits"), "the key line names both: {k}");
    // No kit is open yet, so the library marks nothing: an empty box would say "not picked".
    assert!(!out.contains("[ ] ▸ writing/") && out.contains("▸ writing/"), "{out}");
}

#[test]
fn columns_enter_launches_only_in_column_1() {
    let b = Fake::real();
    let mut s = State::new(&b);
    assert_eq!(press(&mut s, &b, &[Down, Enter]), Some(Exit::Launch("sticksandstrings".into())));
    let mut s = State::new(&b);
    press(&mut s, &b, &[Right]);
    assert_eq!(s.handle(&b, Enter), None, "Enter on a kit opens it, it launches nothing");
}

#[test]
fn columns_profile_keys_edit_default_copy_and_shell() {
    let b = Fake::real();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Down, Char('e')]);
    assert!(s.wants_editor());
    s.run_editor(&b);
    press(&mut s, &b, &[Char('d')]);
    assert_eq!(b.called(), ["edit_profile sticksandstrings", "set_default sticksandstrings"]);
    press(&mut s, &b, &[Char('c')]);
    typed(&mut s, &b, "copy");
    press(&mut s, &b, &[Enter]);
    assert!(b.called().contains(&"new_profile copy sticksandstrings".to_string()), "{:?}", b.called());
    let mut s = State::new(&b);
    assert_eq!(press(&mut s, &b, &[Char('!')]), Some(Exit::Shell("claude-bedrock-gary".into())));
}

#[test]
fn columns_esc_goes_back_a_column_and_quits_only_from_column_1() {
    let b = Fake::real();
    let mut s = at_sticksandstrings_kits(&b);
    assert_eq!(s.handle(&b, Esc), None, "column 2 → column 1");
    assert!(keys(&screen(&s)).contains("enter starts claude"));
    assert_eq!(s.handle(&b, Esc), Some(Exit::Quit));
    let mut s = at_sticksandstrings_kits(&b);
    assert_eq!(s.handle(&b, Char('q')), Some(Exit::Quit), "q quits from any column");
}

#[test]
fn columns_the_rows_under_the_profiles_make_a_profile_and_import() {
    let b = Fake::real().old_layout();
    let mut s = State::new(&b);
    let out = screen(&s);
    assert!(out.contains("+ new profile") && out.contains("import"), "{out}");
    press(&mut s, &b, &[Down, Down, Enter]);
    typed(&mut s, &b, "lab");
    press(&mut s, &b, &[Enter]);
    assert!(b.called().contains(&"new_profile lab -".to_string()), "{:?}", b.called());
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('i')]);
    let out = screen(&s);
    for want in ["migrate", "~/.claude", "aws dev", "every AWS profile"] {
        assert!(out.contains(want), "{want:?} in the import list:\n{out}");
    }
    let first = out.find("migrate").unwrap();
    assert!(first < out.find("~/.claude").unwrap(), "migrate first when an old layout is here");
    press(&mut s, &b, &[Enter]);
    assert!(b.called().contains(&"migrate".to_string()));
    assert!(screen(&s).contains("shared/ is now kit shared"), "the result is shown");
}

#[test]
fn columns_an_import_that_sorts_first_keeps_the_cursor_on_its_profile() {
    let b = Fake::real();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Down, Char('i'), Down, Enter, Esc]);
    assert!(b.called().contains(&"import_aws dev false".to_string()), "{:?}", b.called());
    assert_eq!(b.profiles().iter().map(|p| p.name.as_str().to_string()).collect::<Vec<_>>(), ["claude-bedrock-gary", "dev", "sticksandstrings"]);
    let out = screen(&s);
    assert!(out.contains("● sticksandstrings") && out.contains("○ dev"), "the mark stays on sticksandstrings, not on the index it had:\n{out}");
    assert_eq!(press(&mut s, &b, &[Enter]), Some(Exit::Launch("sticksandstrings".into())), "dev sorts in above it; the cursor stays on sticksandstrings");
}

// ── Column 2: kits ───────────────────────────────────────────────────────────

#[test]
fn columns_a_profiles_own_kits_come_first_then_the_other_kits() {
    let b = Fake::real();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Right]);
    let out = screen(&s);
    let own = out.find("[x] base").expect("own kits are ticked");
    let other = out.find("OTHER KITS").expect("other kits are listed");
    assert!(own < other && out.find("[ ] claude-flow").unwrap() > other, "{out}");
}

#[test]
fn columns_space_puts_a_kit_on_the_profile_shows_the_command_and_z_undoes() {
    let b = Fake::real();
    let mut s = at_sticksandstrings_kits(&b);
    // sticksandstrings has no kits: the rows are "+ new kit", then the others.
    let out = screen(&s);
    assert!(out.contains("none yet"), "{out}");
    press(&mut s, &b, &[Down, Char(' ')]);
    assert_eq!(b.kits_of("sticksandstrings"), ["base"]);
    let out = screen(&s);
    assert!(out.contains("ccs kit attach base sticksandstrings"), "the matching command is shown:\n{out}");
    assert!(keys(&out).contains("space"), "the key line stays on screen after a change:\n{out}");
    press(&mut s, &b, &[Char('z')]);
    assert!(b.kits_of("sticksandstrings").is_empty(), "z undoes the last change");
}

#[test]
fn columns_n_makes_a_kit_puts_it_on_this_profile_and_opens_the_library() {
    let b = Fake::real();
    let mut s = at_sticksandstrings_kits(&b);
    press(&mut s, &b, &[Char('n')]);
    assert!(screen(&s).contains("sticksandstrings"), "the box names the profile it goes on");
    typed(&mut s, &b, "research");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.kits_of("sticksandstrings"), ["research"]);
    let out = screen(&s);
    assert!(steps(&out).contains("4 add from library"), "it lands in the library: {}", steps(&out));
    assert!(out.contains("ccs kit new research") && out.contains("ccs kit attach research sticksandstrings"), "{out}");
}

#[test]
fn columns_n_shows_the_new_kit_in_column_2_when_the_attach_fails() {
    let b = Fake::real();
    let mut s = at_sticksandstrings_kits(&b);
    *b.attach_fails.borrow_mut() = true;
    press(&mut s, &b, &[Char('n')]);
    typed(&mut s, &b, "research");
    press(&mut s, &b, &[Enter]);
    assert!(b.kits().contains(&"research".to_string()), "new_kit wrote the kit");
    let out = screen(&s);
    assert!(out.contains("[ ] research"), "column 2 lists the kit that is now on disk:\n{out}");
    assert!(out.contains("cannot attach research"), "the error is shown:\n{out}");
    assert!(!out.lines().any(|l| l.contains("✓") && l.contains("cannot attach")), "a partial failure is not shown as a success:\n{out}");
}

#[test]
fn columns_n_shows_the_new_profile_in_column_1_when_the_status_line_fails() {
    let b = Fake::real();
    let mut s = State::new(&b);
    *b.seed_fails.borrow_mut() = true;
    press(&mut s, &b, &[Char('n')]);
    typed(&mut s, &b, "research");
    press(&mut s, &b, &[Enter]);
    assert!(b.profiles().iter().any(|p| p.name.as_str() == "research"), "new_profile wrote the profile");
    let out = screen(&s);
    assert!(out.lines().any(|l| l.contains("research") && !l.contains("cannot")), "column 1 lists the profile that is now on disk:\n{out}");
    assert!(out.contains("cannot write the status line for research"), "the error is shown:\n{out}");
}

#[test]
fn columns_a_kit_cannot_take_a_plugins_name_and_the_screen_says_why() {
    let b = Fake::real();
    let mut s = at_sticksandstrings_kits(&b);
    press(&mut s, &b, &[Char('n')]);
    typed(&mut s, &b, "egm");
    press(&mut s, &b, &[Enter]);
    assert!(screen(&s).contains("egm is a plugin in the library"), "{}", screen(&s));
    assert!(b.kits_of("sticksandstrings").is_empty());
}

#[test]
fn columns_r_renames_and_x_deletes_a_kit_after_asking() {
    let b = Fake::real();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Right, Down, Char('r')]);
    for _ in 0.."writing".len() {
        press(&mut s, &b, &[Key::Backspace]);
    }
    typed(&mut s, &b, "prose");
    press(&mut s, &b, &[Enter]);
    assert!(b.called().contains(&"rename_kit writing prose".to_string()), "{:?}", b.called());
    press(&mut s, &b, &[Char('x')]);
    assert!(screen(&s).contains("y/n"), "it asks first");
    press(&mut s, &b, &[Char('n')]);
    assert!(!b.called().iter().any(|c| c.starts_with("remove_kit")), "n keeps it");
    press(&mut s, &b, &[Char('x'), Char('y')]);
    assert!(b.called().iter().any(|c| c.starts_with("remove_kit")), "{:?}", b.called());
}

#[test]
fn columns_a_shared_kit_says_a_change_changes_the_other_profile_too() {
    let b = Fake::real().writing_shared();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Right, Down]);
    let out = screen(&s);
    assert!(line_with(&out, "also on").contains("sticksandstrings"), "{out}");
}

// ── Column 3: in the kit ─────────────────────────────────────────────────────

#[test]
fn columns_a_kits_lines_show_the_items_they_give_and_space_drops_a_line() {
    let b = Fake::real();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Right, Down, Down, Down, Right]); // kit test
    let out = screen(&s);
    assert!(steps(&out).contains("3 in test"), "{}", steps(&out));
    // Kit test has no CLAUDE.md: its first row says so and offers to write one.
    for want in ["instructions: none", "skills/forward-dev/", "fn-parity", "skills/horizon/", "design-system-review"] {
        assert!(out.contains(want), "{want:?}:\n{out}");
    }
    press(&mut s, &b, &[Down, Char(' ')]);
    assert_eq!(b.uses("test"), ["skills/horizon/"]);
    assert!(screen(&s).contains("ccs kit drop test skills/forward-dev/"));
}

#[test]
fn columns_e_on_the_instructions_row_edits_them() {
    let b = Fake::real();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Right, Right, Char('e')]); // kit base, the instructions row
    assert!(s.wants_editor());
    s.run_editor(&b);
    assert_eq!(b.called(), ["edit_kit base"]);
}

// ── Column 4: the library ────────────────────────────────────────────────────

#[test]
fn columns_the_library_marks_what_the_kit_picks_and_folds_folders() {
    let b = Fake::real();
    let s = writing_in_library(&b);
    let out = screen(&s);
    assert!(steps(&out).contains("4 add from library"), "{}", steps(&out));
    // Rows read: mark, arrow, name. Four columns share each screen line, so
    // the check is on that exact text, not on "a line that also has [x]".
    assert!(out.contains("[x] ▸ writing/"), "the kit picks the folder:\n{out}");
    assert!(out.contains("[ ] ▸ claude-flow/"), "{out}");
    assert!(!out.contains("flow-00"), "folders start folded: 76 agents are one row");
}

#[test]
fn columns_space_picks_and_unpicks_and_a_folder_item_says_where_it_comes_from() {
    let b = Fake::real();
    let mut s = writing_in_library(&b);
    press(&mut s, &b, &[Char('/')]);
    typed(&mut s, &b, "humanize");
    press(&mut s, &b, &[Enter, Char(' ')]);
    assert!(screen(&s).contains("comes with skills/writing/"), "{}", screen(&s));
    assert_eq!(b.uses("writing"), ["skills/writing/"], "nothing changed");
    press(&mut s, &b, &[Char('/')]);
    typed(&mut s, &b, "fn-parity");
    press(&mut s, &b, &[Enter, Char(' ')]);
    assert_eq!(b.uses("writing"), ["skills/writing/", "skills/fn-parity"], "an item line is name-only");
    assert!(screen(&s).contains("ccs kit use writing skills/fn-parity"));
    press(&mut s, &b, &[Char(' ')]);
    assert_eq!(b.uses("writing"), ["skills/writing/"]);
}

#[test]
fn columns_a_partly_picked_folder_says_how_many() {
    let b = Fake::real();
    let mut s = writing_in_library(&b);
    press(&mut s, &b, &[Char('/')]);
    typed(&mut s, &b, "fn-parity");
    press(&mut s, &b, &[Enter, Char(' '), Esc]);
    let out = screen(&s);
    assert!(line_with(&out, "forward-dev/").contains("[-]"), "{out}");
    assert!(line_with(&out, "forward-dev/").contains("1/12"), "{out}");
}

#[test]
fn columns_a_plugin_is_one_switch_and_its_contents_cannot_be_picked() {
    let b = Fake::real();
    let mut s = writing_in_library(&b);
    press(&mut s, &b, &[Char('/')]);
    typed(&mut s, &b, "egm");
    press(&mut s, &b, &[Enter, Esc]);
    // Without a filter the cursor stays on egm; open it and step onto a skill inside.
    let out = screen(&s);
    assert!(line_with(&out, " egm").contains("7 skills, 9 agents"), "{out}");
    press(&mut s, &b, &[Enter]);
    assert!(screen(&s).contains("egm_draft"), "the plugin opens to show what is inside");
    press(&mut s, &b, &[Down, Char(' ')]);
    assert!(screen(&s).contains("inside the plugin egm"), "{}", screen(&s));
    assert_eq!(b.uses("writing"), ["skills/writing/"]);
}

#[test]
fn columns_leaving_a_search_reopens_plugins_to_keep_the_cursor_inside_a_plugin() {
    let b = Fake::real();
    let mut s = writing_in_library(&b);
    press(&mut s, &b, &[End, Up, Up, Enter]); // PLUGINS closes
    assert!(!screen(&s).contains("forward-decks"), "PLUGINS is closed:\n{}", screen(&s));
    press(&mut s, &b, &[Char('/')]);
    typed(&mut s, &b, "egm_draft");
    press(&mut s, &b, &[Enter, Esc]);
    // Without a filter the cursor stays on egm_draft, so space names the plugin it is inside.
    press(&mut s, &b, &[Char(' ')]);
    assert!(screen(&s).contains("egm_draft is inside the plugin egm"), "{}", screen(&s));
}

#[test]
fn columns_left_closes_an_open_folder_and_otherwise_leaves_the_column() {
    let b = Fake::real();
    let mut s = writing_in_library(&b);
    press(&mut s, &b, &[Char('/')]);
    typed(&mut s, &b, "writing");
    press(&mut s, &b, &[Enter, Esc, Enter]);
    assert!(screen(&s).contains("humanize-writing"), "the folder is open");
    press(&mut s, &b, &[Left]);
    let out = screen(&s);
    assert!(!out.lines().any(|l| l.contains("[^]") && l.contains("humanize-writing")), "← closes it:\n{out}");
    assert!(steps(&out).contains("4 add from library"), "and stays in the column");
    press(&mut s, &b, &[Left]);
    assert!(steps(&screen(&s)).contains("3 in writing"), "← then leaves it: {}", steps(&screen(&s)));
}

#[test]
fn columns_the_strip_shows_the_whole_description_and_v_reads_the_item() {
    let b = Fake::real();
    let mut s = writing_in_library(&b);
    press(&mut s, &b, &[Char('/')]);
    typed(&mut s, &b, "humanize");
    press(&mut s, &b, &[Enter]);
    let out = screen(&s);
    assert!(out.contains("Rewrite AI-generated text into natural prose in your voice"), "{out}");
    press(&mut s, &b, &[Char('v')]);
    assert!(screen(&s).contains("Body of skills/humanize-writing"), "{}", screen(&s));
    press(&mut s, &b, &[Esc]);
    assert!(!screen(&s).contains("Body of"), "esc closes the reader");
}

#[test]
fn columns_the_reader_pages_through_long_text() {
    let b = Fake::real();
    let mut s = writing_in_library(&b);
    press(&mut s, &b, &[Char('/')]);
    typed(&mut s, &b, "fn-parity");
    press(&mut s, &b, &[Enter, Char('v')]);
    let first = screen(&s);
    assert!(first.lines().any(|l| l.trim_end().ends_with("line 1") || l.contains("line 1 ")), "{first}");
    press(&mut s, &b, &[PageDown]);
    assert!(!screen(&s).lines().any(|l| l.trim_end().ends_with(" line 1")), "a page down moves the text");
    press(&mut s, &b, &[End]);
    assert!(screen(&s).contains("line 200"));
}

#[test]
fn columns_library_actions_keep_their_keys() {
    let b = Fake::real();
    let mut s = writing_in_library(&b);
    press(&mut s, &b, &[Char('a')]);
    typed(&mut s, &b, "~/sk");
    press(&mut s, &b, &[Key::Tab]);
    assert!(screen(&s).contains("~/skills/"), "tab completes a path");
    press(&mut s, &b, &[Enter]);
    assert!(b.called().iter().any(|c| c.starts_with("add ~/skills/")), "{:?}", b.called());
    press(&mut s, &b, &[Char('u')]);
    assert!(b.called().iter().any(|c| c.starts_with("update_plugins")), "{:?}", b.called());
}

// ── Widths, scrolling and the mouse ──────────────────────────────────────────

#[test]
fn columns_at_80_three_columns_show_and_slide_and_below_70_one() {
    let b = Fake::real();
    let mut s = State::new(&b);
    let out = screen_at(&s, 80, 24);
    assert!(out.contains("1 PROFILE") && out.contains("2 KITS") && !out.contains("4 LIBRARY"), "{out}");
    press(&mut s, &b, &[Right, Down, Right, Right]);
    let out = screen_at(&s, 80, 24);
    assert!(out.contains("4 LIBRARY") && !out.contains("1 PROFILE"), "they slide with the focus:\n{out}");
    let out = screen_at(&s, 60, 24);
    assert!(out.contains("4 LIBRARY") && !out.contains("2 KITS"), "one column below 70:\n{out}");
}

#[test]
fn columns_the_wheel_and_page_keys_move_in_the_library() {
    let b = Fake::real();
    let mut s = writing_in_library(&b);
    press(&mut s, &b, &[Char('/')]);
    typed(&mut s, &b, "claude-flow");
    press(&mut s, &b, &[Enter, Esc, Enter]); // claude-flow/ opens
    press(&mut s, &b, &[Down, WheelDown]);
    assert!(keys(&screen(&s)).contains("flow-0"), "{}", keys(&screen(&s)));
    press(&mut s, &b, &[End]);
    assert!(keys(&screen(&s)).contains("forward-decks"), "End reaches the last row: {}", keys(&screen(&s)));
    let mut s2 = writing_in_library(&b);
    press(&mut s2, &b, &[Up, Up, Up]);
    assert!(steps(&screen(&s2)).contains("4 add from library"), "Up at the top stays put");
}

#[test]
fn columns_a_click_selects_a_row_and_a_click_on_a_step_goes_there() {
    let b = Fake::real();
    let mut s = State::new(&b);
    let out = screen(&s);
    let (row, line) = out.lines().enumerate().skip(1).find(|(_, l)| l.contains("sticksandstrings")).unwrap();
    let col = line.find("sticksandstrings").unwrap();
    s.handle(&b, Key::Click(line[..col].chars().count() as u16, row as u16));
    assert!(steps(&screen(&s)).contains("1 sticksandstrings"), "a click chose the profile:\n{}", screen(&s));
    press(&mut s, &b, &[Right, Down, Right]);
    let top = steps(&screen(&s));
    let col = top.find("1 sticksandstrings").unwrap();
    s.handle(&b, Key::Click(top[..col].chars().count() as u16, 0));
    assert!(keys(&screen(&s)).contains("enter starts claude"), "a click on step 1 goes back to it");
}

#[test]
fn ui_is_a_command_and_a_reserved_name() {
    let p = |a: &[&str]| ccs::domain::parse_command(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>(), |_| false);
    assert_eq!(p(&["ui"]).unwrap(), ccs::domain::Command::Ui);
    assert!(ProfileName::parse("ui").is_err());
}

#[test]
fn columns_a_click_on_a_library_box_acts_on_the_kit_the_screen_shows() {
    let b = Fake::real();
    let mut s = writing_in_library(&b);
    // Back to column 2, onto base: columns 3 and 4 now show base, not writing.
    press(&mut s, &b, &[Left, Left, Up]);
    let out = screen(&s);
    assert!(steps(&out).contains("3 in base"), "{out}");
    // The box is the four cells before the row's ▸, drawn or blank.
    let click_box = |s: &mut State, out: &str| {
        let (row, line) = out.lines().enumerate().find(|(_, l)| l.contains("▸ writing/")).expect(out);
        let col = line.find("▸ writing/").unwrap();
        s.handle(&b, Key::Click(line[..col].chars().count() as u16 - 4, row as u16));
    };
    click_box(&mut s, &out);
    assert_eq!(b.uses("base"), ["skills/writing/"], "the click changes base, the kit on screen");
    assert_eq!(b.uses("writing"), ["skills/writing/"], "writing, opened earlier, is not touched");
    // From column 1 the library shows no kit, so a box click changes nothing.
    let mut s = writing_in_library(&b);
    press(&mut s, &b, &[Left, Left, Left]);
    let out = screen(&s);
    assert!(steps(&out).contains("1 claude-bedrock-gary") && !steps(&out).contains("3 in"), "{out}");
    click_box(&mut s, &out);
    assert_eq!(b.uses("writing"), ["skills/writing/"], "no kit is shown, so no kit changes");
}

#[test]
fn columns_a_click_on_a_contents_row_opens_the_kit_the_screen_shows() {
    let b = Fake::real();
    let mut s = writing_in_library(&b);
    // Back to column 2, onto base: column 3 shows base's instructions, not writing's lines.
    press(&mut s, &b, &[Left, Left, Up]);
    let out = screen(&s);
    // The plan under column 1 can show the same words; column 3 is the rightmost.
    let (row, line) = out.lines().enumerate().find(|(_, l)| l.contains("instructions (CLAUDE.md)")).expect(&out);
    let col = line.rfind("instructions (CLAUDE.md)").unwrap();
    s.handle(&b, Key::Click(line[..col].chars().count() as u16, row as u16));
    assert!(steps(&screen(&s)).contains("3 in base"), "the click opens base, the kit on screen:\n{}", screen(&s));
    press(&mut s, &b, &[Char('e')]);
    s.run_editor(&b);
    assert_eq!(b.called(), ["edit_kit base"], "not writing, the kit opened earlier");
}

#[test]
fn columns_a_click_in_a_scrolled_import_list_imports_the_line_it_hits() {
    let b = Fake::real();
    let mut s = State::new(&b);
    // ~/.claude, aws dev, aws sso, every AWS: the cursor on sso, in a box two lines high.
    press(&mut s, &b, &[Char('i'), Down, Down]);
    let (w, h) = (120, 8);
    let out = screen_at(&s, w, h);
    assert!(!out.contains("~/.claude"), "the list scrolled past its first item:\n{out}");
    let (row, line) = out.lines().enumerate().find(|(_, l)| l.contains("aws dev")).expect(&out);
    let col = line.find("aws dev").unwrap();
    s.handle(&b, Key::Click(line[..col].chars().count() as u16, row as u16));
    assert!(b.called().contains(&"import_aws dev false".to_string()), "the click imports the line it hits: {:?}", b.called());
    assert!(!b.called().contains(&"import_claude".to_string()), "not the item at that line before the scroll: {:?}", b.called());
}
