//! Gate for the TUI (ADR-2610261255; the start screen and its pages, ADR-2610261643 §18).
//! Driven with keys, drawn into a TestBackend — no terminal needed. A fake
//! `Browser` keeps the library, kits and attachments, and records each call.

use ccs::adapters::primary::tui::{draw, Exit, Key, State};
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
    /// The resolved profile, as `ccs` would launch it.
    current: RefCell<Option<String>>,
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
            current: RefCell::new(None),
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
    fn current(self, p: &str) -> Self {
        *self.current.borrow_mut() = Some(p.into());
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
                current: self.current.borrow().as_deref() == Some(p.as_str()),
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

use Key::{Char, Down, End, Enter, Esc, Home, Left, PageDown, Right, Tab, Up, WheelDown, WheelUp};

/// The screen line holding `text`.
fn line_with<'a>(screen: &'a str, text: &str) -> &'a str {
    screen.lines().find(|l| l.contains(text)).unwrap_or_else(|| panic!("no line with {text:?} in:\n{screen}"))
}


/// The breadcrumb: the screen's first line.
fn crumb(out: &str) -> String {
    out.lines().next().unwrap_or("").to_string()
}

/// Home: profiles lab, work (the first is selected when nothing resolves); kits eng, net.
fn home(b: &Fake) -> State {
    State::new(b)
}

/// The page of profile `work`: Home, Down to work, e.
fn work_page(b: &Fake) -> State {
    let mut s = State::new(b);
    press(&mut s, b, &[Down, Char('e')]);
    s
}

/// The page of kit `net`: Home, Tab to the kits, Down to net, e.
/// Rows: skills/net/, + Add from the library, CLAUDE.md, lab, work, Rename, Delete.
fn net_page(b: &Fake) -> State {
    let mut s = State::new(b);
    press(&mut s, b, &[Tab, Down, Char('e')]);
    s
}

/// The library as a picker for kit net: its page, then "+ Add from the library".
fn net_library(b: &Fake) -> State {
    let mut s = net_page(b);
    press(&mut s, b, &[Down, Enter]);
    s
}

/// The library to manage: Home, l. Skills rows: humanize, net/, inspect-edge, nqe/, find-nqe-query.
fn library_skills(b: &Fake) -> State {
    let mut s = State::new(b);
    press(&mut s, b, &[Char('l')]);
    s
}

// ── Home (ADR-2610261643 §10, §18) ───────────────────────────────────────────

#[test]
fn home_shows_profiles_with_their_provider_and_kits_with_their_counts() {
    let b = Fake::new().current("work");
    let out = screen(&home(&b));
    assert!(line_with(&out, "▶ work").contains("bedrock sso@us-east-1"), "the resolved profile is selected:\n{out}");
    assert!(out.contains("anthropic"), "{out}");
    assert!(line_with(&out, "[x] net").contains('2'), "net picks 2 items:\n{out}");
    assert!(out.contains("[ ] eng"), "{out}");
}

#[test]
fn moving_to_another_profile_ticks_that_profiles_kits() {
    let b = Fake::new().current("work");
    let mut s = home(&b);
    press(&mut s, &b, &[Up]);
    let out = screen(&s);
    assert!(out.contains("▶ lab") && out.contains("[ ] net"), "{out}");
    assert!(screen(&home(&Fake::new())).contains("▶ lab"), "with nothing resolved, the first profile");
}

#[test]
fn enter_launches_with_the_ticked_kits_for_this_launch_only() {
    let b = Fake::new().current("work");
    let mut s = home(&b);
    let exit = press(&mut s, &b, &[Tab, Up, Char(' '), Enter]);
    assert_eq!(exit, Some(Exit::LaunchWith { profile: "work".into(), kits: vec!["net".into(), "eng".into()] }), "defaults first, in order");
    assert!(b.called().is_empty(), "nothing saved");
    let mut s = home(&b);
    assert_eq!(press(&mut s, &b, &[Enter]), Some(Exit::LaunchWith { profile: "work".into(), kits: vec!["net".into()] }), "one Enter");
}

#[test]
fn s_saves_the_ticked_kits_as_the_profiles_defaults() {
    let b = Fake::new().current("work");
    let mut s = home(&b);
    press(&mut s, &b, &[Tab, Up, Char(' '), Char('s')]);
    assert_eq!(b.called(), ["set_kits work net,eng"]);
    assert!(screen(&s).contains("work now starts with +net +eng"), "{}", screen(&s));
}

#[test]
fn q_and_esc_quit_from_home() {
    let b = Fake::new();
    assert_eq!(press(&mut home(&b), &b, &[Char('q')]), Some(Exit::Quit));
    assert_eq!(press(&mut home(&b), &b, &[Esc]), Some(Exit::Quit));
}

#[test]
fn the_footer_says_what_enter_starts_and_what_else_home_does() {
    let b = Fake::new().current("work");
    let mut s = home(&b);
    let out = screen(&s);
    assert!(out.contains("⏎ launch work +net"), "{out}");
    for k in ["e open", "n new", "i import", "l library"] {
        assert!(out.contains(k), "{k}:\n{out}");
    }
    press(&mut s, &b, &[Tab, Char(' ')]);
    assert!(screen(&s).contains("⏎ launch work ·"), "net unticked: {}", screen(&s));
}

#[test]
fn with_no_profiles_home_says_n_makes_one_and_n_opens_the_new_profile() {
    let b = Fake::with(&[]);
    let mut s = home(&b);
    assert!(screen(&s).contains("n makes one"), "{}", screen(&s));
    assert_eq!(press(&mut s, &b, &[Enter]), None, "nothing to launch");
    press(&mut s, &b, &[Char('n')]);
    assert!(screen(&s).contains("New profile name:"), "{}", screen(&s));
    typed(&mut s, &b, "zed");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called(), ["new_profile zed -"]);
    assert!(crumb(&screen(&s)).contains("ccs › zed"), "its page opens, to fill it in:\n{}", screen(&s));
}

#[test]
fn a_bad_or_taken_profile_name_says_why() {
    let b = Fake::new();
    let mut s = home(&b);
    press(&mut s, &b, &[Char('n')]);
    typed(&mut s, &b, "bad name");
    press(&mut s, &b, &[Enter]);
    assert!(screen(&s).contains("bad profile name 'bad name'"), "{}", screen(&s));
    press(&mut s, &b, &[Char('n')]);
    typed(&mut s, &b, "work");
    press(&mut s, &b, &[Enter]);
    assert!(screen(&s).contains("profile 'work' already exists"), "{}", screen(&s));
    assert!(!crumb(&screen(&s)).contains('›'), "still home");
    assert!(b.called().is_empty());
}

#[test]
fn n_in_the_kit_list_makes_a_kit_and_opens_it() {
    let b = Fake::new();
    let mut s = home(&b);
    press(&mut s, &b, &[Tab, Char('n')]);
    assert!(screen(&s).contains("New kit name:"), "{}", screen(&s));
    typed(&mut s, &b, "write");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called(), ["new_kit write"]);
    let out = screen(&s);
    assert!(crumb(&out).contains("ccs › kit write") && out.contains("nothing yet"), "{out}");
}

#[test]
fn the_highlighted_kit_on_home_opens_to_show_what_it_holds() {
    let b = Fake::new().current("work");
    let mut s = home(&b);
    assert!(!screen(&s).contains("skills/net/"), "closed while the profiles have the focus");
    press(&mut s, &b, &[Tab]);
    let out = screen(&s);
    assert!(line_with(&out, "skills/net/").contains('2') && out.contains("CLAUDE.md"), "tab lands on work's ticked kit, net:\n{out}");
    press(&mut s, &b, &[Up]);
    let out = screen(&s);
    assert!(out.contains("nothing picked") && !out.contains("skills/net/"), "{out}");
}

#[test]
fn each_entry_of_an_open_kit_counts_its_own_items() {
    let b = Fake::new().current("work");
    b.set_use("net", &Use::Item(Kind::Skill, "humanize".into()), true).unwrap();
    b.set_use("net", &Use::Item(Kind::Skill, "gone".into()), true).unwrap();
    let mut s = home(&b);
    screen(&s);
    press(&mut s, &b, &[Tab]);
    let out = screen(&s);
    assert!(line_with(&out, "skills/net/").contains("  2"), "two items, not the kit's three:\n{out}");
    assert!(line_with(&out, "skills/humanize").contains("  1"), "{out}");
    assert!(line_with(&out, "skills/gone").contains("not in the library"), "{out}");
}

// ── Where you are (§17, §18) ─────────────────────────────────────────────────

#[test]
fn the_breadcrumb_shows_the_path_and_the_current_profile() {
    let b = Fake::new().current("work");
    let mut s = home(&b);
    let top = crumb(&screen(&s));
    assert!(top.contains("ccs") && !top.contains('›') && top.contains("⬡ work +net"), "{top}");
    press(&mut s, &b, &[Char('e')]);
    assert!(crumb(&screen(&s)).contains("ccs › work"), "{}", screen(&s));
    press(&mut s, &b, &[Down, Down, Enter]);
    assert!(crumb(&screen(&s)).contains("ccs › work › kit net"), "{}", screen(&s));
    press(&mut s, &b, &[Down, Enter]);
    let top = crumb(&screen(&s));
    assert!(top.contains("ccs › work › kit net › library") && top.contains("⬡ work +net"), "{top}");
    press(&mut s, &b, &[Esc, Esc, Esc]);
    assert!(!crumb(&screen(&s)).contains('›'), "esc walks back home:\n{}", screen(&s));
    assert_eq!(press(&mut s, &b, &[Esc]), Some(Exit::Quit), "and from home it quits");
    assert!(crumb(&screen(&home(&Fake::new()))).contains("no profile"), "nothing resolved says so");
}

#[test]
fn q_quits_from_any_page() {
    let b = Fake::new();
    assert_eq!(press(&mut net_library(&b), &b, &[Char('q')]), Some(Exit::Quit));
    assert_eq!(press(&mut work_page(&b), &b, &[Char('q')]), Some(Exit::Quit));
}

// ── A profile's page (§18) ───────────────────────────────────────────────────
// Rows: Launch, eng, net, Variables, Default, Copy, Shell.

#[test]
fn a_profiles_page_shows_launch_its_kits_variables_actions_and_doctor() {
    let b = Fake::new();
    let out = screen(&work_page(&b));
    for want in ["Launch work +net", "Kits it starts with", "[ ] eng", "[x] net", "AWS_PROFILE=sso", "Make it the default", "Copy to a new profile", "Open a shell in its folder", "/profiles/work", "✗ claude is not on PATH"] {
        assert!(out.contains(want), "missing {want:?}:\n{out}");
    }
}

#[test]
fn enter_on_launch_starts_the_profile_with_its_kits() {
    let b = Fake::new();
    assert_eq!(press(&mut work_page(&b), &b, &[Enter]), Some(Exit::Launch("work".into())));
}

#[test]
fn space_on_a_kit_switches_it_and_home_follows() {
    let b = Fake::new();
    let mut s = work_page(&b);
    press(&mut s, &b, &[Down, Char(' ')]);
    assert_eq!(b.called(), ["attach eng work true"]);
    assert!(screen(&s).contains("[x] eng"), "{}", screen(&s));
    assert!(screen(&s).contains("Launch work +net +eng"), "{}", screen(&s));
    press(&mut s, &b, &[Down, Char(' ')]);
    assert_eq!(b.called().last().unwrap(), "attach net work false", "space switches a kit off too");
    press(&mut s, &b, &[Up, Char(' '), Char(' ')]);
    assert_eq!(b.called().last().unwrap(), "attach eng work true");
    press(&mut s, &b, &[Down, Char(' ')]);
    assert!(screen(&s).contains("Launch work +eng +net"), "{}", screen(&s));
    press(&mut s, &b, &[Esc, Down]);
    assert!(screen(&s).contains("[x] eng"), "home ticks work's new kits:\n{}", screen(&s));
}

#[test]
fn the_highlighted_kit_on_a_profile_page_opens_and_enter_goes_to_it() {
    let b = Fake::new();
    let mut s = work_page(&b);
    press(&mut s, &b, &[Down, Down]);
    assert!(line_with(&screen(&s), "skills/net/").contains('2'), "{}", screen(&s));
    press(&mut s, &b, &[Enter]);
    assert!(crumb(&screen(&s)).contains("ccs › work › kit net"), "{}", screen(&s));
}

#[test]
fn variables_default_copy_and_shell_do_what_they_say() {
    let b = Fake::new();
    let mut s = work_page(&b);
    press(&mut s, &b, &[Down, Down, Down, Enter]);
    assert!(s.wants_editor(), "Enter on the variables edits profile.env");
    s.run_editor(&b);
    assert_eq!(b.called(), ["edit_profile work"]);
    press(&mut s, &b, &[Down, Enter]);
    assert_eq!(b.default.borrow().as_deref(), Some("work"));
    assert!(screen(&s).contains("work is now the default"), "{}", screen(&s));
    press(&mut s, &b, &[Down, Enter]);
    assert!(screen(&s).contains("Copy work to:"), "{}", screen(&s));
    typed(&mut s, &b, "work2");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called().last().unwrap(), "new_profile work2 work");
    assert!(crumb(&screen(&s)).contains("ccs › work2"), "the copy opens:\n{}", screen(&s));
    let mut s = work_page(&b);
    assert_eq!(press(&mut s, &b, &[End, Enter]), Some(Exit::Shell("work".into())));
}

// ── A kit's page (§18) ───────────────────────────────────────────────────────

#[test]
fn a_kits_page_shows_what_it_holds_its_claude_md_and_who_uses_it() {
    let b = Fake::new();
    let out = screen(&net_page(&b));
    assert!(line_with(&out, "skills/net/").contains('2'), "{out}");
    for want in ["Holds", "+ Add from the library", "CLAUDE.md", "NET RULES", "Used by", "[ ] lab", "[x] work", "Rename the kit", "Delete the kit"] {
        assert!(out.contains(want), "missing {want:?}:\n{out}");
    }
}

#[test]
fn space_on_an_entry_removes_it() {
    let b = Fake::new();
    let mut s = net_page(&b);
    press(&mut s, &b, &[Char(' ')]);
    assert_eq!(b.called(), ["set_use net skills/net/ false"]);
    let out = screen(&s);
    assert!(out.contains("nothing yet") && !out.contains("skills/net/  "), "{out}");
}

#[test]
fn enter_on_an_item_views_it_and_on_a_folder_opens_the_library() {
    let b = Fake::new();
    b.set_use("net", &Use::Item(Kind::Skill, "humanize".into()), true).unwrap();
    let mut s = net_page(&b);
    press(&mut s, &b, &[Down, Enter]);
    assert!(screen(&s).contains("Body of skills/humanize"), "{}", screen(&s));
    press(&mut s, &b, &[Esc, Up, Enter]);
    assert!(crumb(&screen(&s)).contains("kit net › library"), "{}", screen(&s));
}

#[test]
fn add_from_the_library_picks_for_the_kit_and_esc_shows_the_result() {
    let b = Fake::new();
    let mut s = net_library(&b);
    let out = screen(&s);
    assert!(line_with(&out, "net/").contains("[x]") && line_with(&out, "nqe/").contains("[·]"), "{out}");
    assert!(line_with(&out, "humanize").contains("[ ]"), "{out}");
    press(&mut s, &b, &[Up, Char(' ')]);
    assert_eq!(b.called(), ["set_use net skills/humanize true"], "the cursor starts on the first pick, net/, below humanize");
    press(&mut s, &b, &[Down, Down, Down, Char(' ')]);
    assert!(screen(&s).contains("picked by skills/net/"), "{}", screen(&s));
    press(&mut s, &b, &[Esc]);
    assert!(line_with(&screen(&s), "skills/humanize").contains('1'), "the kit page shows the new entry:\n{}", screen(&s));
}

#[test]
fn left_and_right_change_the_kind_in_the_library() {
    let b = Fake::new();
    let mut s = net_library(&b);
    assert!(screen(&s).contains("Skills (2)"), "{}", screen(&s));
    press(&mut s, &b, &[Right]);
    assert!(line_with(&screen(&s), "rev").contains("[ ]"), "agents:\n{}", screen(&s));
    press(&mut s, &b, &[Right, Char(' ')]);
    assert_eq!(b.called(), ["set_use net plugins/egm true"]);
    press(&mut s, &b, &[Left, Left]);
    assert!(screen(&s).contains("humanize"), "back to skills:\n{}", screen(&s));
    press(&mut s, &b, &[Char('p')]);
    let out = screen(&s);
    assert!(out.contains("inspect-edge") && !out.contains("humanize"), "p shows only the picks:\n{out}");
}

#[test]
fn the_kit_page_edits_claude_md_switches_profiles_renames_and_deletes() {
    let b = Fake::new();
    let mut s = net_page(&b);
    press(&mut s, &b, &[Down, Down, Enter]);
    assert!(s.wants_editor());
    s.run_editor(&b);
    assert_eq!(b.called(), ["edit_kit net"]);
    press(&mut s, &b, &[Down, Char(' ')]);
    assert_eq!(b.called().last().unwrap(), "attach net lab true");
    assert!(line_with(&screen(&s), "lab").contains("[x]"), "{}", screen(&s));
    press(&mut s, &b, &[Down, Down, Enter]);
    assert!(screen(&s).contains("Rename kit net to:"), "{}", screen(&s));
    typed(&mut s, &b, "nets");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called().last().unwrap(), "rename_kit net nets");
    assert!(crumb(&screen(&s)).contains("kit nets"), "{}", screen(&s));
    press(&mut s, &b, &[Down, Enter]);
    assert!(screen(&s).contains("Delete kit nets? y/n"), "{}", screen(&s));
    press(&mut s, &b, &[Char('y')]);
    assert_eq!(b.called().last().unwrap(), "remove_kit nets");
    assert!(!crumb(&screen(&s)).contains('›'), "back home after a delete:\n{}", screen(&s));
}

// ── The library, to manage (§9, §18) ─────────────────────────────────────────

#[test]
fn l_opens_the_library_with_descriptions_and_no_ticks() {
    let b = Fake::new();
    let out = screen(&library_skills(&b));
    assert!(crumb(&out).contains("ccs › library"), "{out}");
    assert!(line_with(&out, "inspect-edge").contains("about inspect-edge"), "{out}");
    assert!(!out.contains("[x]") && !out.contains("[ ]"), "{out}");
    assert!(out.contains("a add") && out.contains("esc back"), "{out}");
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
    assert_eq!(press(&mut s, &b, &[Esc]), None, "esc leaves the view");
    assert!(crumb(&screen(&s)).contains("library"), "and stays in the library");
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
    assert!(out.contains("~/skills/") && out.contains("~/src/a/"), "several matches are listed:\n{out}");
    typed(&mut s, &b, "rc/a");
    press(&mut s, &b, &[Tab, Enter]);
    assert_eq!(b.called(), ["add ~/src/a/ in 'net'"]);
    assert!(screen(&s).contains("added skills/a"), "{}", screen(&s));
    press(&mut s, &b, &[Char('a'), Char('q'), Esc]);
    assert_eq!(b.called().len(), 1, "q is text while typing; esc adds nothing");
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
fn n_makes_a_folder_and_m_moves() {
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
    assert!(out.contains(".trash") && !out.contains("about humanize"), "{out}");
}

#[test]
fn right_shows_agents_then_plugins() {
    let b = Fake::new();
    let mut s = library_skills(&b);
    press(&mut s, &b, &[Right]);
    let out = screen(&s);
    assert!(out.contains("hex/") && out.contains("rev") && !out.contains("humanize"), "{out}");
    press(&mut s, &b, &[Right]);
    assert!(screen(&s).contains("egm"), "{}", screen(&s));
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
fn space_marks_rows_and_m_moves_them_through_a_folder_picker() {
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
    assert!(!line_with(&out, "humanize").contains('‹'), "{out}");
}

#[test]
fn r_renames_a_folder_not_an_item() {
    let b = Fake::new();
    let mut s = library_skills(&b);
    press(&mut s, &b, &[Down, Char('r')]);
    typed(&mut s, &b, "ops");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called(), ["rename_folder skills net ops"]);
    press(&mut s, &b, &[Up, Char('r')]);
    assert_eq!(b.called().len(), 1, "an item is not renamed");
}

#[test]
fn plugins_update_with_u_and_a_asks_for_a_git_source() {
    let b = Fake::new();
    let mut s = library_skills(&b);
    press(&mut s, &b, &[Right, Right, Char('u')]);
    assert_eq!(b.called(), ["update_plugins egm"], "on a plugin: that one");
    assert!(screen(&s).contains("plugins/egm: up to date"), "{}", screen(&s));
    press(&mut s, &b, &[Esc, Char('a')]);
    assert!(screen(&s).contains("owner/repo"), "{}", screen(&s));
}

#[test]
fn ui_is_a_command_and_a_reserved_name() {
    assert!(ProfileName::parse("ui").is_err());
}

// ── Import (§12) ─────────────────────────────────────────────────────────────

#[test]
fn i_on_home_lists_what_can_be_imported() {
    let b = Fake::new();
    let mut s = home(&b);
    press(&mut s, &b, &[Char('i')]);
    let out = screen(&s);
    assert!(out.contains("~/.claude") && out.contains("aws dev") && out.contains("every AWS profile"), "{out}");
    assert!(line_with(&out, "aws sso").contains("→ work"), "{out}");
    assert!(!out.contains("migrate"), "nothing to migrate:\n{out}");
}

#[test]
fn i_imports_an_aws_profile_or_claude_home_and_shows_the_result() {
    let b = Fake::new();
    let mut s = home(&b);
    press(&mut s, &b, &[Char('i')]);
    typed(&mut s, &b, "dev");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called(), ["import_aws dev false"]);
    assert!(screen(&s).contains("✓ dev (bedrock, us-east-1)"), "{}", screen(&s));
    press(&mut s, &b, &[Esc]);
    assert!(screen(&s).contains("dev"), "home lists it:\n{}", screen(&s));
    press(&mut s, &b, &[Char('i')]);
    typed(&mut s, &b, "claude");
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called().last().unwrap(), "import_claude");
}

#[test]
fn migrate_is_offered_first_only_when_an_old_layout_is_here() {
    let b = Fake::new().old_layout();
    let mut s = home(&b);
    press(&mut s, &b, &[Char('i')]);
    assert!(screen(&s).contains("migrate"), "{}", screen(&s));
    press(&mut s, &b, &[Enter]);
    assert_eq!(b.called(), ["migrate"]);
}

// ── Scrolling (§11) ──────────────────────────────────────────────────────────
// Library skills with bulk(60): bulk/, s000…s059, humanize, net/, inspect-edge, nqe/, find-nqe-query = 66 rows.

/// The first row of the page's list: the line under the second line's top border.
fn first_list_row(out: &str) -> String {
    let lines: Vec<&str> = out.lines().collect();
    let top = lines.iter().position(|l| l.contains('┌')).unwrap_or(1);
    lines[top + 1].trim_matches(|c| c == '│' || c == ' ').to_string()
}

#[test]
fn page_down_moves_a_page_and_end_and_home_reach_the_ends() {
    let b = Fake::new().bulk(60);
    let mut s = library_skills(&b);
    screen(&s);
    press(&mut s, &b, &[PageDown]);
    assert!(!screen(&s).contains("s000"), "{}", screen(&s));
    press(&mut s, &b, &[End]);
    assert!(screen(&s).contains("find-nqe-query") && screen(&s).contains("66/66"), "{}", screen(&s));
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
    let before = first_list_row(&screen(&s));
    press(&mut s, &b, &[Up]);
    assert_eq!(first_list_row(&screen(&s)), before, "{}", screen(&s));
    assert!(screen(&s).contains("30/66"));
}

#[test]
fn the_wheel_moves_three_rows() {
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
    assert!(!screen(&library_skills(&Fake::new())).contains('█'));
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
    assert_eq!(press(&mut s, &b, &[Esc]), None);
}

#[test]
fn home_pages_too_and_scrolls_to_a_ticked_kit() {
    let b = Fake::new();
    let mut s = home(&b);
    press(&mut s, &b, &[End]);
    assert!(screen(&s).contains("▶ work"));
    press(&mut s, &b, &[Home]);
    assert!(screen(&s).contains("▶ lab"));
    // With many_kits(30) the kits are eng, k00…k29, net: work's net is row 32.
    let b = Fake::new().many_kits(30).current("work");
    let mut s = home(&b);
    assert!(screen(&s).contains("[x] net"), "{}", screen(&s));
    press(&mut s, &b, &[Tab]);
    assert!(screen(&s).contains("[x] net"), "tab keeps it in view:\n{}", screen(&s));
}

#[test]
fn a_kits_library_opens_scrolled_to_what_it_picks() {
    let b = Fake::new().bulk(60);
    let out = screen(&net_library(&b));
    assert!(line_with(&out, "net/").contains("[x]"), "net/ is row 62 of 66:\n{out}");
}

#[test]
fn a_narrow_terminal_stacks_home_and_cuts_nothing() {
    let b = Fake::new().current("work");
    let s = home(&b);
    let mut t = Terminal::new(TestBackend::new(30, 24)).unwrap();
    t.draw(|f| draw(f, &s)).unwrap();
    let out = t.backend().buffer().content().chunks(30).map(|r| r.iter().map(|c| c.symbol()).collect::<String>()).collect::<Vec<_>>().join("\n");
    let line = |w: &str| out.lines().position(|l| l.contains(w)).unwrap_or_else(|| panic!("no {w}:\n{out}"));
    assert!(line("▶ work") < line("[x] net"), "kits under the profiles:\n{out}");
}

#[test]
fn with_no_kits_home_and_a_profile_page_say_how_to_make_one() {
    let b = Fake::new().no_kits();
    assert!(screen(&home(&b)).contains("No kits yet: tab here, then n"), "{}", screen(&home(&b)));
    let mut s = home(&b);
    press(&mut s, &b, &[Tab, Char('n')]);
    assert!(screen(&s).contains("New kit name:"), "the hint works:\n{}", screen(&s));
    let out = screen(&work_page(&b));
    assert!(out.contains("no kits yet: on Home, tab to the kits and press n"), "{out}");
}
