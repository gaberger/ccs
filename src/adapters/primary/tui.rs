//! The ratatui TUI (ADR-2610261255; rows, trees and keys in ADR-2610261643 §8)
//! — a primary adapter.
//!
//! Rule 4: it imports `ports/` only. Everything it does goes through the
//! `Browser` driving port. `State` takes keys and `draw` paints it, so tests
//! drive the screen without a terminal; `run` is the only part that touches one.

use crate::ports::{library_tree, pick, AwsImport, Browser, Entry, Finding, ItemKind, Name, Pick, ProfileOverview, ProfileRow, TreeRow};
use std::collections::BTreeSet;
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use ratatui::crossterm::execute;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Tabs};
use ratatui::Frame;
use std::cell::Cell;

/// The adapter's own key type, so tests need no terminal events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Tab,
    Enter,
    Esc,
    Backspace,
    PageUp,
    PageDown,
    Home,
    End,
    /// The mouse wheel: three rows.
    WheelUp,
    WheelDown,
    Char(char),
}

/// How far a key moves a cursor, given the page height; `None` when it is not a movement.
fn movement(key: Key, page: usize) -> Option<isize> {
    let page = page.max(1) as isize;
    Some(match key {
        Key::Up | Key::Char('k') => -1,
        Key::Down | Key::Char('j') => 1,
        Key::PageUp => -page,
        Key::PageDown => page,
        Key::Home | Key::Char('g') => isize::MIN / 2,
        Key::End | Key::Char('G') => isize::MAX / 2,
        Key::WheelUp => -3,
        Key::WheelDown => 3,
        _ => return None,
    })
}

/// `sel` moved by `delta`, kept inside `0..len`.
fn shifted(sel: usize, delta: isize, len: usize) -> usize {
    (sel as isize).saturating_add(delta).clamp(0, len.saturating_sub(1) as isize) as usize
}

/// A list's scroll offset and page height, kept between frames (ADR-2610261643 §11).
#[derive(Default)]
struct Scroll {
    offset: Cell<usize>,
    page: Cell<usize>,
    /// A row to bring into view at the next frame, when the page height is known.
    reveal: Cell<Option<usize>>,
}

impl Scroll {
    fn reset(&self) {
        self.offset.set(0);
    }

    fn reveal(&self, row: Option<usize>) {
        self.reveal.set(row);
    }

    /// `sel` if it is in view, else the top row in view: a list that takes the
    /// focus then keeps the view it had.
    fn in_view(&self, sel: usize) -> usize {
        let (top, page) = (self.offset.get(), self.page.get().max(1));
        if (top..top + page).contains(&sel) { sel } else { top }
    }

    /// Page Up / Page Down move the view by a page too, as a pager does; other keys
    /// leave it to follow the cursor.
    fn follow(&self, key: Key, delta: isize, len: usize) {
        if matches!(key, Key::PageUp | Key::PageDown) {
            let last = len.saturating_sub(self.page.get());
            self.offset.set(shifted(self.offset.get(), delta, last + 1));
        }
    }
}

/// Draw a list that keeps its place between frames, with a scrollbar when it
/// overflows and, when a row is selected, its position in the bottom border.
fn render_list(frame: &mut Frame, area: Rect, block: Block, items: Vec<ListItem>, selected: Option<usize>, scroll: &Scroll, symbol: Option<&'static str>) {
    let len = items.len();
    let inner = area.height.saturating_sub(2) as usize;
    scroll.page.set(inner);
    if let Some(row) = scroll.reveal.take() {
        let top = scroll.offset.get();
        if row < top {
            scroll.offset.set(row);
        } else if inner > 0 && row >= top + inner {
            scroll.offset.set(row + 1 - inner);
        }
    }
    let block = match selected {
        Some(i) if len > 0 => block.title_bottom(Line::from(format!(" {}/{len} ", i + 1)).right_aligned()),
        _ => block,
    };
    let style = if symbol.is_some() { Style::default().add_modifier(Modifier::BOLD) } else { Style::default().add_modifier(Modifier::REVERSED) };
    let mut list = List::new(items).block(block).highlight_style(style);
    if let Some(sym) = symbol {
        list = list.highlight_symbol(sym);
    }
    let mut ls = ListState::default().with_selected(selected).with_offset(scroll.offset.get());
    frame.render_stateful_widget(list, area, &mut ls);
    scroll.offset.set(ls.offset());
    scrollbar(frame, area, len, inner, ls.offset());
}

fn scrollbar(frame: &mut Frame, area: Rect, len: usize, visible: usize, at: usize) {
    if len > visible {
        let mut state = ScrollbarState::new(len.saturating_sub(visible)).position(at);
        let bar = Scrollbar::new(ScrollbarOrientation::VerticalRight).begin_symbol(None).end_symbol(None);
        frame.render_stateful_widget(bar, area.inner(Margin { vertical: 1, horizontal: 0 }), &mut state);
    }
}

/// Why the TUI ended. Launching happens after the terminal is restored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exit {
    Quit,
    /// Launch with the profile's own kits (the manager).
    Launch(String),
    /// Launch with these kits, for this launch only (the launch screen).
    LaunchWith { profile: String, kits: Vec<String> },
    /// A shell in the profile's folder, with its variables (ADR-2610261643 §14).
    Shell(String),
}

/// The menu bar over the left-hand list: one section shows at a time (ADR-2610261643 §13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Profiles,
    Kits,
    Library,
}

const SECTIONS: [Section; 3] = [Section::Profiles, Section::Kits, Section::Library];

impl Section {
    fn label(self) -> &'static str {
        match self {
            Section::Profiles => "Profiles",
            Section::Kits => "Kits",
            Section::Library => "Library",
        }
    }
}

/// A row of the left-hand list.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Row {
    /// The only row of an empty Profiles section, so `n` and `i` work before any profile exists.
    ProfilesHeading,
    Profile(Name),
    /// The only row of an empty Kits section.
    KitsHeading,
    Kit(String),
    Library(ItemKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Kits,
    Skills,
    Agents,
    Instructions,
    Profiles,
}

impl Tab {
    fn title(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Kits => "Kits",
            Tab::Skills => "Skills",
            Tab::Agents => "Agents",
            Tab::Instructions => "CLAUDE.md",
            Tab::Profiles => "Profiles",
        }
    }

    fn kind(self) -> Option<ItemKind> {
        match self {
            Tab::Skills => Some(ItemKind::Skill),
            Tab::Agents => Some(ItemKind::Agent),
            _ => None,
        }
    }
}

fn tabs(row: &Row) -> &'static [Tab] {
    match row {
        Row::Profile(_) => &[Tab::Overview, Tab::Kits],
        Row::ProfilesHeading => &[Tab::Overview],
        Row::KitsHeading => &[Tab::Kits],
        Row::Kit(_) => &[Tab::Skills, Tab::Agents, Tab::Instructions, Tab::Profiles],
        Row::Library(ItemKind::Skill) => &[Tab::Skills],
        Row::Library(ItemKind::Agent) => &[Tab::Agents],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    List,
    Body,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Pending {
    DeleteEntries(Vec<Entry>),
    DeleteKit(String),
}

/// What a picker chooses for (ADR-2610261643 §9).
#[derive(Debug, Clone, PartialEq, Eq)]
enum PickFor {
    /// A folder to move these entries into.
    MoveTo(Vec<Entry>),
    /// A kit to add these entries to.
    AddToKit(Vec<Entry>),
    /// One of the things `ccs import` and `ccs migrate` can do.
    Import(Vec<ImportItem>),
}

/// A line of the import list (ADR-2610261643 §12).
#[derive(Debug, Clone, PartialEq, Eq)]
enum ImportItem {
    Migrate,
    Claude,
    Aws { aws: String, imported_as: Option<String> },
    AllAws,
}

impl ImportItem {
    fn label(&self) -> String {
        match self {
            ImportItem::Migrate => "migrate what an earlier ccs left (shared/, native)".into(),
            ImportItem::Claude => "~/.claude: its skills, agents and CLAUDE.md, into the library and kit imported".into(),
            ImportItem::Aws { aws, imported_as: None } => format!("aws {aws}: a Bedrock profile"),
            ImportItem::Aws { aws, imported_as: Some(p) } => format!("aws {aws} → {p}, imported"),
            ImportItem::AllAws => "every AWS profile not imported yet".into(),
        }
    }
}

/// One line of a picker: an existing folder or kit, or a new one made from the typed text.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Choice {
    Existing(String),
    New(String),
}

/// What a line of typed text is for.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Purpose {
    /// A path to copy into the library, into `folder`.
    Add { folder: String },
    /// A folder name, made inside `base`.
    Mkdir { base: String },
    NewKit,
    /// `ccs new`, or `ccs new --from` with `from`.
    NewProfile { from: Option<Name> },
    RenameKit { old: String },
    RenameFolder { kind: ItemKind, folder: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Normal,
    Search,
    /// A y/n question.
    Confirm(Pending),
    View { title: String, text: String, scroll: u16 },
    /// Text being typed, and the path completions last offered.
    Input { purpose: Purpose, text: String, candidates: Vec<String> },
    /// A list to choose from; typing filters it, or names a new folder or kit.
    Pick { purpose: PickFor, filter: String, sel: usize },
}

/// What the run loop hands to the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
enum EditTarget {
    Entry(Entry),
    Kit(String),
    Profile(Name),
}

/// The longest start every string shares.
fn common_prefix(all: &[String]) -> String {
    let Some(first) = all.first() else { return String::new() };
    let mut end = first.len();
    for s in &all[1..] {
        end = first.char_indices().zip(s.chars()).take_while(|((_, a), b)| a == b).map(|((i, a), _)| i + a.len_utf8()).last().unwrap_or(0).min(end);
    }
    first[..end].to_string()
}

fn matches(filter: &str, text: &str) -> bool {
    filter.is_empty() || text.to_lowercase().contains(&filter.to_lowercase())
}

/// `skills/net/`, or `skills/ (the top)` for the top folder.
/// `skills/humanize`, or `3 entries`.
fn count(entries: &[Entry]) -> String {
    match entries {
        [one] => one.line(),
        many => format!("{} entries", many.len()),
    }
}

fn folder_label(kind: ItemKind, folder: &str) -> String {
    if folder.is_empty() {
        format!("{}/ (the top)", kind.dir())
    } else {
        format!("{}/{folder}/", kind.dir())
    }
}

pub struct State {
    profiles: Vec<ProfileRow>,
    kits: Vec<String>,
    sel: usize,
    list_filter: String,
    tab: usize,
    focus: Focus,
    body_sel: usize,
    body_filter: String,
    overview: Option<Result<ProfileOverview, String>>,
    /// The library tree for the tab's kind.
    tree: Vec<TreeRow>,
    /// Item counts per kind: the library's, or what the selected kit picks.
    counts: [usize; 2],
    /// Every kit's `uses`, for the labels on library rows.
    all_uses: Vec<(String, Vec<Entry>)>,
    /// Marked library rows, by path, for `m`, `k` and `x`.
    marks: BTreeSet<String>,
    /// On a kit: show only what it picks.
    picked_only: bool,
    /// The selected kit's `uses`.
    uses: Vec<Entry>,
    instructions: Option<String>,
    mode: Mode,
    message: Option<String>,
    editing: Option<EditTarget>,
    section: Section,
    /// The row each section was on, so switching back finds it.
    sels: [usize; 3],
    list_scroll: Scroll,
    body_scroll: Scroll,
    pick_scroll: Scroll,
    /// The `v` view's height, for paging.
    view_scroll: Scroll,
}

impl State {
    pub fn new(b: &dyn Browser) -> State {
        let mut s = State {
            profiles: vec![],
            kits: vec![],
            sel: 0,
            list_filter: String::new(),
            tab: 0,
            focus: Focus::List,
            body_sel: 0,
            body_filter: String::new(),
            overview: None,
            tree: vec![],
            counts: [0, 0],
            all_uses: vec![],
            marks: BTreeSet::new(),
            picked_only: false,
            uses: vec![],
            instructions: None,
            mode: Mode::Normal,
            message: None,
            editing: None,
            section: Section::Profiles,
            sels: [0; 3],
            list_scroll: Scroll::default(),
            body_scroll: Scroll::default(),
            pick_scroll: Scroll::default(),
            view_scroll: Scroll::default(),
        };
        s.refresh(b);
        if s.profiles.is_empty() {
            s.message = Some("No profiles yet: n makes one, i imports your AWS profiles".into());
        }
        s
    }

    fn rows(&self) -> Vec<Row> {
        let f = &self.list_filter;
        let rows: Vec<Row> = match self.section {
            Section::Profiles => self.profiles.iter().filter(|p| matches(f, p.name.as_str())).map(|p| Row::Profile(p.name.clone())).collect(),
            Section::Kits => self.kits.iter().filter(|k| matches(f, k)).map(|k| Row::Kit(k.clone())).collect(),
            Section::Library => vec![Row::Library(ItemKind::Skill), Row::Library(ItemKind::Agent)],
        };
        if !rows.is_empty() {
            return rows;
        }
        vec![if self.section == Section::Profiles { Row::ProfilesHeading } else { Row::KitsHeading }]
    }

    fn row(&self) -> Row {
        let rows = self.rows();
        rows.get(self.sel).or(rows.last()).cloned().unwrap_or(Row::ProfilesHeading)
    }

    fn tab(&self) -> Tab {
        let t = tabs(&self.row());
        t[self.tab.min(t.len() - 1)]
    }

    /// The selected profile, for callers and tests. `None` on any other row.
    pub fn selected_profile(&self) -> Option<&str> {
        let name = match self.rows().get(self.sel) {
            Some(Row::Profile(n)) => n.clone(),
            _ => return None,
        };
        self.profiles.iter().find(|p| p.name == name).map(|p| p.name.as_str())
    }

    /// Whether the body is a list the cursor can move in.
    fn body_is_list(&self) -> bool {
        matches!(self.tab(), Tab::Skills | Tab::Agents | Tab::Profiles) || (self.tab() == Tab::Kits && matches!(self.row(), Row::Profile(_)))
    }

    /// Tree rows matching the body filter, with the folders above each match.
    fn visible_tree(&self) -> Vec<&TreeRow> {
        let f = &self.body_filter;
        let kind = self.tab().kind().unwrap_or(ItemKind::Skill);
        let shown = |r: &TreeRow| !self.picked_only || !matches!(self.row(), Row::Kit(_)) || pick(&self.uses, kind, r) != Pick::No;
        let hit = |r: &TreeRow| {
            shown(r) && r.item.as_ref().is_some_and(|i| matches(f, &i.name) || i.description.as_deref().is_some_and(|d| matches(f, d)))
        };
        self.tree
            .iter()
            .filter(|r| match &r.item {
                Some(_) => hit(r),
                None => (f.is_empty() && shown(r)) || self.tree.iter().any(|x| hit(x) && x.path.starts_with(&format!("{}/", r.path))),
            })
            .collect()
    }

    fn tree_row(&self) -> Option<TreeRow> {
        self.visible_tree().get(self.body_sel).map(|r| (*r).clone())
    }

    /// The first body row with a tick, so a long list can show it.
    fn first_ticked(&self) -> Option<usize> {
        match (self.row(), self.tab()) {
            (Row::Profile(n), Tab::Kits) => {
                let mine = self.profiles.iter().find(|p| p.name == n).map(|p| p.kits.clone()).unwrap_or_default();
                self.kits.iter().position(|k| mine.contains(k))
            }
            (Row::Kit(k), Tab::Profiles) => self.profiles.iter().position(|p| p.kits.contains(&k)),
            (Row::Kit(_), tab) => {
                let kind = tab.kind()?;
                self.visible_tree().iter().position(|r| pick(&self.uses, kind, r) != Pick::No)
            }
            _ => None,
        }
    }

    fn body_len(&self) -> usize {
        match self.tab() {
            Tab::Skills | Tab::Agents => self.visible_tree().len(),
            Tab::Kits => self.kits.len(),
            Tab::Profiles => self.profiles.len(),
            _ => 0,
        }
    }

    /// Reload everything the selected row and tab show.
    fn refresh(&mut self, b: &dyn Browser) {
        self.profiles = b.profiles();
        self.kits = b.kits();
        self.sel = self.sel.min(self.rows().len() - 1);
        let row = self.row();
        self.overview = match &row {
            Row::Profile(n) => Some(b.overview(n).map_err(|e| e.to_string())),
            _ => None,
        };
        (self.uses, self.instructions) = match &row {
            Row::Kit(k) => (b.kit_uses(k).unwrap_or_default(), b.kit_instructions(k)),
            _ => (vec![], None),
        };
        self.all_uses = self.kits.iter().map(|k| (k.clone(), b.kit_uses(k).unwrap_or_default())).collect();
        let mut trees = [ItemKind::Skill, ItemKind::Agent].map(|kind| {
            let (items, folders) = b.library(kind);
            (kind, library_tree(&items, &folders))
        });
        for (i, (kind, tree)) in trees.iter().enumerate() {
            let items = tree.iter().filter(|r| r.item.is_some());
            self.counts[i] = match &row {
                Row::Kit(_) => items.filter(|r| pick(&self.uses, *kind, r) != Pick::No).count(),
                _ => items.count(),
            };
        }
        self.tree = match self.tab().kind() {
            Some(ItemKind::Skill) => std::mem::take(&mut trees[0].1),
            Some(ItemKind::Agent) => std::mem::take(&mut trees[1].1),
            None => vec![],
        };
        self.body_sel = self.body_sel.min(self.body_len().saturating_sub(1));
        if !self.body_is_list() {
            self.focus = Focus::List;
        }
    }

    /// Apply one key. `Some` ends the TUI.
    pub fn handle(&mut self, b: &dyn Browser, key: Key) -> Option<Exit> {
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::View { title, text, scroll } => {
                if matches!(key, Key::Esc | Key::Char('q')) {
                    return None;
                }
                let page = self.view_scroll.page.get();
                let last = text.lines().count().saturating_sub(page);
                let scroll = match movement(key, page) {
                    Some(d) => u16::try_from(shifted(scroll as usize, d, last + 1)).unwrap_or(u16::MAX),
                    None => scroll,
                };
                self.mode = Mode::View { title, text, scroll };
                None
            }
            Mode::Input { purpose, mut text, mut candidates } => {
                match key {
                    Key::Esc => {
                        self.message = Some("cancelled".into());
                        return None;
                    }
                    Key::Enter => {
                        self.submit(b, purpose, text.trim());
                        return None;
                    }
                    Key::Backspace => {
                        text.pop();
                        candidates.clear();
                    }
                    Key::Char(c) => {
                        text.push(c);
                        candidates.clear();
                    }
                    Key::Tab if matches!(purpose, Purpose::Add { .. }) => {
                        let found = b.complete_path(&text);
                        if let [one] = found.as_slice() {
                            text = one.clone();
                            candidates.clear();
                        } else {
                            let prefix = common_prefix(&found);
                            if prefix.len() > text.len() {
                                text = prefix;
                            }
                            candidates = found;
                        }
                    }
                    _ => {}
                }
                self.mode = Mode::Input { purpose, text, candidates };
                None
            }
            Mode::Pick { purpose, mut filter, mut sel } => {
                match key {
                    Key::Esc => {
                        self.message = Some("cancelled".into());
                        return None;
                    }
                    Key::Enter => {
                        if let Some(choice) = self.choices(&purpose, &filter).get(sel).cloned() {
                            self.chosen(b, purpose, choice);
                        }
                        return None;
                    }
                    // Letters are filter text here, so only the arrow, page and wheel keys move.
                    k if !matches!(k, Key::Char(_)) && movement(k, 0).is_some() => {
                        let d = movement(k, self.pick_scroll.page.get()).unwrap_or(0);
                        let len = self.choices(&purpose, &filter).len();
                        self.pick_scroll.follow(k, d, len);
                        sel = shifted(sel, d, len);
                    }
                    Key::Backspace => {
                        filter.pop();
                        sel = 0;
                    }
                    Key::Char(c) => {
                        filter.push(c);
                        sel = 0;
                        self.pick_scroll.reset();
                    }
                    _ => {}
                }
                self.mode = Mode::Pick { purpose, filter, sel };
                None
            }
            Mode::Confirm(pending) => {
                if key == Key::Char('y') {
                    self.message = Some(match pending {
                        Pending::DeleteEntries(entries) => {
                            let mut lines = Vec::new();
                            for e in &entries {
                                lines.push(match b.remove(e) {
                                    Ok(r) if r.kits.is_empty() => format!("moved {} to {}", e.line(), r.trashed.display()),
                                    Ok(r) => format!("moved {} to {}; kits that used it: {}", e.line(), r.trashed.display(), r.kits.join(", ")),
                                    Err(err) => err.to_string(),
                                });
                            }
                            self.marks.clear();
                            lines.join(" · ")
                        }
                        Pending::DeleteKit(k) => match b.remove_kit(&k) {
                            Ok(at) => format!("moved kit {k} to {}; no profile uses it now", at.display()),
                            Err(err) => err.to_string(),
                        },
                    });
                    self.refresh(b);
                } else {
                    self.message = Some("cancelled".into());
                }
                None
            }
            Mode::Search => {
                let filter = match self.focus {
                    Focus::List => &mut self.list_filter,
                    Focus::Body => &mut self.body_filter,
                };
                match key {
                    Key::Enter => return None,
                    Key::Esc => filter.clear(),
                    Key::Backspace => {
                        filter.pop();
                        self.mode = Mode::Search;
                    }
                    Key::Char(c) => {
                        filter.push(c);
                        self.mode = Mode::Search;
                    }
                    _ => self.mode = Mode::Search,
                }
                match self.focus {
                    Focus::List => self.sel = 0,
                    Focus::Body => self.body_sel = 0,
                }
                self.refresh(b);
                None
            }
            Mode::Normal => self.normal(b, key),
        }
    }

    fn normal(&mut self, b: &dyn Browser, key: Key) -> Option<Exit> {
        self.message = None;
        match key {
            Key::Char('q') | Key::Esc => return Some(Exit::Quit),
            k if movement(k, 0).is_some() => {
                let (scroll, len) = match self.focus {
                    Focus::List => (&self.list_scroll, self.rows().len()),
                    Focus::Body => (&self.body_scroll, self.body_len()),
                };
                let delta = movement(k, scroll.page.get()).unwrap_or(0);
                scroll.follow(k, delta, len);
                self.move_by(b, delta);
            }
            Key::Left | Key::Right | Key::Char('h') | Key::Char('l') => {
                let right = matches!(key, Key::Right | Key::Char('l'));
                let last = tabs(&self.row()).len() - 1;
                self.tab = if right { (self.tab.min(last) + 1).min(last) } else { self.tab.min(last).saturating_sub(1) };
                self.body_sel = 0;
                self.body_filter.clear();
                self.marks.clear();
                self.body_scroll.reset();
                self.refresh(b);
                self.body_scroll.reveal(self.first_ticked());
            }
            Key::Tab if self.body_is_list() => {
                self.focus = if self.focus == Focus::List { Focus::Body } else { Focus::List };
                self.body_sel = self.body_scroll.in_view(self.body_sel);
            }
            Key::Enter => match (self.focus, self.row()) {
                (Focus::List, Row::Profile(n)) => return Some(Exit::Launch(n.as_str().into())),
                (Focus::Body, _) => self.view(b),
                _ => {}
            },
            Key::Char('/') => self.mode = Mode::Search,
            Key::Char('d') => {
                if let Row::Profile(n) = self.row() {
                    self.message = Some(match b.set_default(&n) {
                        Ok(()) => format!("{} is now the default", n.as_str()),
                        Err(e) => e.to_string(),
                    });
                    self.refresh(b);
                }
            }
            Key::Char(' ') if self.focus == Focus::Body => self.toggle(b),
            Key::Char('v') if self.focus == Focus::Body => self.view(b),
            Key::Char(c @ '1'..='3') => self.switch(b, SECTIONS[c as usize - '1' as usize]),
            Key::Char('e') => self.ask_edit(),
            Key::Char('t') => {
                if let (Row::Profile(n), Focus::List) = (self.row(), self.focus) {
                    return Some(Exit::Shell(n.as_str().into()));
                }
            }
            Key::Char('c') => self.ask_copy(),
            Key::Char('i') => self.ask_import(b),
            Key::Char('a') => self.ask_add(),
            Key::Char('n') => self.ask_new(),
            Key::Char('m') => self.ask_move(),
            Key::Char('x') => self.ask_delete(),
            Key::Char('+') => self.ask_kit(),
            Key::Char('r') => self.ask_rename(),
            Key::Char('p') if self.focus == Focus::Body && matches!(self.row(), Row::Kit(_)) => {
                self.picked_only = !self.picked_only;
                self.body_sel = 0;
                self.body_scroll.reset();
            }
            _ => {}
        }
        None
    }

    fn move_by(&mut self, b: &dyn Browser, delta: isize) {
        match self.focus {
            Focus::List => {
                let to = shifted(self.sel, delta, self.rows().len());
                if to == self.sel {
                    return;
                }
                self.sel = to;
                self.row_changed(b);
            }
            Focus::Body => self.body_sel = shifted(self.body_sel, delta, self.body_len()),
        }
    }

    /// A new row on the left: its body starts fresh, scrolled to its first tick.
    fn row_changed(&mut self, b: &dyn Browser) {
        self.body_scroll.reset();
        self.body_sel = 0;
        self.body_filter.clear();
        self.marks.clear();
        self.picked_only = false;
        self.refresh(b);
        self.body_scroll.reveal(self.first_ticked());
    }

    /// `1` `2` `3`: show another section, on the row it was left at.
    fn switch(&mut self, b: &dyn Browser, to: Section) {
        if to == self.section {
            return;
        }
        let at = |s: Section| SECTIONS.iter().position(|x| *x == s).unwrap_or(0);
        self.sels[at(self.section)] = self.sel;
        self.section = to;
        self.sel = self.sels[at(to)];
        self.list_filter.clear();
        self.list_scroll.reset();
        self.focus = Focus::List;
        self.tab = 0;
        self.row_changed(b);
    }

    /// Put the cursor on `row`, when the list shows it.
    fn select(&mut self, row: &Row) {
        if let Some(i) = self.rows().iter().position(|r| r == row) {
            self.sel = i;
        }
    }

    fn library_tab(&self) -> Option<ItemKind> {
        (matches!(self.row(), Row::Library(_)) && self.focus == Focus::Body).then(|| self.tab().kind()).flatten()
    }

    /// Space: attach a kit to the profile, or pick a tree row for the kit.
    fn toggle(&mut self, b: &dyn Browser) {
        match (self.row(), self.tab()) {
            (Row::Profile(n), Tab::Kits) => {
                let Some(kit) = self.kits.get(self.body_sel).cloned() else { return };
                let on = !self.profiles.iter().any(|p| p.name == n && p.kits.contains(&kit));
                if let Err(e) = b.attach(&kit, &n, on) {
                    self.message = Some(e.to_string());
                }
            }
            (Row::Kit(k), Tab::Profiles) => {
                let Some(p) = self.profiles.get(self.body_sel).cloned() else { return };
                if let Err(e) = b.attach(&k, &p.name, !p.kits.contains(&k)) {
                    self.message = Some(e.to_string());
                }
            }
            (Row::Library(_), _) => {
                let Some(r) = self.tree_row() else { return };
                if !self.marks.remove(&r.path) {
                    self.marks.insert(r.path);
                }
                return;
            }
            (Row::Kit(k), tab) => {
                let (Some(kind), Some(r)) = (tab.kind(), self.tree_row()) else { return };
                let entry = r.entry(kind);
                let result = match pick(&self.uses, kind, &r) {
                    Pick::Direct => b.set_use(&k, &entry, false),
                    Pick::No => b.set_use(&k, &entry, true),
                    Pick::Through(f) => {
                        self.message = Some(format!("{} is picked by {f}; space on that folder changes it", entry.line()));
                        return;
                    }
                };
                if let Err(e) = result {
                    self.message = Some(e.to_string());
                }
            }
            _ => return,
        }
        self.refresh(b);
    }

    fn view(&mut self, b: &dyn Browser) {
        let (Some(kind), Some(r)) = (self.tab().kind(), self.tree_row()) else { return };
        if r.item.is_none() {
            return;
        }
        let entry = r.entry(kind);
        match b.read(&entry) {
            Ok(text) => self.mode = Mode::View { title: entry.line(), text, scroll: 0 },
            Err(e) => self.message = Some(e.to_string()),
        }
    }

    /// `e`: a kit's CLAUDE.md on its tab, else the highlighted item.
    fn ask_edit(&mut self) {
        match (self.row(), self.tab()) {
            (Row::Kit(k), Tab::Instructions) => self.editing = Some(EditTarget::Kit(k)),
            (Row::Profile(n), _) if self.focus == Focus::List => self.editing = Some(EditTarget::Profile(n)),
            (_, tab) if self.focus == Focus::Body => {
                if let (Some(kind), Some(r)) = (tab.kind(), self.tree_row()) {
                    if r.item.is_some() {
                        self.editing = Some(EditTarget::Entry(r.entry(kind)));
                    }
                }
            }
            _ => {}
        }
    }

    /// Whether the run loop must hand the terminal to the editor now.
    pub fn wants_editor(&self) -> bool {
        self.editing.is_some()
    }

    /// Run the queued edit. The caller has given the terminal to the editor.
    pub fn run_editor(&mut self, b: &dyn Browser) {
        let Some(target) = self.editing.take() else { return };
        self.message = Some(match target {
            EditTarget::Entry(e) => match b.edit(&e) {
                Ok(()) => format!("edited {}; every kit using it gets the change", e.line()),
                Err(err) => err.to_string(),
            },
            EditTarget::Kit(k) => match b.edit_kit(&k) {
                Ok(()) => format!("edited the instructions of kit {k}"),
                Err(err) => err.to_string(),
            },
            EditTarget::Profile(n) => match b.edit_profile(&n) {
                Ok(()) => format!("edited the variables of {}", n.as_str()),
                Err(err) => err.to_string(),
            },
        });
        self.refresh(b);
    }

    fn input(&mut self, purpose: Purpose) {
        self.mode = Mode::Input { purpose, text: String::new(), candidates: vec![] };
    }

    fn ask_add(&mut self) {
        if self.library_tab().is_some() {
            let folder = self.tree_row().map(|r| r.folder()).unwrap_or_default();
            self.input(Purpose::Add { folder });
        }
    }

    fn ask_new(&mut self) {
        match self.row() {
            Row::ProfilesHeading | Row::Profile(_) if self.focus == Focus::List => self.input(Purpose::NewProfile { from: None }),
            Row::KitsHeading | Row::Kit(_) if self.focus == Focus::List => self.input(Purpose::NewKit),
            Row::Library(_) if self.library_tab().is_some() => {
                let base = self.tree_row().map(|r| r.folder()).unwrap_or_default();
                self.input(Purpose::Mkdir { base });
            }
            _ => {}
        }
    }

    /// `c`: a new profile with the highlighted one's variables, kits and settings.
    fn ask_copy(&mut self) {
        if let (Row::Profile(n), Focus::List) = (self.row(), self.focus) {
            self.input(Purpose::NewProfile { from: Some(n) });
        }
    }

    /// `i`: what `ccs migrate`, `ccs import` and `ccs import aws` can do here.
    fn ask_import(&mut self, b: &dyn Browser) {
        let mut items = Vec::new();
        if b.needs_migrate() {
            items.push(ImportItem::Migrate);
        }
        items.push(ImportItem::Claude);
        let aws = b.aws_profiles().unwrap_or_default();
        let any = !aws.is_empty();
        items.extend(aws.into_iter().map(|r| ImportItem::Aws { aws: r.aws, imported_as: r.imported_as }));
        if any {
            items.push(ImportItem::AllAws);
        }
        self.pick_scroll.reset();
        self.mode = Mode::Pick { purpose: PickFor::Import(items), filter: String::new(), sel: 0 };
    }

    /// Run one import, then show every note it made.
    fn import(&mut self, b: &dyn Browser, item: ImportItem) {
        let outcome = |r: Result<Vec<AwsImport>, crate::ports::Error>| r.map(|o| o.iter().map(ToString::to_string).collect::<Vec<_>>());
        let (title, result) = match item {
            ImportItem::Migrate => ("migrate", b.migrate().map(|n| if n.is_empty() { vec!["Nothing to migrate.".into()] } else { n })),
            ImportItem::Claude => (
                "import ~/.claude",
                b.import_claude().map(|mut n| {
                    n.push("~/.claude is in the library and in kit imported; nothing in ~/.claude changed.".into());
                    n
                }),
            ),
            ImportItem::Aws { aws, .. } => ("import aws", outcome(b.import_aws(&[aws], false))),
            ImportItem::AllAws => ("import aws --all", outcome(b.import_aws(&[], true))),
        };
        match result {
            Ok(lines) => self.mode = Mode::View { title: title.into(), text: lines.join("\n"), scroll: 0 },
            Err(e) => self.message = Some(e.to_string()),
        }
        self.refresh(b);
    }

    /// The marked library rows, or the highlighted one when none is marked.
    fn targets(&self) -> Vec<Entry> {
        let Some(kind) = self.library_tab() else { return vec![] };
        if self.marks.is_empty() {
            return self.tree_row().map(|r| vec![r.entry(kind)]).unwrap_or_default();
        }
        self.tree.iter().filter(|r| self.marks.contains(&r.path)).map(|r| r.entry(kind)).collect()
    }

    fn ask_move(&mut self) {
        let entries = self.targets();
        if !entries.is_empty() {
            self.mode = Mode::Pick { purpose: PickFor::MoveTo(entries), filter: String::new(), sel: 0 };
        }
    }

    fn ask_kit(&mut self) {
        let entries = self.targets();
        if !entries.is_empty() {
            self.mode = Mode::Pick { purpose: PickFor::AddToKit(entries), filter: String::new(), sel: 0 };
        }
    }

    fn ask_rename(&mut self) {
        match (self.row(), self.focus) {
            (Row::Kit(k), Focus::List) => self.input(Purpose::RenameKit { old: k }),
            (Row::Library(_), Focus::Body) => {
                if let (Some(kind), Some(r)) = (self.tab().kind(), self.tree_row()) {
                    if r.item.is_none() {
                        self.input(Purpose::RenameFolder { kind, folder: r.path });
                    }
                }
            }
            _ => {}
        }
    }

    /// The lines a picker offers for `filter`: the matches first, so typing then Enter
    /// takes a match; a new one from the text last, or alone when nothing matches.
    fn choices(&self, purpose: &PickFor, filter: &str) -> Vec<Choice> {
        let wanted = filter.trim().trim_matches('/');
        let existing: Vec<String> = match purpose {
            PickFor::MoveTo(entries) => {
                // A folder cannot go into itself or anything under it.
                let moving: Vec<&str> = entries.iter().filter_map(|e| if let Entry::Folder(_, f) = e { Some(f.as_str()) } else { None }).collect();
                let folders = self.tree.iter().filter(|r| r.item.is_none()).map(|r| r.path.clone());
                std::iter::once(String::new())
                    .chain(folders)
                    .filter(|f| !moving.iter().any(|m| f == m || f.starts_with(&format!("{m}/"))))
                    .filter(|f| matches(wanted, if f.is_empty() { "/ (top)" } else { f }))
                    .collect()
            }
            PickFor::AddToKit(_) => self.kits.iter().filter(|k| matches(wanted, k)).cloned().collect(),
            PickFor::Import(items) => return items.iter().map(ImportItem::label).filter(|l| matches(wanted, l)).map(Choice::Existing).collect(),
        };
        let new = (!wanted.is_empty() && !existing.iter().any(|e| e == wanted)).then(|| Choice::New(wanted.to_string()));
        existing.into_iter().map(Choice::Existing).chain(new).collect()
    }

    fn chosen(&mut self, b: &dyn Browser, purpose: PickFor, choice: Choice) {
        let kind = self.tab().kind().unwrap_or(ItemKind::Skill);
        let (target, new) = match choice {
            Choice::Existing(t) => (t, false),
            Choice::New(t) => (t, true),
        };
        let (entries, moving) = match purpose {
            PickFor::MoveTo(e) => (e, true),
            PickFor::AddToKit(e) => (e, false),
            PickFor::Import(items) => {
                if let Some(item) = items.into_iter().find(|i| i.label() == target) {
                    self.import(b, item);
                }
                return;
            }
        };
        let mut failed = Vec::new();
        if !moving && new {
            if let Err(e) = b.new_kit(&target) {
                self.message = Some(e.to_string());
                return;
            }
        }
        for e in &entries {
            let r = if moving { b.move_entry(e, &target) } else { b.set_use(&target, e, true) };
            if let Err(err) = r {
                failed.push(format!("{}: {err}", e.line()));
            }
        }
        let done = entries.len() - failed.len();
        let what = if entries.len() == 1 { entries[0].line() } else { done.to_string() };
        let mut msg = if moving { format!("moved {what} to {}", folder_label(kind, &target)) } else { format!("{target} now picks {what}") };
        if !failed.is_empty() {
            msg += &format!(" · {}", failed.join(" · "));
        }
        self.message = Some(msg);
        self.marks.clear();
        self.refresh(b);
    }

    fn ask_delete(&mut self) {
        let pending = match (self.row(), self.focus) {
            (Row::Kit(k), Focus::List) => Pending::DeleteKit(k),
            (Row::Library(_), Focus::Body) => match self.targets() {
                t if t.is_empty() => return,
                t => Pending::DeleteEntries(t),
            },
            _ => return,
        };
        self.message = Some(match &pending {
            Pending::DeleteKit(k) => format!("Delete kit {k}? y/n"),
            Pending::DeleteEntries(e) if e.len() == 1 => format!("Delete {} from the library? y/n", e[0].line()),
            Pending::DeleteEntries(e) => format!("Delete {} entries from the library? y/n", e.len()),
        });
        self.mode = Mode::Confirm(pending);
    }

    fn submit(&mut self, b: &dyn Browser, purpose: Purpose, text: &str) {
        let kind = self.tab().kind().unwrap_or(ItemKind::Skill);
        let result = match purpose {
            Purpose::Add { folder } => b.add(text, &folder).map(|(k, n)| format!("added {}/{n} to the library", k.dir())),
            Purpose::Mkdir { base } => {
                let name = text.trim_matches('/');
                let folder = if base.is_empty() { name.to_string() } else { format!("{base}/{name}") };
                b.mkdir(kind, &folder).map(|()| format!("made {}", folder_label(kind, &folder)))
            }
            Purpose::NewKit => {
                let made = b.new_kit(text);
                if made.is_ok() {
                    self.refresh(b);
                    self.select(&Row::Kit(text.into()));
                }
                made.map(|()| format!("made kit {text}; pick its skills on its Skills tab"))
            }
            Purpose::NewProfile { from } => {
                let made = b.new_profile(text, from.as_ref());
                if made.is_ok() {
                    self.refresh(b);
                    if let Some(n) = self.profiles.iter().find(|p| p.name.as_str() == text).map(|p| p.name.clone()) {
                        self.select(&Row::Profile(n));
                    }
                }
                made.map(|()| match &from {
                    Some(f) => format!("copied {} to {text}: variables, kits and settings, not the login", f.as_str()),
                    None => format!("made profile {text}; e edits its variables, ⏎ launches it to log in"),
                })
            }
            Purpose::RenameKit { old } => b.rename_kit(&old, text).map(|()| format!("kit {old} is now {text}")),
            Purpose::RenameFolder { kind, folder } => {
                b.rename_folder(kind, &folder, text).map(|()| format!("renamed {} to {text}; kits follow it", folder_label(kind, &folder)))
            }
        };
        self.message = Some(result.unwrap_or_else(|e| e.to_string()));
        self.refresh(b);
    }

    fn hints(&self) -> &'static str {
        match (self.focus, self.row(), self.tab()) {
            (Focus::List, Row::Profile(_), _) => "⏎ launch · t shell · n new · c copy · e edit · d default · i import · ←→ tab · 1 2 3 section · q quit",
            (Focus::List, Row::ProfilesHeading, _) => "n new profile · i import · 1 2 3 section · q quit",
            (Focus::List, Row::KitsHeading, _) => "n new kit · i import · 1 2 3 section · q quit",
            (Focus::List, Row::Kit(_), Tab::Instructions) => "↑↓ move · ←→ tab · e edit · n new kit · r rename · x delete kit · 1 2 3 section · q quit",
            (Focus::List, Row::Kit(_), _) => "↑↓ move · ←→ tab · tab focus · n new kit · r rename · x delete kit · 1 2 3 section · q quit",
            (Focus::List, Row::Library(_), _) => "1 2 3 section · ↑↓ move · tab focus · i import · q quit",
            (Focus::Body, Row::Profile(_), _) => "space attach · ↑↓ move · tab back · q quit",
            (Focus::Body, Row::Kit(_), Tab::Profiles) => "space attach · ↑↓ move · tab back · q quit",
            (Focus::Body, Row::Kit(_), _) => "space pick · p picked only · e edit · v view · / search · tab back",
            (Focus::Body, _, _) => "space mark · m move · + to kit · a add · n folder · r rename · x delete · e edit · v view · /",
        }
    }
}

/// `[x] name` with a green tick, or `[ ] name`.
fn tick(on: bool, name: &str) -> Line<'static> {
    if on {
        Line::from(vec![Span::styled("[x] ", Style::default().fg(Color::Green)), Span::raw(name.to_string())])
    } else {
        Line::from(format!("[ ] {name}"))
    }
}

fn border(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    }
}

fn overview_lines(o: &ProfileOverview) -> Vec<Line<'static>> {
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let under = Style::default().add_modifier(Modifier::UNDERLINED);
    let mut lines = vec![Line::styled(o.segment.clone(), bold), Line::from("")];
    let kits: String = o.kits.iter().map(|k| format!("+{k} ")).collect();
    lines.push(Line::from(if kits.is_empty() { "Kits: none (attach on the Kits tab)".to_string() } else { format!("Kits: {}", kits.trim_end()) }));
    lines.push(Line::from(vec![
        Span::raw(format!("Folder: {}  ", o.dir.display())),
        Span::styled("t opens a shell here", Style::default().fg(Color::Cyan)),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::styled("Variables", under));
    if o.env.is_empty() {
        lines.push(Line::from("  (none)"));
    }
    lines.extend(o.env.iter().map(|(k, v)| Line::from(format!("  {k}={v}"))));
    lines.push(Line::from(""));
    lines.push(Line::styled("Doctor", under));
    lines.extend(o.findings.iter().map(|f| match f {
        Finding::Pass(m) => Line::styled(format!("  ✓ {m}"), Style::default().fg(Color::Green)),
        Finding::Warn(m) => Line::styled(format!("  ! {m}"), Style::default().fg(Color::Yellow)),
        Finding::Fail(m) => Line::styled(format!("  ✗ {m}"), Style::default().fg(Color::Red)),
    }));
    lines
}

const PROFILES_TEXT: &str = "\
A profile is a login: a claude.ai account, or an AWS Bedrock account. It keeps
its variables in profile.env and its own Claude Code config directory.

n makes an empty profile; e on it edits its variables; ⏎ launches it, where
/login signs in. On a profile, c copies it: variables, kits and settings,
never the login. i imports your AWS CLI profiles as Bedrock profiles.";

const KITS_TEXT: &str = "\
A kit is a named set of skills and agents from the library, with optional
instructions (a CLAUDE.md). A profile launches with the kits attached to it;
`ccs <profile> +kit` picks others for one run.

n makes a kit. On a kit, the Skills and Agents tabs pick from the library:
space marks an item or a whole folder.";

/// Below this width the list and the body stack, so neither is cut off.
const NARROW: u16 = 70;

/// Paint the whole screen from `state`.
pub fn draw(frame: &mut Frame, state: &State) {
    let [main, footer] = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(frame.area());
    // Side by side when there is room; a narrow terminal stacks the list over the body.
    let [left, right] = if main.width >= NARROW {
        Layout::horizontal([Constraint::Length(30), Constraint::Min(20)]).areas(main)
    } else {
        let list = u16::try_from(state.rows().len() + 3).unwrap_or(u16::MAX).min(main.height / 2);
        Layout::vertical([Constraint::Length(list), Constraint::Min(5)]).areas(main)
    };
    draw_list(frame, state, left);

    let [tabs_area, body] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(right);
    let row_tabs = tabs(&state.row());
    let titles: Vec<String> = row_tabs
        .iter()
        .map(|t| match t {
            Tab::Skills => format!("Skills ({})", state.counts[0]),
            Tab::Agents => format!("Agents ({})", state.counts[1]),
            t => t.title().to_string(),
        })
        .collect();
    let tabs_widget = Tabs::new(titles)
        .select(state.tab.min(row_tabs.len() - 1))
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    frame.render_widget(tabs_widget, tabs_area);
    draw_body(frame, state, body);

    let kind = state.tab().kind().unwrap_or(ItemKind::Skill);
    let text = match &state.mode {
        Mode::Input { purpose, text, .. } => match purpose {
            Purpose::Add { folder } => {
                format!("Add to the library into {}: {text}▏  tab complete · enter add · esc cancel", folder_label(kind, folder))
            }
            Purpose::Mkdir { base } => format!("New folder in {}: {text}▏  enter make · esc cancel", folder_label(kind, base)),
            Purpose::NewKit => format!("New kit name: {text}▏  enter make · esc cancel"),
            Purpose::NewProfile { from: None } => format!("New profile name: {text}▏  enter make · esc cancel"),
            Purpose::NewProfile { from: Some(f) } => format!("Copy {} to: {text}▏  enter copy · esc cancel", f.as_str()),
            Purpose::RenameKit { old } => format!("Rename kit {old} to: {text}▏  enter rename · esc cancel"),
            Purpose::RenameFolder { kind, folder } => {
                format!("Rename {} to: {text}▏  enter rename · esc cancel", folder_label(*kind, folder))
            }
        },
        Mode::Pick { purpose, filter, .. } => match purpose {
            PickFor::MoveTo(e) => format!("Move {} to folder: {filter}▏  type to filter or name a new one · enter move · esc cancel", count(e)),
            PickFor::AddToKit(e) => format!("Add {} to kit: {filter}▏  type to filter or name a new one · enter add · esc cancel", count(e)),
            PickFor::Import(_) => format!("Import: {filter}▏  type to filter · enter import · esc cancel"),
        },
        Mode::Search => {
            let f = if state.focus == Focus::List { &state.list_filter } else { &state.body_filter };
            format!("/ {f}▏  enter keep · esc clear")
        }
        _ => state.message.clone().unwrap_or_else(|| state.hints().into()),
    };
    frame.render_widget(Paragraph::new(text).style(Style::default().fg(Color::DarkGray)), footer);
}

fn draw_list(frame: &mut Frame, state: &State, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" ccs ").border_style(border(state.focus == Focus::List));
    let rows = state.rows();
    let items: Vec<ListItem> = rows
        .iter()
        .map(|r| match r {
            Row::Profile(n) => {
                let Some(p) = state.profiles.iter().find(|p| p.name == *n) else { return ListItem::new(n.as_str().to_string()) };
                let mut s = p.name.as_str().to_string();
                for k in &p.kits {
                    s += &format!(" +{k}");
                }
                if p.is_default {
                    s += " (default)";
                }
                if p.current {
                    s += " *";
                }
                ListItem::new(s)
            }
            Row::ProfilesHeading => ListItem::new("none yet · n makes one").style(Style::default().fg(Color::DarkGray)),
            Row::KitsHeading => ListItem::new("none yet · n makes one").style(Style::default().fg(Color::DarkGray)),
            Row::Kit(k) => ListItem::new(k.clone()),
            Row::Library(kind) => ListItem::new(format!("{}s", kind.label())),
        })
        .collect();
    let [bar, list] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
    let mut spans = vec![Span::raw(" ")];
    for (i, sec) in SECTIONS.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
        }
        spans.push(if *sec == state.section {
            Span::styled(sec.label(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD | Modifier::UNDERLINED))
        } else {
            Span::raw(sec.label())
        });
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), bar);
    render_list(frame, list, block, items, Some(state.sel), &state.list_scroll, Some("▶ "));
}

fn draw_body(frame: &mut Frame, state: &State, area: Rect) {
    let block = Block::default().borders(Borders::ALL).border_style(border(state.focus == Focus::Body));
    if let Mode::View { title, text, scroll } = &state.mode {
        let visible = area.height.saturating_sub(2) as usize;
        state.view_scroll.page.set(visible);
        let lines = text.lines().count();
        let at = format!(" {}–{} of {lines} ", *scroll as usize + 1, (*scroll as usize + visible).min(lines));
        let block = block.title(format!(" {title} · esc back ")).title_bottom(Line::from(at).right_aligned());
        frame.render_widget(Paragraph::new(text.as_str()).block(block).scroll((*scroll, 0)), area);
        scrollbar(frame, area, lines, visible, *scroll as usize);
        return;
    }
    if let Mode::Input { candidates, .. } = &state.mode {
        if !candidates.is_empty() {
            let rows: Vec<ListItem> = candidates.iter().map(|c| ListItem::new(c.as_str())).collect();
            frame.render_widget(List::new(rows).block(block.title(" Completions ")), area);
            return;
        }
    }
    if let Mode::Pick { purpose, filter, sel } = &state.mode {
        let kind = state.tab().kind().unwrap_or(ItemKind::Skill);
        let rows: Vec<ListItem> = state
            .choices(purpose, filter)
            .iter()
            .map(|c| {
                ListItem::new(match (c, purpose) {
                    (Choice::New(t), PickFor::MoveTo(_)) => format!("+ new folder: {t}"),
                    (Choice::New(t), PickFor::AddToKit(_)) => format!("+ new kit: {t}"),
                    (Choice::Existing(f), PickFor::MoveTo(_)) if f.is_empty() => "/ (top)".to_string(),
                    (Choice::Existing(f), PickFor::MoveTo(_)) => folder_label(kind, f),
                    (Choice::Existing(k), PickFor::AddToKit(_) | PickFor::Import(_)) => k.clone(),
                    (Choice::New(t), PickFor::Import(_)) => t.clone(),
                })
            })
            .collect();
        let title = match purpose {
            PickFor::MoveTo(_) => " Move to ",
            PickFor::AddToKit(_) => " Add to kit ",
            PickFor::Import(_) => " Import ",
        };
        render_list(frame, area, block.title(title), rows, Some(*sel), &state.pick_scroll, None);
        return;
    }
    let selected = (state.focus == Focus::Body).then_some(state.body_sel);
    match (state.row(), state.tab()) {
        (Row::Profile(_), Tab::Overview) => {
            let p = match &state.overview {
                Some(Ok(o)) => Paragraph::new(overview_lines(o)),
                Some(Err(e)) => Paragraph::new(e.as_str()),
                None => Paragraph::new(""),
            };
            frame.render_widget(p.block(block), area);
        }
        (Row::Profile(n), Tab::Kits) => {
            if state.kits.is_empty() {
                frame.render_widget(Paragraph::new("No kits yet. Select ── kits ── and press n.").block(block), area);
                return;
            }
            let mine = state.profiles.iter().find(|p| p.name == n).map(|p| p.kits.clone()).unwrap_or_default();
            let rows: Vec<ListItem> =
                state.kits.iter().map(|k| ListItem::new(tick(mine.contains(k), k))).collect();
            render_list(frame, area, block, rows, selected, &state.body_scroll, None);
        }
        (Row::ProfilesHeading, _) => frame.render_widget(Paragraph::new(PROFILES_TEXT).block(block), area),
        (Row::KitsHeading, _) => frame.render_widget(Paragraph::new(KITS_TEXT).block(block), area),
        (Row::Kit(k), Tab::Profiles) => {
            if state.profiles.is_empty() {
                frame.render_widget(Paragraph::new("No profiles yet: n on ── profiles ── makes one").block(block), area);
                return;
            }
            let rows: Vec<ListItem> = state
                .profiles
                .iter()
                .map(|p| ListItem::new(tick(p.kits.contains(&k), p.name.as_str())))
                .collect();
            render_list(frame, area, block, rows, selected, &state.body_scroll, None);
        }
        (Row::Kit(_), Tab::Instructions) => {
            let text = state.instructions.clone().unwrap_or_else(|| {
                "No instructions yet; e writes them. They reach Claude as an appended system prompt, in every session that loads this kit.".into()
            });
            frame.render_widget(Paragraph::new(text).block(block), area);
        }
        (row, tab) => {
            let Some(kind) = tab.kind() else { return };
            let tree = state.visible_tree();
            if tree.is_empty() {
                let empty = if state.body_filter.is_empty() {
                    format!("No {}s in the library. Add one: ◆ library, then a.", kind.label())
                } else {
                    format!("No {} matches '{}'.", kind.label(), state.body_filter)
                };
                frame.render_widget(Paragraph::new(empty).block(block), area);
                return;
            }
            let width = tree.iter().map(|r| 2 * r.depth + r.leaf().chars().count() + 1).max().unwrap_or(0);
            let rows: Vec<ListItem> = tree
                .iter()
                .map(|r| {
                    let mark = match &row {
                        Row::Kit(_) => match pick(&state.uses, kind, r) {
                            Pick::Direct => Span::styled("[x] ", Style::default().fg(Color::Green)),
                            Pick::Through(_) => Span::styled("[·] ", Style::default().fg(Color::Yellow)),
                            Pick::No => Span::raw("[ ] "),
                        },
                        _ if state.marks.contains(&r.path) => Span::styled("● ", Style::default().fg(Color::Cyan)),
                        _ => Span::raw("  "),
                    };
                    let name = if r.item.is_some() { r.leaf().to_string() } else { format!("{}/", r.leaf()) };
                    let shown = format!("{}{name}", "  ".repeat(r.depth));
                    let desc = r.item.as_ref().and_then(|i| i.description.clone()).unwrap_or_default();
                    // On the library, the kits that pick each row, before the description so it never cuts them off.
                    let used: String = match &row {
                        Row::Library(_) => state.all_uses.iter().filter(|(_, u)| pick(u, kind, r) != Pick::No).map(|(k, _)| format!("‹{k}› ")).collect(),
                        _ => String::new(),
                    };
                    let name_style = if r.item.is_none() { Style::default().add_modifier(Modifier::BOLD) } else { Style::default() };
                    ListItem::new(Line::from(vec![
                        mark,
                        Span::styled(format!("{shown:width$}  "), name_style),
                        Span::styled(used, Style::default().fg(Color::Magenta)),
                        Span::styled(desc, Style::default().fg(Color::DarkGray)),
                    ]))
                })
                .collect();
            render_list(frame, area, block, rows, selected, &state.body_scroll, None);
        }
    }
}

fn map_key(code: KeyCode) -> Option<Key> {
    Some(match code {
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Tab | KeyCode::BackTab => Key::Tab,
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::Char(c) => Key::Char(c),
        _ => return None,
    })
}

// ── The launch screen (ADR-2610261643 §10) ──────────────────────────────────

/// What a bare `ccs` shows: a provider (profile) on the left, kits on the right.
pub struct Picker {
    profiles: Vec<ProfileRow>,
    /// Every kit, with how many items it picks.
    kits: Vec<(String, usize)>,
    sel: usize,
    kit_sel: usize,
    focus: Focus,
    /// The kits to launch with, in order: the profile's own first.
    ticked: Vec<String>,
    message: Option<String>,
    profile_scroll: Scroll,
    kit_scroll: Scroll,
}

impl Picker {
    /// `preselect` is the resolved profile, when there is one.
    pub fn new(b: &dyn Browser, preselect: Option<&str>) -> Picker {
        let trees = [ItemKind::Skill, ItemKind::Agent].map(|kind| {
            let (items, folders) = b.library(kind);
            (kind, library_tree(&items, &folders))
        });
        let kits = b
            .kits()
            .into_iter()
            .map(|k| {
                let uses = b.kit_uses(&k).unwrap_or_default();
                let n = trees.iter().map(|(kind, t)| t.iter().filter(|r| r.item.is_some() && pick(&uses, *kind, r) != Pick::No).count()).sum();
                (k, n)
            })
            .collect();
        let profiles = b.profiles();
        let sel = preselect.and_then(|p| profiles.iter().position(|r| r.name.as_str() == p)).unwrap_or(0);
        let mut picker = Picker {
            profiles,
            kits,
            sel,
            kit_sel: 0,
            focus: Focus::List,
            ticked: vec![],
            message: None,
            profile_scroll: Scroll::default(),
            kit_scroll: Scroll::default(),
        };
        picker.tick_defaults();
        if picker.profiles.is_empty() {
            picker.message = Some("No profiles yet: ccs new <name>, or ccs import aws".into());
        }
        picker
    }

    /// Tick the selected profile's own kits, as long as they exist.
    fn tick_defaults(&mut self) {
        self.ticked = match self.profiles.get(self.sel) {
            Some(p) => p.kits.iter().filter(|k| self.kits.iter().any(|(n, _)| n == *k)).cloned().collect(),
            None => vec![],
        };
        self.kit_scroll.reveal(self.kits.iter().position(|(k, _)| self.ticked.contains(k)));
    }

    /// Apply one key. `Some` ends the screen.
    pub fn handle(&mut self, b: &dyn Browser, key: Key) -> Option<Exit> {
        self.message = None;
        match key {
            Key::Char('q') | Key::Esc => return Some(Exit::Quit),
            k if movement(k, 0).is_some() => match self.focus {
                Focus::List => {
                    let delta = movement(k, self.profile_scroll.page.get()).unwrap_or(0);
                    self.profile_scroll.follow(k, delta, self.profiles.len());
                    let to = shifted(self.sel, delta, self.profiles.len());
                    if to != self.sel {
                        self.sel = to;
                        self.tick_defaults();
                    }
                }
                Focus::Body => {
                    let delta = movement(k, self.kit_scroll.page.get()).unwrap_or(0);
                    self.kit_scroll.follow(k, delta, self.kits.len());
                    self.kit_sel = shifted(self.kit_sel, delta, self.kits.len());
                }
            },
            Key::Tab if !self.kits.is_empty() => {
                self.focus = if self.focus == Focus::List { Focus::Body } else { Focus::List };
                self.kit_sel = self.kit_scroll.in_view(self.kit_sel);
            }
            Key::Char(' ') if self.focus == Focus::Body => {
                if let Some((k, _)) = self.kits.get(self.kit_sel) {
                    if !self.ticked.contains(k) {
                        self.ticked.push(k.clone());
                    } else {
                        self.ticked.retain(|t| t != k);
                    }
                }
            }
            Key::Char('s') => {
                let p = self.profiles.get(self.sel).map(|p| p.name.clone())?;
                self.message = Some(match b.set_kits(&p, &self.ticked) {
                    Ok(()) if self.ticked.is_empty() => format!("{} now starts with no kits", p.as_str()),
                    Ok(()) => format!("{} now starts with {}", p.as_str(), self.ticked.iter().map(|k| format!("+{k}")).collect::<Vec<_>>().join(" ")),
                    Err(e) => e.to_string(),
                });
                self.profiles = b.profiles();
            }
            Key::Enter => {
                let p = self.profiles.get(self.sel)?;
                return Some(Exit::LaunchWith { profile: p.name.as_str().into(), kits: self.ticked.clone() });
            }
            _ => {}
        }
        None
    }
}

/// Paint the launch screen.
pub fn draw_picker(frame: &mut Frame, p: &Picker) {
    let [main, footer] = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(frame.area());
    let [left, right] = if main.width >= NARROW {
        Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).areas(main)
    } else {
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(main)
    };
    let block = Block::default().borders(Borders::ALL).title(" Provider ").border_style(border(p.focus == Focus::List));
    if p.profiles.is_empty() {
        frame.render_widget(Paragraph::new("No profiles yet.").block(block), left);
    } else {
        let width = p.profiles.iter().map(|r| r.name.as_str().chars().count()).max().unwrap_or(0);
        let rows: Vec<ListItem> = p
            .profiles
            .iter()
            .map(|r| {
                ListItem::new(Line::from(vec![
                    Span::raw(format!("{:width$}  ", r.name.as_str())),
                    Span::styled(r.provider.clone(), Style::default().fg(Color::Cyan)),
                    Span::styled(if r.is_default { "  (default)" } else { "" }, Style::default().fg(Color::DarkGray)),
                ]))
            })
            .collect();
        render_list(frame, left, block, rows, Some(p.sel), &p.profile_scroll, Some("▶ "));
    }

    let block = Block::default().borders(Borders::ALL).title(" Kits ").border_style(border(p.focus == Focus::Body));
    if p.kits.is_empty() {
        frame.render_widget(Paragraph::new("No kits yet: ccs ui, then n on ── kits ──").block(block), right);
    } else {
        let width = p.kits.iter().map(|(k, _)| k.chars().count()).max().unwrap_or(0);
        let rows: Vec<ListItem> = p
            .kits
            .iter()
            .map(|(k, n)| {
                let mut line = tick(p.ticked.contains(k), &format!("{k:width$}  "));
                line.spans.push(Span::styled(n.to_string(), Style::default().fg(Color::DarkGray)));
                ListItem::new(line)
            })
            .collect();
        render_list(frame, right, block, rows, (p.focus == Focus::Body).then_some(p.kit_sel), &p.kit_scroll, None);
    }

    let hints = "↑↓ PgUp/PgDn choose · tab switch · space tick a kit · ⏎ launch · s save kits as this profile's default · q quit";
    let text = p.message.clone().unwrap_or_else(|| hints.into());
    frame.render_widget(Paragraph::new(text).style(Style::default().fg(Color::DarkGray)), footer);
}

/// A full-screen view the run loop can drive.
trait Screen {
    fn key(&mut self, b: &dyn Browser, key: Key) -> Option<Exit>;
    fn paint(&self, frame: &mut Frame);
    /// Hand the terminal to the editor, if a key asked for it.
    fn edit(&mut self, _b: &dyn Browser) -> bool {
        false
    }
}

impl Screen for State {
    fn key(&mut self, b: &dyn Browser, key: Key) -> Option<Exit> {
        self.handle(b, key)
    }
    fn paint(&self, frame: &mut Frame) {
        draw(frame, self)
    }
    fn edit(&mut self, b: &dyn Browser) -> bool {
        if !self.wants_editor() {
            return false;
        }
        // The editor needs the real terminal: leave the alternate screen, then come back.
        close_terminal();
        self.run_editor(b);
        true
    }
}

impl Screen for Picker {
    fn key(&mut self, b: &dyn Browser, key: Key) -> Option<Exit> {
        self.handle(b, key)
    }
    fn paint(&self, frame: &mut Frame) {
        draw_picker(frame, self)
    }
}

/// The launch screen of a bare `ccs`. The terminal is restored before this returns.
pub fn run_picker(b: &dyn Browser, preselect: Option<&str>) -> std::io::Result<Exit> {
    drive(b, &mut Picker::new(b, preselect))
}

/// Take over the terminal until the user quits or picks a profile to launch.
/// The terminal is restored before this returns, on every path.
pub fn run(b: &dyn Browser) -> std::io::Result<Exit> {
    drive(b, &mut State::new(b))
}

/// The alternate screen with the mouse wheel on; `ratatui::init` leaves the mouse off.
fn open_terminal() -> ratatui::DefaultTerminal {
    let terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
    terminal
}

fn close_terminal() {
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
}

fn drive(b: &dyn Browser, state: &mut dyn Screen) -> std::io::Result<Exit> {
    let mut terminal = open_terminal();
    let result = loop {
        if let Err(e) = terminal.draw(|f| state.paint(f)) {
            break Err(e);
        }
        match event::read() {
            Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => {
                if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
                    break Ok(Exit::Quit);
                }
                if let Some(exit) = map_key(k.code).and_then(|key| state.key(b, key)) {
                    break Ok(exit);
                }
                if state.edit(b) {
                    terminal = open_terminal();
                }
            }
            Ok(Event::Mouse(m)) => {
                let key = match m.kind {
                    MouseEventKind::ScrollDown => Key::WheelDown,
                    MouseEventKind::ScrollUp => Key::WheelUp,
                    _ => continue,
                };
                if let Some(exit) = state.key(b, key) {
                    break Ok(exit);
                }
            }
            Ok(_) => {}
            Err(e) => break Err(e),
        }
    };
    // Mouse capture off first, or the shell receives every later mouse move as text.
    close_terminal();
    result
}
