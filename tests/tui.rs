//! Gate for the TUI (ADR-2610261255; rows, trees and keys from ADR-2610261643 §8).
//! Driven with keys, drawn into a TestBackend — no terminal needed. A fake
//! `Browser` keeps the library, kits and attachments, and records each call.

use ccs::adapters::primary::tui::{draw, draw_picker, Exit, Key, Picker, State};
use ccs::domain::{CcsError, Finding, Kind, LibItem, LibraryRemoved, ListEntry, Overview, ProfileName, Use};
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
    profiles: Vec<&'static str>,
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
            profiles: profiles.to_vec(),
            default: RefCell::new(None),
            attached: RefCell::new(attached),
            kits: RefCell::new(kits),
            instructions: [("net".to_string(), "NET RULES".to_string())].into_iter().collect(),
            items: RefCell::new(vec![
                item(Kind::Skill, "", "humanize"),
                item(Kind::Skill, "net", "inspect-edge"),
                item(Kind::Skill, "net/nqe", "find-nqe-query"),
                item(Kind::Agent, "hex", "rev"),
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
            .iter()
            .map(|p| ListEntry {
                name: name(p),
                current: false,
                is_default: self.default.borrow().as_deref() == Some(*p),
                kits: self.attached.borrow().get(*p).cloned().unwrap_or_default(),
                provider: if *p == "lab" { "anthropic".into() } else { "bedrock sso@us-east-1".into() },
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

/// Rows: lab, work, kits, eng, net, library. Downs from the top.
const LAB: usize = 0;
const WORK: usize = 1;
const KITS: usize = 2;
const ENG: usize = 3;
const NET: usize = 4;
const LIBRARY: usize = 5;

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
fn rows_are_profiles_with_their_kits_then_kits_then_the_library() {
    let b = Fake::new();
    let s = State::new(&b);
    let out = screen(&s);
    for want in ["lab", "work +net", "kits", "eng", "net", "◆ library"] {
        assert!(out.contains(want), "missing {want:?}:\n{out}");
    }
    let order: Vec<usize> = ["▶ lab", "work +net", "── kits", "  eng", "  net", "◆ library"].iter().map(|w| out.find(w).unwrap_or_else(|| panic!("{w}\n{out}"))).collect();
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
    assert!(line_with(&screen(&s), "◆ library").contains('▶'), "{}", screen(&s));
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
fn no_profiles_still_shows_kits_and_the_library_and_says_how_to_start() {
    let b = Fake::with(&[]);
    let mut s = State::new(&b);
    let out = screen(&s);
    assert!(out.contains("ccs new <name>") && out.contains("◆ library"), "{out}");
    assert_eq!(press(&mut s, &b, &[Enter]), None, "the kits heading does not launch");
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
    press(&mut s, &b, &[Down, Down, Down, Down]);
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
    press(&mut s, &b, &[Down, Down, Down, Down, Tab]);
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
    press(&mut s, &b, &[Down, Down, Down, Down, Right, Right]);
    assert!(screen(&s).contains("NET RULES"), "{}", screen(&s));
    press(&mut s, &b, &[Char('e')]);
    assert!(s.wants_editor());
    assert!(b.called().is_empty(), "the editor never runs inside handle()");
    s.run_editor(&b);
    assert_eq!(b.called(), ["edit_kit net"]);
    // A kit with no instructions says how to start them.
    let mut s = State::new(&b);
    press(&mut s, &b, &[Down, Down, Down, Right, Right]);
    assert!(screen(&s).contains("e writes"), "{}", screen(&s));
}

#[test]
fn n_makes_a_kit_from_the_heading_and_x_deletes_one_after_asking() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Down, Down, Char('n')]);
    typed(&mut s, &b, "write");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called(), ["new_kit write"]);
    assert!(screen(&s).contains("write"), "{}", screen(&s));
    press(&mut s, &b, &[Down, Char('x')]);
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
    for _ in 0..LIBRARY {
        s.handle(b, Down);
    }
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
    for _ in 0..LIBRARY {
        s.handle(&b, Down);
    }
    press(&mut s, &b, &[Right]);
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
    press(&mut s, &b, &[Down, Down, Down, Down, Tab]);
    assert!(screen(&s).contains("space pick"), "{}", screen(&s));
}

#[test]
fn ui_is_a_command_and_a_reserved_name() {
    assert!(ProfileName::parse("ui").is_err());
    let _ = (LAB, KITS, ENG, NET, WORK);
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
    press(&mut s, &b, &[Down, Down, Down, Char('r')]);
    typed(&mut s, &b, "engineering");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called().last().unwrap(), "rename_kit eng engineering");
    assert!(screen(&s).contains("engineering"), "{}", screen(&s));
}

#[test]
fn a_kit_attaches_to_profiles_from_its_profiles_tab() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Down, Down, Down, Down, Right, Right, Right, Tab]);
    let out = screen(&s);
    assert!(out.contains("[ ] lab") && out.contains("[x] work"), "{out}");
    press(&mut s, &b, &[Char(' ')]);
    assert_eq!(b.called(), ["attach net lab true"]);
    assert!(screen(&s).contains("lab +net"), "{}", screen(&s));
}

#[test]
fn p_shows_only_what_a_kit_picks_and_tabs_count() {
    let b = Fake::new();
    let mut s = State::new(&b);
    press(&mut s, &b, &[Down, Down, Down, Down]);
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
