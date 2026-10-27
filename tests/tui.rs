//! Gate for the TUI (ADR-2610261255; rows, trees and keys from ADR-2610261643 §8).
//! Driven with keys, drawn into a TestBackend — no terminal needed. A fake
//! `Browser` keeps the library, kits and attachments, and records each call.

use ccs::adapters::primary::tui::{draw, draw_picker, Exit, Key, Picker, State};
use ccs::domain::{AwsRow, CcsError, Finding, ImportOutcome, Kind, LibItem, LibraryRemoved, ListEntry, Overview, ProfileName, Use};
use ccs::ports::Browser;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn name(s: &str) -> ProfileName {
    ProfileName::parse(s).unwrap()
}

fn item(kind: Kind, folder: &str, n: &str) -> LibItem {
    LibItem { kind, name: n.into(), folder: folder.into(), description: Some(format!("about {n}")), source: PathBuf::from(format!("/lib/{folder}/{n}")) }
}

struct Fake {
    profiles: RefCell<Vec<String>>,
    old_layout: RefCell<bool>,
    default: RefCell<Option<String>>,
    attached: RefCell<BTreeMap<String, Vec<String>>>,
    kits: RefCell<BTreeMap<String, Vec<Use>>>,
    instructions: BTreeMap<String, String>,
    items: RefCell<Vec<LibItem>>,
    folders: RefCell<Vec<(Kind, String)>>,
    calls: RefCell<Vec<String>>,
}

impl Fake {
    /// Profiles `lab` and `work` (work has kit net); kits `eng` and `net`
    /// (net picks skills/net/); skills net/nqe/find-nqe-query, net/inspect-edge,
    /// humanize; agent hex/rev.
    fn new() -> Self {
        Self::with(&["lab", "work"])
    }
    fn with(profiles: &[&'static str]) -> Self {
        let mut kits = BTreeMap::new();
        kits.insert("eng".to_string(), vec![]);
        kits.insert("net".to_string(), vec![Use::Folder(Kind::Skill, "net".into())]);
        let mut attached = BTreeMap::new();
        attached.insert("work".to_string(), vec!["net".to_string()]);
        Fake {
            profiles: RefCell::new(profiles.iter().map(|p| p.to_string()).collect()),
            old_layout: RefCell::new(false),
            default: RefCell::new(None),
            attached: RefCell::new(attached),
            kits: RefCell::new(kits),
            instructions: [("net".to_string(), "NET RULES".to_string())].into_iter().collect(),
            items: RefCell::new(vec![
                item(Kind::Skill, "", "humanize"),
                item(Kind::Skill, "net", "inspect-edge"),
                item(Kind::Skill, "net/nqe", "find-nqe-query"),
                item(Kind::Agent, "hex", "rev"),
                item(Kind::Plugin, "", "egm"),
            ]),
            folders: RefCell::new(vec![(Kind::Skill, "net".into()), (Kind::Skill, "net/nqe".into()), (Kind::Agent, "hex".into())]),
            calls: RefCell::new(vec![]),
        }
    }
    /// Adds `n` skills `s000…` in folder `bulk`, for scrolling.
    fn bulk(self, n: usize) -> Self {
        self.folders.borrow_mut().push((Kind::Skill, "bulk".into()));
        self.items.borrow_mut().extend((0..n).map(|i| item(Kind::Skill, "bulk", &format!("s{i:03}"))));
        self
    }
    /// Adds `n` empty kits `k00…`, which sort between eng and net, for scrolling.
    fn many_kits(self, n: usize) -> Self {
        self.kits.borrow_mut().extend((0..n).map(|i| (format!("k{i:02}"), vec![])));
        self
    }
    /// An earlier ccs left its layout, so the import list offers migrate.
    fn old_layout(self) -> Self {
        *self.old_layout.borrow_mut() = true;
        self
    }
    fn no_kits(self) -> Self {
        self.kits.borrow_mut().clear();
        self.attached.borrow_mut().clear();
        self
    }
    fn called(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
    fn log(&self, s: String) {
        self.calls.borrow_mut().push(s);
    }
}

impl Browser for Fake {
    fn profiles(&self) -> Vec<ListEntry> {
        self.profiles
            .borrow()
            .iter()
            .map(|p| ListEntry {
                name: name(p),
                current: false,
                is_default: self.default.borrow().as_deref() == Some(p.as_str()),
                kits: self.attached.borrow().get(p).cloned().unwrap_or_default(),
                provider: if p == "lab" { "anthropic".into() } else { "bedrock sso@us-east-1".into() },
            })
            .collect()
    }
    fn overview(&self, n: &ProfileName) -> Result<Overview, CcsError> {
        Ok(Overview {
            name: n.clone(),
            segment: format!("⬡ {} · bedrock sso@us-east-1", n.as_str()),
            env: vec![("AWS_PROFILE".into(), "sso".into())],
            findings: vec![Finding::Pass("config dir ok".into()), Finding::Fail("claude is not on PATH".into())],
            is_default: false,
            kits: self.attached.borrow().get(n.as_str()).cloned().unwrap_or_default(),
            dir: PathBuf::from(format!("/profiles/{}", n.as_str())),
        })
    }
    fn set_default(&self, n: &ProfileName) -> Result<(), CcsError> {
        *self.default.borrow_mut() = Some(n.as_str().into());
        Ok(())
    }
    fn kits(&self) -> Vec<String> {
        self.kits.borrow().keys().cloned().collect()
    }
    fn attach(&self, kit: &str, profile: &ProfileName, on: bool) -> Result<(), CcsError> {
        self.log(format!("attach {kit} {} {on}", profile.as_str()));
        let mut a = self.attached.borrow_mut();
        let list = a.entry(profile.as_str().into()).or_default();
        list.retain(|k| k != kit);
        if on {
            list.push(kit.into());
        }
        Ok(())
    }
    fn new_kit(&self, kit: &str) -> Result<(), CcsError> {
        if kit == "Bad" {
            return Err(CcsError::Usage("bad kit name 'Bad'".into()));
        }
        self.log(format!("new_kit {kit}"));
        self.kits.borrow_mut().insert(kit.into(), vec![]);
        Ok(())
    }
    fn remove_kit(&self, kit: &str) -> Result<PathBuf, CcsError> {
        self.log(format!("remove_kit {kit}"));
        self.kits.borrow_mut().remove(kit);
        Ok(PathBuf::from(format!("/t/.trash/1-kit-{kit}")))
    }
    fn kit_uses(&self, kit: &str) -> Result<Vec<Use>, CcsError> {
        Ok(self.kits.borrow().get(kit).cloned().unwrap_or_default())
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
        self.instructions.get(kit).cloned()
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
        if entry.line().starts_with("skills/s0") {
            return Ok((1..=200).map(|i| format!("line {i}\n")).collect());
        }
        Ok(format!("---\ndescription: x\n---\nBody of {}\n", entry.line()))
    }
    fn edit(&self, entry: &Use) -> Result<(), CcsError> {
        self.log(format!("edit {}", entry.line()));
        Ok(())
    }
    fn add(&self, path: &str, folder: &str) -> Result<(Kind, String), CcsError> {
        if path == "bad" {
            return Err(CcsError::Usage("bad is neither a folder holding SKILL.md nor an agent .md file".into()));
        }
        self.log(format!("add {path} in '{folder}'"));
        let n = path.trim_end_matches('/').rsplit('/').next().unwrap_or(path).to_string();
        self.items.borrow_mut().push(item(Kind::Skill, folder, &n));
        Ok((Kind::Skill, n))
    }
    fn mkdir(&self, kind: Kind, folder: &str) -> Result<(), CcsError> {
        self.log(format!("mkdir {folder}"));
        self.folders.borrow_mut().push((kind, folder.into()));
        Ok(())
    }
    fn move_entry(&self, entry: &Use, to: &str) -> Result<(), CcsError> {
        self.log(format!("move {} to '{to}'", entry.line()));
        Ok(())
    }
    fn remove(&self, entry: &Use) -> Result<LibraryRemoved, CcsError> {
        self.log(format!("remove {}", entry.line()));
        if let Use::Item(_, n) = entry {
            self.items.borrow_mut().retain(|i| i.name != *n);
        }
        Ok(LibraryRemoved { trashed: PathBuf::from("/t/.trash/1-library-skill-x"), kits: vec!["net".into()] })
    }
    fn rename_kit(&self, old: &str, new: &str) -> Result<(), CcsError> {
        self.log(format!("rename_kit {old} {new}"));
        let uses = self.kits.borrow_mut().remove(old).unwrap_or_default();
        self.kits.borrow_mut().insert(new.into(), uses);
        Ok(())
    }
    fn rename_folder(&self, kind: Kind, folder: &str, new: &str) -> Result<(), CcsError> {
        self.log(format!("rename_folder {} {folder} {new}", kind.dir()));
        Ok(())
    }
    fn set_kits(&self, profile: &ProfileName, kits: &[String]) -> Result<(), CcsError> {
        self.log(format!("set_kits {} {}", profile.as_str(), kits.join(",")));
        self.attached.borrow_mut().insert(profile.as_str().into(), kits.to_vec());
        Ok(())
    }
    fn complete_path(&self, typed: &str) -> Vec<String> {
        ["~/skills/", "~/src/a/", "~/src/b/", "~/notes.md"].iter().filter(|c| c.starts_with(typed)).map(|c| c.to_string()).collect()
    }
    fn update_plugins(&self, which: Option<&str>) -> Result<Vec<String>, CcsError> {
        self.log(format!("update_plugins {}", which.unwrap_or("*")));
        Ok(vec!["plugins/egm: up to date at 3d0b348".into()])
    }
    fn new_profile(&self, n: &str, from: Option<&ProfileName>) -> Result<(), CcsError> {
        if ProfileName::parse(n).is_err() {
            return Err(CcsError::Usage(format!("bad profile name '{n}'")));
        }
        if self.profiles.borrow().iter().any(|p| p == n) {
            return Err(CcsError::Exists(n.into()));
        }
        self.log(format!("new_profile {n} {}", from.map_or("-", |f| f.as_str())));
        if let Some(kits) = from.and_then(|f| self.attached.borrow().get(f.as_str()).cloned()) {
            self.attached.borrow_mut().insert(n.into(), kits);
        }
        let mut all = self.profiles.borrow_mut();
        all.push(n.into());
        all.sort();
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
        Ok(vec![AwsRow { aws: "dev".into(), imported_as: None }, AwsRow { aws: "sso".into(), imported_as: Some("work".into()) }])
    }
    fn import_aws(&self, names: &[String], all: bool) -> Result<Vec<ImportOutcome>, CcsError> {
        self.log(format!("import_aws {} {all}", names.join(",")));
        let mut out = vec![];
        for n in if all { vec!["dev".to_string(), "sso".to_string()] } else { names.to_vec() } {
            if n == "sso" {
                out.push(ImportOutcome::Skipped { aws: n, reason: "ccs profile 'work' already exists".into() });
            } else {
                self.profiles.borrow_mut().push(n.clone());
                out.push(ImportOutcome::Created { name: n, region: Some("us-east-1".into()) });
            }
        }
        Ok(out)
    }
}

fn screen(state: &State) -> String {
    let mut t = Terminal::new(TestBackend::new(120, 20)).unwrap();
    t.draw(|f| draw(f, state)).unwrap();
    let buf = t.backend().buffer();
    let w = buf.area.width as usize;
    buf.content().chunks(w).map(|row| row.iter().map(|c| c.symbol()).collect::<String>()).collect::<Vec<_>>().join("\n")
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

use Key::{Char, Down, Enter, Esc, Right, Tab, Up};

/// Profiles: lab, work. Downs from the top.
const WORK: usize = 1;

fn at(row: usize) -> State {
    let b = Fake::new();
    let mut s = State::new(&b);
    for _ in 0..row {
        s.handle(&b, Down);
    }
    s
}

/// The screen line holding `text`.
fn line_with<'a>(screen: &'a str, text: &str) -> &'a str {
    screen.lines().find(|l| l.contains(text)).unwrap_or_else(|| panic!("no line with {text:?} in:\n{screen}"))
}

// ── The list ─────────────────────────────────────────────────────────────────

#[test]
fn the_profiles_section_lists_profiles_with_their_kits() {
    let b = Fake::new();
    let s = State::new(&b);
    let out = screen(&s);
    let order: Vec<usize> = ["▶ lab", "work +net"].iter().map(|w| out.find(w).unwrap_or_else(|| panic!("{w}\n{out}"))).collect();
    assert!(order.windows(2).all(|p| p[0] < p[1]), "{out}");
}

#[test]
fn moving_stops_at_the_ends() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Up, Up]);
    assert_eq!(s.selected_profile(), Some("lab"));
    for _ in 0..20 {
        s.handle(&b, Down);
    }
    assert_eq!(s.selected_profile(), Some("work"), "the end of the section");
}

#[test]
fn q_and_esc_quit() {
    let b = Fake::new();
    assert_eq!(press(&mut State::new(&b), &b, &[Char('q')]), Some(Exit::Quit));
    assert_eq!(press(&mut State::new(&b), &b, &[Esc]), Some(Exit::Quit));
}

#[test]
fn search_filters_profiles_and_enter_launches_the_match() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('/')]);
    typed(&mut s, &b, "wor");
    press(&mut s, &b, &[Enter]);
    assert!(!screen(&s).contains("lab"), "{}", screen(&s));
    assert_eq!(press(&mut s, &b, &[Enter]), Some(Exit::Launch("work".into())));
}

#[test]
fn no_profiles_says_how_to_start_and_enter_launches_nothing() {
    let b = Fake::with(&[]);
    let mut s = State::new(&b);
    let out = screen(&s);
    assert!(out.contains("none yet · n makes one"), "{out}");
    assert_eq!(press(&mut s, &b, &[Enter]), None);
}

// ── A profile ────────────────────────────────────────────────────────────────

#[test]
fn enter_launches_a_profile_and_d_sets_the_default() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Down, Char('d')]);
    assert_eq!(b.default.borrow().as_deref(), Some("work"));
    assert_eq!(press(&mut s, &b, &[Enter]), Some(Exit::Launch("work".into())));
}

#[test]
fn the_overview_shows_segment_variables_kits_and_doctor() {
    let s = at(WORK);
    let out = screen(&s);
    for want in ["⬡ work · bedrock sso@us-east-1", "AWS_PROFILE=sso", "Kits", "+net", "✓ config dir ok", "✗ claude is not on PATH"] {
        assert!(out.contains(want), "missing {want:?}:\n{out}");
    }
}

#[test]
fn the_kits_tab_attaches_and_detaches_with_space() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Down, Right, Tab]);
    let out = screen(&s);
    assert!(out.contains("[x] net") && out.contains("[ ] eng"), "{out}");
    press(&mut s, &b, &[Char(' ')]);
    assert_eq!(b.called(), ["attach eng work true"]);
    assert!(screen(&s).contains("work +net +eng"), "{}", screen(&s));
    press(&mut s, &b, &[Down, Char(' ')]);
    assert_eq!(b.called().last().unwrap(), "attach net work false");
    assert!(line_with(&screen(&s), "work").contains("+eng") && !line_with(&screen(&s), "work").contains("+net"));
}

// ── A kit ────────────────────────────────────────────────────────────────────

#[test]
fn a_kit_shows_the_library_tree_with_its_picks() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('2'), Down]);
    let out = screen(&s);
    assert!(line_with(&out, "net/").contains("[x]"), "net/ is picked:\n{out}");
    assert!(line_with(&out, "nqe/").contains("[·]"), "nqe/ is picked through net/:\n{out}");
    assert!(line_with(&out, "find-nqe-query").contains("[·]"), "{out}");
    assert!(line_with(&out, "humanize").contains("[ ]"), "{out}");
    let (net, nqe, find) = (out.find("net/").unwrap(), out.find("nqe/").unwrap(), out.find("find-nqe-query").unwrap());
    assert!(net < nqe && nqe < find, "folders come before what is in them:\n{out}");
}

#[test]
fn space_picks_an_item_or_a_folder_and_explains_a_pick_from_above() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('2'), Down, Tab]);
    // Tree rows: humanize, net/, inspect-edge, nqe/, find-nqe-query.
    press(&mut s, &b, &[Char(' ')]);
    assert_eq!(b.called(), ["set_use net skills/humanize true"]);
    assert!(line_with(&screen(&s), "humanize").contains("[x]"));
    press(&mut s, &b, &[Down, Down, Char(' ')]);
    assert!(screen(&s).contains("picked by skills/net/"), "{}", screen(&s));
    assert_eq!(b.called().len(), 1, "a pick from above changes nothing");
    press(&mut s, &b, &[Up, Char(' ')]);
    assert_eq!(b.called().last().unwrap(), "set_use net skills/net/ false");
    assert!(line_with(&screen(&s), "find-nqe-query").contains("[ ]"), "{}", screen(&s));
}

#[test]
fn a_kit_claude_md_tab_shows_the_instructions_and_e_edits_them() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('2'), Down, Right, Right, Right]);
    assert!(screen(&s).contains("NET RULES"), "{}", screen(&s));
    press(&mut s, &b, &[Char('e')]);
    assert!(s.wants_editor());
    assert!(b.called().is_empty(), "the editor never runs inside handle()");
    s.run_editor(&b);
    assert_eq!(b.called(), ["edit_kit net"]);
    // A kit with no instructions says how to start them.
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('2'), Right, Right, Right]);
    assert!(screen(&s).contains("e writes"), "{}", screen(&s));
}

#[test]
fn n_makes_a_kit_from_the_heading_and_x_deletes_one_after_asking() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('2'), Char('n')]);
    typed(&mut s, &b, "write");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called(), ["new_kit write"]);
    assert!(screen(&s).contains("▶ write"), "the new kit is selected:\n{}", screen(&s));
    press(&mut s, &b, &[Home, Char('x')]);
    assert!(screen(&s).contains("Delete kit eng? y/n"), "{}", screen(&s));
    press(&mut s, &b, &[Char('n')]);
    assert_eq!(b.called().len(), 1);
    press(&mut s, &b, &[Char('x'), Char('y')]);
    assert_eq!(b.called().last().unwrap(), "remove_kit eng");
    assert!(screen(&s).contains(".trash"), "{}", screen(&s));
    // A bad name is reported, not swallowed.
    press(&mut s, &b, &[Char('n')]);
    typed(&mut s, &b, "Bad");
    press(&mut s, &b, &[Enter]);
    assert!(screen(&s).contains("bad kit name"), "{}", screen(&s));
}

// ── The library ──────────────────────────────────────────────────────────────

fn library_skills(b: &Fake) -> State {
    let mut s = State::new(b);
    s.handle(b, Char('3'));
    s.handle(b, Tab);
    s
}

#[test]
fn the_library_tree_has_no_marks_and_shows_descriptions() {
    let b = Fake::new();
    let s = library_skills(&b);
    let out = screen(&s);
    assert!(line_with(&out, "inspect-edge").contains("about inspect-edge"), "{out}");
    assert!(!out.contains("[x]") && !out.contains("[ ]"), "{out}");
}

#[test]
fn e_and_v_act_on_items_not_folders() {
    let b = Fake::new();
    let mut s = library_skills(&b);
    press(&mut s, &b, &[Char('e')]);
    s.run_editor(&b);
    assert_eq!(b.called(), ["edit skills/humanize"]);
    press(&mut s, &b, &[Down, Char('e')]);
    assert!(!s.wants_editor(), "a folder has no file");
    press(&mut s, &b, &[Down, Char('v')]);
    assert!(screen(&s).contains("Body of skills/inspect-edge"), "{}", screen(&s));
    assert_eq!(press(&mut s, &b, &[Esc]), None, "esc leaves the view, not the TUI");
}

#[test]
fn a_adds_from_a_path_into_the_highlighted_folder_with_completion() {
    let b = Fake::new();
    let mut s = library_skills(&b);
    press(&mut s, &b, &[Down, Down, Char('a')]);
    assert!(screen(&s).contains("into skills/net/"), "the item's folder is the target:\n{}", screen(&s));
    typed(&mut s, &b, "~/s");
    press(&mut s, &b, &[Tab]);
    let out = screen(&s);
    assert!(out.contains("~/s") && out.contains("~/skills/") && out.contains("~/src/a/"), "several matches are listed:\n{out}");
    typed(&mut s, &b, "rc/a");
    press(&mut s, &b, &[Tab, Enter]);
    assert_eq!(b.called(), ["add ~/src/a/ in 'net'"]);
    assert!(screen(&s).contains("added skills/a"), "{}", screen(&s));
    // q is text while typing, and esc adds nothing.
    press(&mut s, &b, &[Char('a'), Char('q'), Esc]);
    assert_eq!(b.called().len(), 1);
}

#[test]
fn a_failed_add_shows_why() {
    let b = Fake::new();
    let mut s = library_skills(&b);
    press(&mut s, &b, &[Char('a')]);
    typed(&mut s, &b, "bad");
    press(&mut s, &b, &[Enter]);
    assert!(screen(&s).contains("neither a folder holding SKILL.md"), "{}", screen(&s));
}

#[test]
fn n_makes_a_folder_inside_the_highlighted_one_and_m_moves() {
    let b = Fake::new();
    let mut s = library_skills(&b);
    press(&mut s, &b, &[Down, Char('n')]);
    typed(&mut s, &b, "edge");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called(), ["mkdir net/edge"]);
    press(&mut s, &b, &[Up, Char('m')]);
    typed(&mut s, &b, "net");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called().last().unwrap(), "move skills/humanize to 'net'");
    press(&mut s, &b, &[Down, Char('m')]);
    typed(&mut s, &b, "/");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called().last().unwrap(), "move skills/net/ to ''", "a folder moves whole; / is the top");
}

#[test]
fn x_deletes_after_asking_and_names_the_kits_that_used_it() {
    let b = Fake::new();
    let mut s = library_skills(&b);
    press(&mut s, &b, &[Char('x')]);
    assert!(screen(&s).contains("Delete skills/humanize from the library? y/n"), "{}", screen(&s));
    press(&mut s, &b, &[Char('n')]);
    assert!(b.called().is_empty());
    press(&mut s, &b, &[Char('x'), Char('y')]);
    assert_eq!(b.called(), ["remove skills/humanize"]);
    let out = screen(&s);
    assert!(out.contains(".trash") && out.contains("net"), "{out}");
    assert!(!out.contains("about humanize"), "gone from the tree:\n{out}");
}

#[test]
fn the_agents_tab_lists_agents() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('3'), Down]);
    let out = screen(&s);
    assert!(out.contains("hex/") && out.contains("rev"), "{out}");
    assert!(!out.contains("humanize"));
}

#[test]
fn search_in_a_tree_keeps_matching_items_and_their_folders() {
    let b = Fake::new();
    let mut s = library_skills(&b);
    press(&mut s, &b, &[Char('/')]);
    typed(&mut s, &b, "nqe-q");
    press(&mut s, &b, &[Enter]);
    let out = screen(&s);
    assert!(out.contains("find-nqe-query") && out.contains("nqe/") && out.contains("net/"), "{out}");
    assert!(!out.contains("humanize") && !out.contains("inspect-edge"), "{out}");
}

#[test]
fn the_footer_shows_the_keys_for_where_you_are() {
    let b = Fake::new();
    assert!(screen(&State::new(&b)).contains("⏎ launch"));
    assert!(screen(&library_skills(&b)).contains("a add"));
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('2'), Down, Tab]);
    assert!(screen(&s).contains("space pick"), "{}", screen(&s));
}

#[test]
fn ui_is_a_command_and_a_reserved_name() {
    assert!(ProfileName::parse("ui").is_err());
}

// ── Organizing (ADR-2610261643 §9) ───────────────────────────────────────────
// Library skills tree rows: humanize, net/, inspect-edge, nqe/, find-nqe-query.

#[test]
fn space_marks_library_rows_and_m_moves_them_through_a_folder_picker() {
    let b = Fake::new();
    let mut s = library_skills(&b);
    press(&mut s, &b, &[Char(' '), Down, Down, Char(' ')]);
    assert_eq!(screen(&s).matches('●').count(), 2, "{}", screen(&s));
    press(&mut s, &b, &[Char('m')]);
    let out = screen(&s);
    assert!(out.contains("/ (top)") && out.contains("net/nqe"), "the picker lists the folders:\n{out}");
    press(&mut s, &b, &[Down, Enter]);
    assert_eq!(b.called(), ["move skills/humanize to 'net'", "move skills/inspect-edge to 'net'"]);
    assert_eq!(screen(&s).matches('●').count(), 0, "marks clear after a move");
    assert!(screen(&s).contains("moved 2"), "{}", screen(&s));
}

#[test]
fn the_folder_picker_filters_as_you_type_and_makes_a_new_folder() {
    let b = Fake::new();
    let mut s = library_skills(&b);
    press(&mut s, &b, &[Char('m')]);
    typed(&mut s, &b, "nq");
    assert!(!screen(&s).contains("/ (top)"), "{}", screen(&s));
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called(), ["move skills/humanize to 'net/nqe'"]);
    press(&mut s, &b, &[Char('m')]);
    typed(&mut s, &b, "tools");
    assert!(screen(&s).contains("+ new folder: tools"), "{}", screen(&s));
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called().last().unwrap(), "move skills/humanize to 'tools'");
    // A folder is not offered as a place to move itself into.
    press(&mut s, &b, &[Down, Char('m')]);
    typed(&mut s, &b, "net");
    assert!(!screen(&s).contains("net/nqe"), "{}", screen(&s));
    press(&mut s, &b, &[Esc]);
}

#[test]
fn plus_adds_marked_rows_to_a_kit_or_to_a_new_one() {
    let b = Fake::new();
    let mut s = library_skills(&b);
    press(&mut s, &b, &[Char(' '), Down, Char(' '), Char('+')]);
    typed(&mut s, &b, "en");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called(), ["set_use eng skills/humanize true", "set_use eng skills/net/ true"]);
    assert!(screen(&s).contains("eng now picks 2"), "{}", screen(&s));
    press(&mut s, &b, &[Up, Char('+')]);
    typed(&mut s, &b, "write");
    assert!(screen(&s).contains("+ new kit: write"), "{}", screen(&s));
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called()[2..], ["new_kit write".to_string(), "set_use write skills/humanize true".to_string()]);
}

#[test]
fn x_with_marks_deletes_them_all_after_one_question() {
    let b = Fake::new();
    let mut s = library_skills(&b);
    press(&mut s, &b, &[Char(' '), Down, Down, Char(' '), Char('x')]);
    assert!(screen(&s).contains("Delete 2 entries from the library? y/n"), "{}", screen(&s));
    press(&mut s, &b, &[Char('y')]);
    assert_eq!(b.called(), ["remove skills/humanize", "remove skills/inspect-edge"]);
}

#[test]
fn library_rows_show_the_kits_that_pick_them() {
    let b = Fake::new();
    let out = screen(&library_skills(&b));
    assert!(line_with(&out, "inspect-edge").contains("‹net›"), "picked through net/:\n{out}");
    assert!(line_with(&out, "net/").contains("‹net›"), "{out}");
    assert!(!line_with(&out, "humanize").contains('‹'), "{out}");
}

#[test]
fn r_renames_a_folder_or_a_kit() {
    let b = Fake::new();
    let mut s = library_skills(&b);
    press(&mut s, &b, &[Down, Char('r')]);
    typed(&mut s, &b, "ops");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called(), ["rename_folder skills net ops"]);
    press(&mut s, &b, &[Up, Char('r')]);
    assert_eq!(b.called().len(), 1, "an item is not renamed");
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('2'), Char('r')]);
    typed(&mut s, &b, "engineering");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called().last().unwrap(), "rename_kit eng engineering");
    assert!(screen(&s).contains("engineering"), "{}", screen(&s));
}

#[test]
fn a_kit_attaches_to_profiles_from_its_profiles_tab() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('2'), Down, Right, Right, Right, Right, Tab]);
    let out = screen(&s);
    assert!(out.contains("[ ] lab") && out.contains("[x] work"), "{out}");
    press(&mut s, &b, &[Char(' ')]);
    assert_eq!(b.called(), ["attach net lab true"]);
    assert!(screen(&s).contains("[x] lab"), "{}", screen(&s));
}

#[test]
fn p_shows_only_what_a_kit_picks_and_tabs_count() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('2'), Down]);
    assert!(screen(&s).contains("Skills (2)"), "inspect-edge and find-nqe-query:\n{}", screen(&s));
    press(&mut s, &b, &[Tab, Char('p')]);
    let out = screen(&s);
    assert!(out.contains("inspect-edge") && out.contains("find-nqe-query") && !out.contains("humanize"), "{out}");
    press(&mut s, &b, &[Char('p')]);
    assert!(screen(&s).contains("humanize"));
    assert!(screen(&library_skills(&b)).contains("Skills (3)"));
}

// ── The launch screen (ADR-2610261643 §10) ──────────────────────────────────

fn picker_screen(p: &Picker) -> String {
    let mut t = Terminal::new(TestBackend::new(120, 14)).unwrap();
    t.draw(|f| draw_picker(f, p)).unwrap();
    let buf = t.backend().buffer();
    let w = buf.area.width as usize;
    buf.content().chunks(w).map(|row| row.iter().map(|c| c.symbol()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

fn keys(p: &mut Picker, b: &dyn Browser, ks: &[Key]) -> Option<Exit> {
    let mut last = None;
    for k in ks {
        last = p.handle(b, *k);
        if last.is_some() {
            break;
        }
    }
    last
}

#[test]
fn the_launch_screen_preselects_the_resolved_profile_and_ticks_its_kits() {
    let b = Fake::new();
    let p = Picker::new(&b, Some("work"));
    let out = picker_screen(&p);
    assert!(line_with(&out, "▶ work").contains("bedrock sso@us-east-1"), "the provider shows:\n{out}");
    assert!(out.contains("anthropic"), "{out}");
    assert!(line_with(&out, "[x] net").contains('2'), "net picks 2 items:\n{out}");
    assert!(out.contains("[ ] eng"), "{out}");
}

#[test]
fn moving_to_another_profile_ticks_that_profiles_kits() {
    let b = Fake::new();
    let mut p = Picker::new(&b, Some("work"));
    keys(&mut p, &b, &[Up]);
    let out = picker_screen(&p);
    assert!(out.contains("▶ lab") && out.contains("[ ] net"), "{out}");
    let p = Picker::new(&b, None);
    assert!(picker_screen(&p).contains("▶ lab"), "with nothing resolved, the first profile");
}

#[test]
fn enter_launches_with_the_ticked_kits_for_this_launch_only() {
    let b = Fake::new();
    let mut p = Picker::new(&b, Some("work"));
    let exit = keys(&mut p, &b, &[Tab, Char(' '), Enter]);
    assert_eq!(exit, Some(Exit::LaunchWith { profile: "work".into(), kits: vec!["net".into(), "eng".into()] }), "defaults first, in order");
    assert!(b.called().is_empty(), "nothing saved");
    let mut p = Picker::new(&b, Some("work"));
    let exit = keys(&mut p, &b, &[Tab, Down, Char(' '), Enter]);
    assert_eq!(exit, Some(Exit::LaunchWith { profile: "work".into(), kits: vec![] }), "no kits is a choice too");
    let mut p = Picker::new(&b, Some("work"));
    assert_eq!(keys(&mut p, &b, &[Enter]), Some(Exit::LaunchWith { profile: "work".into(), kits: vec!["net".into()] }), "one Enter");
}

#[test]
fn s_saves_the_ticked_kits_as_the_profiles_defaults() {
    let b = Fake::new();
    let mut p = Picker::new(&b, Some("work"));
    keys(&mut p, &b, &[Tab, Char(' '), Char('s')]);
    assert_eq!(b.called(), ["set_kits work net,eng"]);
    assert!(picker_screen(&p).contains("work now starts with +net +eng"), "{}", picker_screen(&p));
    assert_eq!(keys(&mut p, &b, &[Enter]), Some(Exit::LaunchWith { profile: "work".into(), kits: vec!["net".into(), "eng".into()] }));
}

#[test]
fn the_launch_screen_with_no_profiles_says_how_to_start_and_q_quits() {
    let b = Fake::with(&[]);
    let mut p = Picker::new(&b, None);
    assert!(picker_screen(&p).contains("ccs new <name>"), "{}", picker_screen(&p));
    assert_eq!(keys(&mut p, &b, &[Enter]), None);
    assert_eq!(keys(&mut p, &b, &[Char('q')]), Some(Exit::Quit));
    let b = Fake::new();
    assert_eq!(keys(&mut Picker::new(&b, None), &b, &[Esc]), Some(Exit::Quit));
}

// ── Scrolling (ADR-2610261643 §11) ───────────────────────────────────────────
// Library skills with bulk(60), in path order: bulk/, s000…s059, humanize, net/, inspect-edge, nqe/,
// find-nqe-query = 66 rows.

use Key::{End, Home, PageDown, WheelDown, WheelUp};

/// The first body row on screen: the line just under the body's top border.
fn first_body_row(out: &str) -> String {
    let lines: Vec<&str> = out.lines().collect();
    let top = lines.iter().position(|l| l.contains("┌") && l.matches('┌').count() == 2).unwrap_or(1);
    lines[top + 1].chars().skip(31).collect::<String>().trim().to_string()
}

#[test]
fn page_down_moves_a_page_and_end_and_home_reach_the_ends() {
    let b = Fake::new().bulk(60);
    let mut s = library_skills(&b);
    screen(&s); // the page height comes from the last frame, as in a real terminal
    press(&mut s, &b, &[PageDown]);
    let out = screen(&s);
    assert!(!out.contains("s000"), "a page down scrolls the first rows away:\n{out}");
    press(&mut s, &b, &[End]);
    let out = screen(&s);
    assert!(out.contains("find-nqe-query") && out.contains("66/66"), "{out}");
    press(&mut s, &b, &[Home]);
    assert!(screen(&s).contains("s000") && screen(&s).contains("1/66"));
    press(&mut s, &b, &[Char('G')]);
    assert!(screen(&s).contains("66/66"));
    press(&mut s, &b, &[Char('g')]);
    assert!(screen(&s).contains("1/66"));
}

#[test]
fn moving_up_one_row_does_not_jump_the_view() {
    let b = Fake::new().bulk(60);
    let mut s = library_skills(&b);
    for _ in 0..30 {
        s.handle(&b, Down);
    }
    let before = first_body_row(&screen(&s));
    press(&mut s, &b, &[Up]);
    assert_eq!(first_body_row(&screen(&s)), before, "{}", screen(&s));
    assert!(screen(&s).contains("30/66"));
}

#[test]
fn the_wheel_moves_three_rows_in_the_focused_pane() {
    let b = Fake::new().bulk(60);
    let mut s = library_skills(&b);
    press(&mut s, &b, &[WheelDown, WheelDown]);
    assert!(screen(&s).contains("7/66"), "{}", screen(&s));
    press(&mut s, &b, &[WheelUp]);
    assert!(screen(&s).contains("4/66"), "{}", screen(&s));
}

#[test]
fn a_long_list_shows_a_scrollbar_and_a_short_one_does_not() {
    let b = Fake::new().bulk(60);
    assert!(screen(&library_skills(&b)).contains('█'), "{}", screen(&library_skills(&b)));
    let b = Fake::new();
    assert!(!screen(&library_skills(&b)).contains('█'));
}

#[test]
fn the_view_pages_through_long_text() {
    let b = Fake::new().bulk(60);
    let mut s = library_skills(&b);
    press(&mut s, &b, &[Down, Down, Char('v')]);
    assert!(screen(&s).contains("line 1 "), "{}", screen(&s));
    press(&mut s, &b, &[PageDown]);
    let out = screen(&s);
    assert!(!out.contains("line 2 ") && out.contains("line 20"), "{out}");
    press(&mut s, &b, &[End]);
    assert!(screen(&s).contains("line 200"), "{}", screen(&s));
    press(&mut s, &b, &[Home, WheelDown]);
    assert!(screen(&s).contains("line 4") && !screen(&s).contains("line 3 "), "{}", screen(&s));
    assert_eq!(press(&mut s, &b, &[Esc]), None);
}

#[test]
fn the_launch_screen_pages_too() {
    let b = Fake::new();
    let mut p = Picker::new(&b, None);
    keys(&mut p, &b, &[End]);
    assert!(picker_screen(&p).contains("▶ work"));
    keys(&mut p, &b, &[Home]);
    assert!(picker_screen(&p).contains("▶ lab"));
}

// A ticked row below the fold is scrolled into view (ADR-2610261643 §11).
// With many_kits(30) the kits are eng, k00…k29, net: work's ticked net is row 32.

#[test]
fn the_launch_screen_shows_a_ticked_kit_below_the_fold() {
    let b = Fake::new().many_kits(30);
    let mut p = Picker::new(&b, Some("work"));
    let out = picker_screen(&p);
    assert!(out.contains("[x] net"), "net is ticked, so it is in view:\n{out}");
    keys(&mut p, &b, &[Tab]);
    let out = picker_screen(&p);
    assert!(out.contains("[x] net"), "tab puts the cursor on a row in view, so the view stays:\n{out}");
    let mut p = Picker::new(&b, Some("lab"));
    assert!(!picker_screen(&p).contains("net"), "lab ticks nothing, so the top shows");
    keys(&mut p, &b, &[Down]);
    assert!(picker_screen(&p).contains("[x] net"), "moving to work shows its kit:\n{}", picker_screen(&p));
}

#[test]
fn a_profiles_kits_tab_shows_a_ticked_kit_below_the_fold() {
    let b = Fake::new().many_kits(30);
    let mut s = State::new(&b);
    press(&mut s, &b, &[Down, Right]);
    let out = screen(&s);
    assert!(out.contains("[x] net"), "{out}");
    press(&mut s, &b, &[Tab]);
    assert!(screen(&s).contains("[x] net"), "{}", screen(&s));
}

#[test]
fn a_kits_skills_tab_shows_what_it_picks_below_the_fold() {
    let b = Fake::new().bulk(60);
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('2'), Down]);
    let out = screen(&s);
    assert!(line_with(&out, "net/").contains("[x]"), "net/ is row 62 of 66:\n{out}");
}

// ── Everything the CLI does (ADR-2610261643 §12) ─────────────────────────────

#[test]
fn n_on_a_profile_makes_one_and_selects_it() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('n')]);
    assert!(screen(&s).contains("New profile name:"), "{}", screen(&s));
    typed(&mut s, &b, "zed");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called(), ["new_profile zed -"]);
    let out = screen(&s);
    assert!(out.contains("▶ zed"), "the new profile, sorted after work, is selected:\n{out}");
    assert!(out.contains("made profile zed"), "{out}");
}

#[test]
fn c_copies_the_highlighted_profile() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Down, Char('c')]);
    assert!(screen(&s).contains("Copy work to:"), "{}", screen(&s));
    typed(&mut s, &b, "work2");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called(), ["new_profile work2 work"]);
    assert!(screen(&s).contains("work2 +net"), "a copy keeps the kits:\n{}", screen(&s));
}

#[test]
fn a_bad_or_taken_profile_name_says_why() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('n')]);
    typed(&mut s, &b, "bad name");
    press(&mut s, &b, &[Enter]);
    assert!(screen(&s).contains("bad profile name 'bad name'"), "{}", screen(&s));
    press(&mut s, &b, &[Char('n')]);
    typed(&mut s, &b, "work");
    press(&mut s, &b, &[Enter]);
    assert!(screen(&s).contains("profile 'work' already exists"), "{}", screen(&s));
    assert!(b.called().is_empty());
}

#[test]
fn e_on_a_profile_edits_its_variables() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Down, Char('e')]);
    assert!(s.wants_editor());
    s.run_editor(&b);
    assert_eq!(b.called(), ["edit_profile work"]);
    assert!(screen(&s).contains("edited the variables of work"), "{}", screen(&s));
}

#[test]
fn i_lists_what_can_be_imported() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('i')]);
    let out = screen(&s);
    assert!(out.contains("Import"), "{out}");
    assert!(out.contains("~/.claude"), "{out}");
    assert!(out.contains("aws dev"), "{out}");
    assert!(line_with(&out, "aws sso").contains("→ work"), "an imported one names its profile:\n{out}");
    assert!(out.contains("every AWS profile"), "{out}");
    assert!(!out.contains("migrate"), "nothing to migrate:\n{out}");
}

#[test]
fn i_imports_one_aws_profile_and_shows_the_result() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('i')]);
    typed(&mut s, &b, "dev");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called(), ["import_aws dev false"]);
    assert!(screen(&s).contains("✓ dev (bedrock, us-east-1)"), "{}", screen(&s));
    press(&mut s, &b, &[Esc]);
    assert!(screen(&s).contains("dev"), "the new profile is in the list:\n{}", screen(&s));
}

#[test]
fn i_imports_every_aws_profile_or_claude_home() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('i')]);
    typed(&mut s, &b, "every");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called(), ["import_aws  true"]);
    assert!(screen(&s).contains("- sso: ccs profile 'work' already exists"), "{}", screen(&s));
    press(&mut s, &b, &[Esc, Char('i')]);
    typed(&mut s, &b, "claude");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called().last().unwrap(), "import_claude");
    assert!(screen(&s).contains("already in the library"), "{}", screen(&s));
}

#[test]
fn migrate_is_offered_first_only_when_an_old_layout_is_here() {
    let b = Fake::new().old_layout();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('i')]);
    assert!(screen(&s).contains("migrate"), "{}", screen(&s));
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called(), ["migrate"]);
    assert!(screen(&s).contains("shared/ is now kit shared"), "{}", screen(&s));
}

#[test]
fn the_profile_footer_names_the_new_keys() {
    let out = screen(&at(WORK));
    for k in ["n new", "c copy", "e edit", "i import"] {
        assert!(out.contains(k), "{k}:\n{out}");
    }
}

#[test]
fn a_ticked_kit_above_the_view_is_scrolled_back_to() {
    let b = Fake::new().many_kits(30);
    b.set_kits(&name("lab"), &["eng".into()]).unwrap();
    let mut p = Picker::new(&b, Some("work"));
    assert!(!picker_screen(&p).contains("eng"), "work's net is in view, at the bottom");
    keys(&mut p, &b, &[Up]);
    assert!(picker_screen(&p).contains("[x] eng"), "lab ticks eng, row 0:\n{}", picker_screen(&p));
}

#[test]
fn with_no_profiles_n_makes_the_first_one() {
    let b = Fake::with(&[]);
    let mut s = State::new(&b);
    let out = screen(&s);
    assert!(line_with(&out, "none yet").contains('▶'), "the start is the empty section's row:\n{out}");
    assert!(out.contains("n makes one"), "{out}");
    press(&mut s, &b, &[Char('n')]);
    typed(&mut s, &b, "zed");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called(), ["new_profile zed -"], "a profile, not a kit");
    assert!(screen(&s).contains("▶ zed"), "{}", screen(&s));
}

// ── Sections: a menu bar, one list at a time (ADR-2610261643 §13) ────────────

#[test]
fn the_menu_bar_shows_one_section_at_a_time() {
    let b = Fake::new();
    let mut s = State::new(&b);
    let out = screen(&s);
    assert!(out.contains("Profiles │ Kits │ Library"), "{out}");
    assert!(out.contains("▶ lab") && !out.contains("  eng"), "profiles only:\n{out}");
    press(&mut s, &b, &[Char('2')]);
    let out = screen(&s);
    assert!(out.contains("▶ eng") && out.contains("  net") && !out.contains("lab"), "kits only:\n{out}");
    press(&mut s, &b, &[Char('3')]);
    let out = screen(&s);
    assert!(out.contains("▶ skills") && out.contains("  agents"), "the library:\n{out}");
    assert!(out.contains("humanize"), "skills shows the tree:\n{out}");
    press(&mut s, &b, &[Down]);
    assert!(screen(&s).contains("rev"), "agents shows the agents:\n{}", screen(&s));
}

#[test]
fn each_section_keeps_its_row() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('2'), Down, Char('1')]);
    assert!(screen(&s).contains("▶ lab"), "{}", screen(&s));
    press(&mut s, &b, &[Char('2')]);
    assert!(screen(&s).contains("▶ net"), "{}", screen(&s));
}

#[test]
fn n_makes_what_the_section_holds() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('2'), Char('n')]);
    assert!(screen(&s).contains("New kit name:"), "{}", screen(&s));
    press(&mut s, &b, &[Esc, Char('1'), Char('n')]);
    assert!(screen(&s).contains("New profile name:"), "{}", screen(&s));
}

#[test]
fn an_empty_kits_section_says_how_to_start() {
    let b = Fake::new().no_kits();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('2')]);
    assert!(screen(&s).contains("none yet"), "{}", screen(&s));
    press(&mut s, &b, &[Char('n')]);
    typed(&mut s, &b, "net2");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called(), ["new_kit net2"]);
}

// ── A shell in the profile's folder (ADR-2610261643 §14) ─────────────────────

#[test]
fn the_overview_offers_a_shell_in_the_profile_folder() {
    let out = screen(&at(WORK));
    assert!(line_with(&out, "/profiles/work").contains("t opens a shell here"), "{out}");
}

#[test]
fn t_on_a_profile_leaves_for_a_shell_and_does_nothing_elsewhere() {
    let b = Fake::new();
    let mut s = State::new(&b);
    assert_eq!(press(&mut s, &b, &[Down, Char('t')]), Some(Exit::Shell("work".into())));
    let mut s = State::new(&b);
    assert_eq!(press(&mut s, &b, &[Char('2'), Char('t')]), None, "a kit has no shell");
}

/// A narrow terminal stacks the list over the body instead of cutting the body off.
#[test]
fn a_narrow_terminal_stacks_the_list_over_the_body() {
    let b = Fake::new();
    let s = State::new(&b);
    let mut t = Terminal::new(TestBackend::new(30, 30)).unwrap();
    t.draw(|f| draw(f, &s)).unwrap();
    let buf = t.backend().buffer();
    let out = buf.content().chunks(30).map(|row| row.iter().map(|c| c.symbol()).collect::<String>()).collect::<Vec<_>>().join("\n");
    assert!(out.contains("Profiles │ Kits │ Library"), "the whole menu bar:\n{out}");
    assert!(out.contains("▶ lab") && out.contains("work +net"), "{out}");
    let (list, overview) = (out.find("work +net").unwrap(), out.find("⬡ lab").unwrap_or_else(|| panic!("the overview shows:\n{out}")));
    assert!(list < overview, "the body is under the list:\n{out}");
}

#[test]
fn a_narrow_launch_screen_stacks_providers_over_kits() {
    let b = Fake::new();
    let p = Picker::new(&b, Some("work"));
    let mut t = Terminal::new(TestBackend::new(30, 20)).unwrap();
    t.draw(|f| draw_picker(f, &p)).unwrap();
    let out = t.backend().buffer().content().chunks(30).map(|row| row.iter().map(|c| c.symbol()).collect::<String>()).collect::<Vec<_>>().join("\n");
    let line = |t: &str| out.lines().position(|l| l.contains(t)).unwrap_or_else(|| panic!("no {t}:\n{out}"));
    assert!(line("▶ work") < line("[x] net"), "kits on a line under the providers:\n{out}");
}

// ── Plugins (ADR-2610261643 §15) ─────────────────────────────────────────────

#[test]
fn the_library_lists_plugins_and_u_updates_them() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('3'), Down, Down]);
    let out = screen(&s);
    assert!(out.contains("▶ plugins") && out.contains("Plugins (1)") && out.contains("egm"), "{out}");
    press(&mut s, &b, &[Char('u')]);
    assert_eq!(b.called(), ["update_plugins *"], "from the list: every plugin");
    assert!(screen(&s).contains("plugins/egm: up to date"), "{}", screen(&s));
    press(&mut s, &b, &[Esc, Tab, Char('u')]);
    assert_eq!(b.called().last().unwrap(), "update_plugins egm", "on a plugin: that one");
}

#[test]
fn a_on_plugins_asks_for_a_git_source() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('3'), Down, Down, Tab, Char('a')]);
    assert!(screen(&s).contains("owner/repo"), "{}", screen(&s));
}

#[test]
fn a_kit_picks_a_plugin_on_its_plugins_tab() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Char('2'), Down, Right, Right]);
    let out = screen(&s);
    assert!(out.contains("Plugins (0)") && line_with(&out, "egm").contains("[ ]"), "{out}");
    press(&mut s, &b, &[Tab, Char(' ')]);
    assert_eq!(b.called(), ["set_use net plugins/egm true"]);
    assert!(line_with(&screen(&s), "egm").contains("[x]"), "{}", screen(&s));
}
