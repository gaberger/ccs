//! The ratatui TUI (ADR-2610261255; rows, trees and keys in ADR-2610261643 §8)
//! — a primary adapter.
//!
//! Rule 4: it imports `ports/` only. Everything it does goes through the
//! `Browser` driving port. `State` takes keys and `draw` paints it, so tests
//! drive the screen without a terminal; `run` is the only part that touches one.

use crate::ports::{library_tree, pick, resolve_kit, AwsImport, LibraryItem, Browser, Entry, Finding, ItemKind, Name, Pick, ProfileOverview, ProfileRow, TreeRow};
use std::collections::BTreeSet;
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::crossterm::execute;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Tabs};
use ratatui::Frame;
use std::cell::{Cell, RefCell};

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
    /// A left click at a column and row of the screen (§20).
    Click(u16, u16),
    /// A right click: back one page; never quits.
    Back,
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

    /// Where the cursor lands when a list takes the focus: the first ticked row
    /// when it is in view (it opens, so it should be the one you use), else `sel`
    /// when in view, else the top row in view. The view stays as it was.
    fn in_view(&self, ticked: Option<usize>, sel: usize) -> usize {
        let (top, page) = (self.offset.get(), self.page.get().max(1));
        let seen = |i: &usize| (top..top + page).contains(i);
        ticked.filter(seen).or(Some(sel).filter(seen)).unwrap_or(top)
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

/// Every kind of library entry, in the order the TUI shows them.
const KINDS: [ItemKind; 3] = [ItemKind::Skill, ItemKind::Agent, ItemKind::Plugin];

/// What the State's row model lists: a kit, for its library picker, or the library's kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Kits,
    Library,
}

/// The library page's subject: a kit it picks for, or one kind of the library.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Row {
    Kit(String),
    Library(ItemKind),
}

/// The library page's kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Skills,
    Agents,
    Plugins,
}

impl Tab {
    fn kind(self) -> Option<ItemKind> {
        Some(match self {
            Tab::Skills => ItemKind::Skill,
            Tab::Agents => ItemKind::Agent,
            Tab::Plugins => ItemKind::Plugin,
        })
    }
}

fn tabs(row: &Row) -> &'static [Tab] {
    match row {
        Row::Kit(_) => &[Tab::Skills, Tab::Agents, Tab::Plugins],
        Row::Library(ItemKind::Skill) => &[Tab::Skills],
        Row::Library(ItemKind::Agent) => &[Tab::Agents],
        Row::Library(ItemKind::Plugin) => &[Tab::Plugins],
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

/// Something on screen a click can land on, recorded as each frame is drawn (§20).
#[derive(Debug, Clone)]
enum Hit {
    /// A list. Line `n` of its rows' area is `map[top + n]`: a row and whether it
    /// has a `[ ]` box, or `None` for a label. The box starts at column `indent`.
    List { list: ListId, top: usize, indent: usize, map: Vec<Option<(usize, bool)>> },
    /// A breadcrumb part: keep this many pages.
    Crumb(usize),
    /// A kind tab of the library.
    Kind(usize),
    /// Home's `3 · ⏎ start …`.
    Start,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListId {
    HomeProfiles,
    HomeKits,
    Page,
    Library,
    Picker,
}

/// The rows' area of a bordered box.
fn inside(area: Rect) -> Rect {
    area.inner(Margin { vertical: 1, horizontal: 1 })
}

/// A page above Home (ADR-2610261643 §18). Esc returns to the one below.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Page {
    Profile(Name),
    Kit(String),
    /// The library: a picker for `kit`, or, with `None`, to manage it.
    Library { kit: Option<String> },
}

/// A row of a profile's page.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ProfileItem {
    Launch,
    Kit(String),
    Variables,
    Default,
    Copy,
    Shell,
}

/// A row of a kit's page.
#[derive(Debug, Clone, PartialEq, Eq)]
enum KitItem {
    Entry(Entry),
    Add,
    Instructions,
    Profile(Name),
    Rename,
    Delete,
}

pub struct State {
    /// Where the last frame put each clickable thing.
    hits: RefCell<Vec<(Rect, Hit)>>,
    /// Home: the launch screen (§10), the page under every other.
    home: Picker,
    /// The pages opened from Home, each with the row it was on.
    stack: Vec<(Page, usize)>,
    page_scroll: Scroll,
    /// The open profile page's overview.
    page_overview: Option<Result<ProfileOverview, String>>,
    /// The open kit page's instructions.
    page_text: Option<String>,
    /// Every library item, for the counts on a kit's page.
    library: Vec<LibraryItem>,
    profiles: Vec<ProfileRow>,
    kits: Vec<String>,
    sel: usize,
    list_filter: String,
    tab: usize,
    focus: Focus,
    body_sel: usize,
    body_filter: String,
    /// The library tree for the tab's kind.
    tree: Vec<TreeRow>,
    /// Item counts per kind: the library's, or what the selected kit picks.
    counts: [usize; 3],
    /// Every kit's `uses`, for the labels on library rows.
    all_uses: Vec<(String, Vec<Entry>)>,
    /// What each kit holds, as `kit_detail` lines, in `kits` order.
    kit_details: Vec<Vec<String>>,
    /// Marked library rows, by path, for `m`, `k` and `x`.
    marks: BTreeSet<String>,
    /// On a kit: show only what it picks.
    picked_only: bool,
    /// The selected kit's `uses`.
    uses: Vec<Entry>,
    mode: Mode,
    message: Option<String>,
    editing: Option<EditTarget>,
    /// Which rows the State's model lists: a kit's, for its library picker, or the library's kinds.
    section: Section,
    list_scroll: Scroll,
    body_scroll: Scroll,
    pick_scroll: Scroll,
    /// The `v` view's height, for paging.
    view_scroll: Scroll,
}

impl State {
    pub fn new(b: &dyn Browser) -> State {
        let current = b.profiles().into_iter().find(|p| p.current).map(|p| p.name.as_str().to_string());
        let mut s = State {
            hits: RefCell::new(vec![]),
            home: Picker::new(b, current.as_deref()),
            stack: vec![],
            page_scroll: Scroll::default(),
            page_overview: None,
            page_text: None,
            library: vec![],
            profiles: vec![],
            kits: vec![],
            sel: 0,
            list_filter: String::new(),
            tab: 0,
            focus: Focus::List,
            body_sel: 0,
            body_filter: String::new(),
            tree: vec![],
            counts: [0; 3],
            all_uses: vec![],
            kit_details: vec![],
            marks: BTreeSet::new(),
            picked_only: false,
            uses: vec![],
            mode: Mode::Normal,
            message: None,
            editing: None,
            section: Section::Library,
            list_scroll: Scroll::default(),
            body_scroll: Scroll::default(),
            pick_scroll: Scroll::default(),
            view_scroll: Scroll::default(),
        };
        s.refresh(b);
        s
    }

    fn rows(&self) -> Vec<Row> {
        let f = &self.list_filter;
        let rows: Vec<Row> = match self.section {
            Section::Kits => self.kits.iter().filter(|k| matches(f, k)).map(|k| Row::Kit(k.clone())).collect(),
            Section::Library => KINDS.iter().map(|k| Row::Library(*k)).collect(),
        };
        if rows.is_empty() {
            return vec![Row::Library(ItemKind::Skill)];
        }
        rows
    }

    fn row(&self) -> Row {
        let rows = self.rows();
        rows.get(self.sel).or(rows.last()).cloned().unwrap_or(Row::Library(ItemKind::Skill))
    }

    fn tab(&self) -> Tab {
        let t = tabs(&self.row());
        t[self.tab.min(t.len() - 1)]
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
        match self.row() {
            Row::Kit(_) => {
                let kind = self.tab().kind()?;
                self.visible_tree().iter().position(|r| pick(&self.uses, kind, r) != Pick::No)
            }
            Row::Library(_) => None,
        }
    }

    fn body_len(&self) -> usize {
        self.visible_tree().len()
    }

    /// Reload everything the selected row and tab show.
    fn refresh(&mut self, b: &dyn Browser) {
        self.profiles = b.profiles();
        self.kits = b.kits();
        self.sel = self.sel.min(self.rows().len() - 1);
        let row = self.row();
        self.uses = match &row {
            Row::Kit(k) => b.kit_uses(k).unwrap_or_default(),
            Row::Library(_) => vec![],
        };
        self.all_uses = self.kits.iter().map(|k| (k.clone(), b.kit_uses(k).unwrap_or_default())).collect();
        let library = whole_library(b);
        self.kit_details = self.all_uses.iter().map(|(k, u)| kit_detail(u, &library, b.kit_instructions(k).is_some())).collect();
        self.library = library;
        self.home.reload(b);
        (self.page_overview, self.page_text) = match self.page() {
            Some(Page::Profile(n)) => (Some(b.overview(&n).map_err(|e| e.to_string())), None),
            Some(Page::Kit(k)) => (None, b.kit_instructions(&k)),
            _ => (None, None),
        };
        let mut trees = KINDS.map(|kind| {
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
        self.tree = match self.tab().kind().and_then(|k| KINDS.iter().position(|x| *x == k)) {
            Some(i) => std::mem::take(&mut trees[i].1),
            None => vec![],
        };
        self.body_sel = self.body_sel.min(self.body_len().saturating_sub(1));
    }

    /// Apply one key. `Some` ends the TUI.
    pub fn handle(&mut self, b: &dyn Browser, key: Key) -> Option<Exit> {
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::View { title, text, scroll } => {
                if matches!(key, Key::Esc | Key::Char('q') | Key::Back) {
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
                // A click picks a row; a click on the picked row chooses it (§20).
                let key = match key {
                    Key::Click(x, y) => match self.hit_at(x, y) {
                        Some((_, Hit::List { list: ListId::Picker, .. }, Some(row))) if row == sel => Key::Enter,
                        Some((_, Hit::List { list: ListId::Picker, .. }, Some(row))) => {
                            sel = row;
                            Key::Back
                        }
                        _ => Key::Back,
                    },
                    Key::Back => Key::Esc,
                    k => k,
                };
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
                            Ok(at) => {
                                // Its pages close: there is nothing left to show.
                                while matches!(self.page(), Some(Page::Kit(x)) | Some(Page::Library { kit: Some(x) }) if x == k) {
                                    self.stack.pop();
                                }
                                format!("moved kit {k} to {}; no profile uses it now", at.display())
                            }
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
            Mode::Normal => {
                self.message = None;
                match key {
                    Key::Click(x, y) => self.click(b, x, y),
                    Key::Back => {
                        if !self.stack.is_empty() {
                            self.pop(b);
                        }
                        None
                    }
                    k => self.dispatch(b, k),
                }
            }
        }
    }

    /// A key on whatever page is open.
    fn dispatch(&mut self, b: &dyn Browser, key: Key) -> Option<Exit> {
        match self.page() {
            None => self.home_key(b, key),
            Some(Page::Profile(n)) => self.profile_key(b, key, n),
            Some(Page::Kit(k)) => self.kit_key(b, key, k),
            Some(Page::Library { kit }) => self.library_key(b, key, kit),
        }
    }

    /// What the last frame drew at a point, and the row a list has there.
    fn hit_at(&self, x: u16, y: u16) -> Option<(Rect, Hit, Option<usize>)> {
        let hits = self.hits.borrow();
        let (r, h) = hits.iter().rev().find(|(r, _)| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height)?;
        let row = match h {
            Hit::List { top, map, .. } => map.get(top + usize::from(y - r.y)).copied().flatten(),
            _ => None,
        };
        Some((*r, h.clone(), row.map(|(row, _)| row)))
    }

    /// Whether a click at column `col` of a list's line `y` lands on a `[ ]` box.
    fn on_box(&self, area: Rect, y: u16, col: usize) -> bool {
        let hits = self.hits.borrow();
        hits.iter().rev().find(|(r, _)| *r == area).is_some_and(|(_, h)| match h {
            Hit::List { top, indent, map, .. } => {
                map.get(top + usize::from(y - area.y)).copied().flatten().is_some_and(|(_, boxed)| boxed) && (*indent..indent + 4).contains(&col)
            }
            _ => false,
        })
    }

    /// A left click (§20): select a row; on the selected row, act as Enter; on a
    /// `[ ]` box, act as space. Never a launch, except on `3 · ⏎ start`.
    fn click(&mut self, b: &dyn Browser, x: u16, y: u16) -> Option<Exit> {
        let (area, hit, row) = self.hit_at(x, y)?;
        let col = usize::from(x - area.x);
        match hit {
            Hit::Crumb(depth) => {
                if depth < self.stack.len() {
                    self.stack.truncate(depth);
                    self.focus = Focus::List;
                    self.page_scroll.reset();
                    self.refresh(b);
                }
                None
            }
            Hit::Kind(i) => {
                if let Some(Page::Library { kit }) = self.page() {
                    self.set_kind(b, kit.is_some(), i);
                }
                None
            }
            Hit::Start => self.home.handle(b, Key::Enter),
            Hit::List { list, .. } => {
                let row = row?;
                let on_box = self.on_box(area, y, col);
                match list {
                    ListId::HomeProfiles => {
                        let again = self.home.focus == Focus::List && self.home.sel == row;
                        self.home.focus = Focus::List;
                        if again {
                            let key = if self.home.action().is_some() { Key::Enter } else { Key::Right };
                            return self.home_key(b, key);
                        }
                        self.home.sel = row;
                        if self.home.profile().is_some_and(|p| Some(&p.name) != self.home.ticks_for.as_ref()) {
                            self.home.tick_defaults();
                        }
                        None
                    }
                    ListId::HomeKits => {
                        let again = self.home.focus == Focus::Body && self.home.kit_sel == row;
                        self.home.focus = Focus::Body;
                        self.home.kit_sel = row;
                        if on_box && row < self.home.kits.len() {
                            return self.home.handle(b, Key::Char(' '));
                        }
                        if again {
                            let key = if row < self.home.kits.len() { Key::Right } else { Key::Enter };
                            return self.home_key(b, key);
                        }
                        None
                    }
                    ListId::Page => {
                        let again = self.page_sel() == row;
                        self.set_page_sel(row);
                        if on_box {
                            return self.dispatch(b, Key::Char(' '));
                        }
                        if again {
                            return self.dispatch(b, Key::Enter);
                        }
                        None
                    }
                    ListId::Library => {
                        let again = self.focus == Focus::Body && self.body_sel == row;
                        self.focus = Focus::Body;
                        self.body_sel = row;
                        if on_box {
                            return self.dispatch(b, Key::Char(' '));
                        }
                        if again {
                            return self.dispatch(b, Key::Enter);
                        }
                        None
                    }
                    // The picker's clicks are taken in its own mode.
                    ListId::Picker => None,
                }
            }
        }
    }

    /// Show kind `to` (an index into KINDS) in the library.
    fn set_kind(&mut self, b: &dyn Browser, for_kit: bool, to: usize) {
        if for_kit {
            self.tab = to;
        } else {
            self.sel = to;
        }
        self.library_landed(b);
    }

    // ── Pages (ADR-2610261643 §18) ───────────────────────────────────────────

    fn page(&self) -> Option<Page> {
        self.stack.last().map(|(p, _)| p.clone())
    }

    fn page_sel(&self) -> usize {
        self.stack.last().map(|(_, i)| *i).unwrap_or(0)
    }

    fn set_page_sel(&mut self, i: usize) {
        if let Some((_, sel)) = self.stack.last_mut() {
            *sel = i;
        }
    }

    fn push(&mut self, b: &dyn Browser, page: Page) {
        self.stack.push((page, 0));
        self.page_scroll.reset();
        self.refresh(b);
    }

    /// Back one page; from the library, the State's own rows return to rest.
    fn pop(&mut self, b: &dyn Browser) {
        self.stack.pop();
        self.page_scroll.reset();
        self.focus = Focus::List;
        self.refresh(b);
    }

    /// Open the library on `kind`: a picker for `kit`, or to manage it.
    fn open_library(&mut self, b: &dyn Browser, kit: Option<String>, kind: ItemKind) {
        let at = KINDS.iter().position(|k| *k == kind).unwrap_or(0);
        self.list_filter.clear();
        match &kit {
            Some(k) => {
                self.section = Section::Kits;
                self.sel = self.kits.iter().position(|x| x == k).unwrap_or(0);
                self.tab = at;
            }
            None => {
                self.section = Section::Library;
                self.sel = at;
                self.tab = 0;
            }
        }
        self.stack.push((Page::Library { kit }, 0));
        self.library_landed(b);
    }

    /// A library page (or kind) just shown: the cursor starts on the first pick,
    /// so what the kit holds is in view (§16).
    fn library_landed(&mut self, b: &dyn Browser) {
        self.focus = Focus::Body;
        self.row_changed(b);
        self.focus = Focus::Body;
        self.body_sel = self.first_ticked().unwrap_or(0);
    }

    /// The kind the library page shows.
    fn library_kind(&self) -> ItemKind {
        self.tab().kind().unwrap_or(ItemKind::Skill)
    }

    fn home_key(&mut self, b: &dyn Browser, key: Key) -> Option<Exit> {
        let on_kits = self.home.focus == Focus::Body;
        let profile = (!on_kits).then(|| self.home.profile().map(|p| p.name.clone())).flatten();
        let kit = on_kits.then(|| self.home.kit().map(String::from)).flatten();
        match key {
            // The rows past the lists are the workflow's actions (§19).
            Key::Enter if !on_kits && self.home.action().is_some() => match self.home.action() {
                Some(HomeAction::NewProfile) => self.input(Purpose::NewProfile { from: None }),
                Some(HomeAction::Import) => self.ask_import(b),
                Some(HomeAction::Library) | None => self.open_library(b, None, ItemKind::Skill),
            },
            Key::Enter if on_kits && kit.is_none() => self.input(Purpose::NewKit),
            Key::Char(' ' | 's') if self.home.action().is_some() && !on_kits => {}
            Key::Right | Key::Char('e') => match (profile, kit) {
                (Some(n), _) => self.push(b, Page::Profile(n)),
                (_, Some(k)) => self.push(b, Page::Kit(k)),
                _ => {}
            },
            Key::Left => {}
            Key::Char('n') if on_kits => self.input(Purpose::NewKit),
            Key::Char('n') => self.input(Purpose::NewProfile { from: None }),
            Key::Char('i') => self.ask_import(b),
            Key::Char('l') => self.open_library(b, None, ItemKind::Skill),
            k => {
                let exit = self.home.handle(b, k);
                self.message = self.home.message.take();
                return exit;
            }
        }
        None
    }

    fn profile_items(&self) -> Vec<ProfileItem> {
        let mut rows = vec![ProfileItem::Launch];
        rows.extend(self.kits.iter().map(|k| ProfileItem::Kit(k.clone())));
        rows.extend([ProfileItem::Variables, ProfileItem::Default, ProfileItem::Copy, ProfileItem::Shell]);
        rows
    }

    fn profile_key(&mut self, b: &dyn Browser, key: Key, n: Name) -> Option<Exit> {
        let rows = self.profile_items();
        let row = rows.get(self.page_sel()).cloned().unwrap_or(ProfileItem::Launch);
        let attached = |s: &State, k: &str| s.profiles.iter().any(|p| p.name == n && p.kits.iter().any(|x| x == k));
        match key {
            Key::Esc | Key::Left => self.pop(b),
            Key::Right => {
                if let ProfileItem::Kit(k) = row {
                    self.push(b, Page::Kit(k));
                }
            }
            Key::Char('q') => return Some(Exit::Quit),
            k if movement(k, 0).is_some() => self.move_page(k, rows.len()),
            Key::Char(' ') => {
                if let ProfileItem::Kit(k) = &row {
                    let on = !attached(self, k);
                    if let Err(e) = b.attach(k, &n, on) {
                        self.message = Some(e.to_string());
                    }
                    self.refresh(b);
                }
            }
            Key::Enter | Key::Char('e') => match row {
                ProfileItem::Launch if key == Key::Enter => return Some(Exit::Launch(n.as_str().into())),
                ProfileItem::Kit(k) => self.push(b, Page::Kit(k)),
                ProfileItem::Default if key == Key::Enter => {
                    self.message = Some(match b.set_default(&n) {
                        Ok(()) => format!("{} is now the default", n.as_str()),
                        Err(e) => e.to_string(),
                    });
                    self.refresh(b);
                }
                ProfileItem::Copy if key == Key::Enter => self.input(Purpose::NewProfile { from: Some(n) }),
                ProfileItem::Shell if key == Key::Enter => return Some(Exit::Shell(n.as_str().into())),
                _ => self.editing = Some(EditTarget::Profile(n)),
            },
            _ => {}
        }
        None
    }

    fn kit_items(&self, kit: &str) -> Vec<KitItem> {
        let uses = self.all_uses.iter().find(|(k, _)| k == kit).map(|(_, u)| u.clone()).unwrap_or_default();
        let mut rows: Vec<KitItem> = uses.into_iter().map(KitItem::Entry).collect();
        rows.extend([KitItem::Add, KitItem::Instructions]);
        rows.extend(self.profiles.iter().map(|p| KitItem::Profile(p.name.clone())));
        rows.extend([KitItem::Rename, KitItem::Delete]);
        rows
    }

    fn kit_key(&mut self, b: &dyn Browser, key: Key, kit: String) -> Option<Exit> {
        let rows = self.kit_items(&kit);
        let row = rows.get(self.page_sel()).cloned().unwrap_or(KitItem::Add);
        let switch = |s: &mut State, p: &Name| {
            let on = !s.profiles.iter().any(|x| x.name == *p && x.kits.contains(&kit));
            if let Err(e) = b.attach(&kit, p, on) {
                s.message = Some(e.to_string());
            }
            s.refresh(b);
        };
        match key {
            Key::Esc | Key::Left => self.pop(b),
            Key::Char('q') => return Some(Exit::Quit),
            k if movement(k, 0).is_some() => self.move_page(k, rows.len()),
            Key::Char(' ') => match &row {
                KitItem::Entry(e) => {
                    self.message = Some(match b.set_use(&kit, e, false) {
                        Ok(()) => format!("{kit} no longer picks {}; + Add from the library brings it back", e.line()),
                        Err(err) => err.to_string(),
                    });
                    self.refresh(b);
                    let len = self.kit_items(&kit).len();
                    self.set_page_sel(self.page_sel().min(len.saturating_sub(1)));
                }
                KitItem::Profile(p) => switch(self, p),
                _ => {}
            },
            Key::Enter | Key::Char('e') => match row {
                KitItem::Entry(e @ Entry::Item(..)) if key == Key::Enter => match b.read(&e) {
                    Ok(text) => self.mode = Mode::View { title: e.line(), text, scroll: 0 },
                    Err(err) => self.message = Some(err.to_string()),
                },
                KitItem::Entry(e) => self.open_library(b, Some(kit), e.kind()),
                KitItem::Add => self.open_library(b, Some(kit), ItemKind::Skill),
                KitItem::Instructions => self.editing = Some(EditTarget::Kit(kit)),
                KitItem::Profile(p) => switch(self, &p),
                KitItem::Rename => self.input(Purpose::RenameKit { old: kit }),
                KitItem::Delete => {
                    self.message = Some(format!("Delete kit {kit}? y/n"));
                    self.mode = Mode::Confirm(Pending::DeleteKit(kit));
                }
            },
            _ => {}
        }
        None
    }

    fn move_page(&mut self, k: Key, len: usize) {
        let delta = movement(k, self.page_scroll.page.get()).unwrap_or(0);
        self.page_scroll.follow(k, delta, len);
        self.set_page_sel(shifted(self.page_sel(), delta, len));
    }

    fn library_key(&mut self, b: &dyn Browser, key: Key, kit: Option<String>) -> Option<Exit> {
        match key {
            Key::Esc => self.pop(b),
            Key::Char('q') => return Some(Exit::Quit),
            Key::Left | Key::Right => {
                let at = KINDS.iter().position(|k| *k == self.library_kind()).unwrap_or(0);
                let to = if key == Key::Right { (at + 1).min(KINDS.len() - 1) } else { at.saturating_sub(1) };
                if to != at {
                    self.set_kind(b, kit.is_some(), to);
                }
            }
            Key::Tab | Key::Char('h' | 'l' | 'c' | 'd' | 't') => {}
            k => return self.library_normal(b, k),
        }
        None
    }

    /// The library page's keys (§9): the tree, its marks, picks and pickers.
    fn library_normal(&mut self, b: &dyn Browser, key: Key) -> Option<Exit> {
        match key {
            k if movement(k, 0).is_some() => {
                let (scroll, len) = match self.focus {
                    Focus::List => (&self.list_scroll, self.rows().len()),
                    Focus::Body => (&self.body_scroll, self.body_len()),
                };
                let delta = movement(k, scroll.page.get()).unwrap_or(0);
                scroll.follow(k, delta, len);
                self.move_by(b, delta);
            }
            Key::Enter => self.view(b),
            Key::Char('/') => self.mode = Mode::Search,
            Key::Char(' ') => self.toggle(b),
            Key::Char('v') => self.view(b),
            Key::Char('e') => self.ask_edit(),
            Key::Char('a') => self.ask_add(),
            Key::Char('n') => self.ask_new(),
            Key::Char('m') => self.ask_move(),
            Key::Char('x') => self.ask_delete(),
            Key::Char('+') => self.ask_kit(),
            Key::Char('u') => self.update_plugins(b),
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

    fn library_tab(&self) -> Option<ItemKind> {
        (matches!(self.row(), Row::Library(_)) && self.focus == Focus::Body).then(|| self.tab().kind()).flatten()
    }

    /// Space in the library: mark a row, or pick it for the kit.
    fn toggle(&mut self, b: &dyn Browser) {
        match (self.row(), self.tab()) {
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

    /// `e` in the library: the highlighted item's file.
    fn ask_edit(&mut self) {
        if let (Some(kind), Some(r)) = (self.tab().kind(), self.tree_row()) {
            if r.item.is_some() {
                self.editing = Some(EditTarget::Entry(r.entry(kind)));
            }
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

    /// `u` on plugins: `git pull` the highlighted plugin, or every plugin from the list.
    fn update_plugins(&mut self, b: &dyn Browser) {
        if self.row() != Row::Library(ItemKind::Plugin) {
            return;
        }
        let one = (self.focus == Focus::Body).then(|| self.tree_row().and_then(|r| r.item.map(|i| i.name))).flatten();
        match b.update_plugins(one.as_deref()) {
            Ok(lines) => self.mode = Mode::View { title: "update plugins".into(), text: lines.join("\n"), scroll: 0 },
            Err(e) => self.message = Some(e.to_string()),
        }
        self.refresh(b);
    }

    fn ask_add(&mut self) {
        if self.library_tab().is_some() {
            let folder = self.tree_row().map(|r| r.folder()).unwrap_or_default();
            self.input(Purpose::Add { folder });
        }
    }

    fn ask_new(&mut self) {
        match self.row() {
            Row::Library(_) if self.library_tab().is_some() => {
                let base = self.tree_row().map(|r| r.folder()).unwrap_or_default();
                self.input(Purpose::Mkdir { base });
            }
            _ => {}
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
            // A new kit or profile opens at once: filling it in is the next step (§18).
            Purpose::NewKit => {
                let made = b.new_kit(text);
                if made.is_ok() {
                    self.push(b, Page::Kit(text.into()));
                }
                made.map(|()| format!("made kit {text}; + Add from the library fills it"))
            }
            Purpose::NewProfile { from } => {
                let made = b.new_profile(text, from.as_ref());
                if made.is_ok() {
                    self.refresh(b);
                    if let Some(n) = self.profiles.iter().find(|p| p.name.as_str() == text).map(|p| p.name.clone()) {
                        // A copy takes the place of the page it was made from.
                        if from.is_some() && matches!(self.page(), Some(Page::Profile(_))) {
                            self.stack.pop();
                        }
                        self.push(b, Page::Profile(n));
                    }
                }
                made.map(|()| match &from {
                    Some(f) => format!("copied {} to {text}: variables, kits and settings, not the login", f.as_str()),
                    None => format!("made profile {text}; set its variables, then launch it to log in"),
                })
            }
            Purpose::RenameKit { old } => b.rename_kit(&old, text).map(|()| {
                for (page, _) in &mut self.stack {
                    match page {
                        Page::Kit(k) if *k == old => *k = text.into(),
                        Page::Library { kit: Some(k) } if *k == old => *k = text.into(),
                        _ => {}
                    }
                }
                format!("kit {old} is now {text}")
            }),
            Purpose::RenameFolder { kind, folder } => {
                b.rename_folder(kind, &folder, text).map(|()| format!("renamed {} to {text}; kits follow it", folder_label(kind, &folder)))
            }
        };
        self.message = Some(result.unwrap_or_else(|e| e.to_string()));
        self.refresh(b);
    }

    /// The footer's keys for the page that is open.
    fn hints(&self) -> String {
        match self.page() {
            None => {
                let h = &self.home;
                let other = if h.focus == Focus::List { "tab kits" } else { "tab profiles" };
                match (h.focus, h.action(), h.kit()) {
                    (Focus::List, Some(a), _) => format!("⏎ {} · ↑↓ choose · {other} · q quit", a.label()),
                    (Focus::Body, _, None) => format!("⏎ + New kit · ↑↓ choose · {other} · q quit"),
                    _ => match h.launch_line() {
                        // Step 3: what Enter starts, spelled as the command line would be (§17).
                        Some(l) => format!("3 · ⏎ start {l} · → open · space tick · {other} · s save · q quit"),
                        None => format!("3 · ⏎ start: make a profile first · {other} · q quit"),
                    },
                }
            }
            Some(Page::Profile(_)) => "⏎ choose · space adds or removes a kit · → open a kit · ← back · q quit".into(),
            Some(Page::Kit(_)) => "⏎ choose · space removes an entry or switches a profile · ← back · q quit".into(),
            Some(Page::Library { kit: Some(_) }) => "space pick · p picked only · ←→ skills, agents, plugins · / search · v view · e edit · esc back".into(),
            Some(Page::Library { kit: None }) if self.library_kind() == ItemKind::Plugin => {
                "←→ skills, agents, plugins · esc back · a add (owner/repo) · u update · + to kit · x delete · v view · / search".into()
            }
            Some(Page::Library { kit: None }) => {
                "←→ skills, agents, plugins · esc back · a add · space mark · m move · + to kit · n folder · r rename · x delete · e edit · v view · / search".into()
            }
        }
    }
}

/// What a kit holds, one line per entry with the items it gives, then
/// `CLAUDE.md` when it has instructions. Shown under the highlighted kit (ADR-2610261643 §16).
fn kit_detail(uses: &[Entry], library: &[LibraryItem], instructions: bool) -> Vec<String> {
    let width = uses.iter().map(|u| u.line().chars().count()).max().unwrap_or(0);
    let mut out: Vec<String> = uses
        .iter()
        .map(|u| match resolve_kit(std::slice::from_ref(u), library).items.len() {
            0 => format!("{:width$}  not in the library", u.line()),
            n => format!("{:width$}  {n}", u.line()),
        })
        .collect();
    if instructions {
        out.push("CLAUDE.md".into());
    }
    if out.is_empty() {
        out.push("nothing picked yet".into());
    }
    out
}

/// `rows` with `detail` opened under row `open`. The rows above it keep their
/// places, so `open` is still the selected row.
fn with_detail<'a>(mut rows: Vec<ListItem<'a>>, open: Option<usize>, detail: &[String]) -> Vec<ListItem<'a>> {
    if let Some(i) = open.filter(|i| *i < rows.len()) {
        let lines = detail.iter().map(|d| ListItem::new(format!("      {d}")).style(Style::default().fg(Color::DarkGray)));
        rows.splice(i + 1..i + 1, lines);
    }
    rows
}

/// Every library item, of every kind.
fn whole_library(b: &dyn Browser) -> Vec<LibraryItem> {
    KINDS.iter().flat_map(|k| b.library(*k).0).collect()
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

/// Below this width the list and the body stack, so neither is cut off.
const NARROW: u16 = 70;

/// Paint the whole screen from `state`: the breadcrumb, the page, the footer.
pub fn draw(frame: &mut Frame, state: &State) {
    let [top, main, footer] = Layout::vertical([Constraint::Length(1), Constraint::Min(3), Constraint::Length(1)]).areas(frame.area());
    state.hits.borrow_mut().clear();
    draw_crumb(frame, state, top);
    if !draw_overlay(frame, state, main) {
        match state.page() {
            None => draw_home(frame, main, state),
            Some(Page::Profile(n)) => draw_profile(frame, state, main, &n),
            Some(Page::Kit(k)) => draw_kit(frame, state, main, &k),
            Some(Page::Library { .. }) => draw_library(frame, state, main),
        }
    }

    let kind = state.library_kind();
    let text = match &state.mode {
        Mode::Input { purpose, text, .. } => match purpose {
            Purpose::Add { .. } if kind == ItemKind::Plugin => {
                format!("Add a plugin (owner/repo, a git URL, or a path): {text}▏  enter add · esc cancel")
            }
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
        Mode::Search => format!("/ {}▏  enter keep · esc clear", state.body_filter),
        _ => state.message.clone().unwrap_or_else(|| state.hints()),
    };
    // Home's step 3 is a button too: a click on it starts Claude.
    if let (Mode::Normal, None, Some(l)) = (&state.mode, state.page(), state.home.launch_line()) {
        if state.message.is_none() && text.starts_with("3 · ⏎ start") && state.home.profile().is_some() {
            let w = u16::try_from(format!("3 · ⏎ start {l}").chars().count()).unwrap_or(u16::MAX);
            state.hits.borrow_mut().push((Rect { width: w.min(footer.width), ..footer }, Hit::Start));
        }
    }
    frame.render_widget(Paragraph::new(text).style(Style::default().fg(Color::DarkGray)), footer);
}

/// `ccs › work › kit net › library` on the left; the current profile on the right (§17, §18).
fn draw_crumb(frame: &mut Frame, state: &State, area: Rect) {
    let mut path = vec!["ccs".to_string()];
    path.extend(state.stack.iter().map(|(p, _)| match p {
        Page::Profile(n) => n.as_str().to_string(),
        Page::Kit(k) => format!("kit {k}"),
        Page::Library { .. } => "library".into(),
    }));
    let now = match state.profiles.iter().find(|p| p.current) {
        Some(p) => format!("⬡ {}{} ", p.name.as_str(), p.kits.iter().map(|k| format!(" +{k}")).collect::<String>()),
        None => "no profile chosen ".into(),
    };
    let width = u16::try_from(now.chars().count()).unwrap_or(u16::MAX);
    let [left, right] = Layout::horizontal([Constraint::Min(1), Constraint::Length(width)]).areas(area);
    // Each part is a way back to its page.
    let mut x = left.x + 1;
    for (depth, part) in path.iter().enumerate() {
        let w = u16::try_from(part.chars().count()).unwrap_or(0);
        state.hits.borrow_mut().push((Rect { x, y: area.y, width: w, height: 1 }, Hit::Crumb(depth)));
        x = x.saturating_add(w + 3);
    }
    frame.render_widget(Paragraph::new(format!(" {}", path.join(" › "))).style(Style::default().add_modifier(Modifier::BOLD)), left);
    frame.render_widget(Paragraph::new(now).style(Style::default().fg(Color::Cyan)), right);
}

/// One line of a page: a row the cursor can be on, or a label it skips.
struct Line2<'a> {
    item: ListItem<'a>,
    row: Option<usize>,
    /// The row starts with a `[ ]` box a click can tick.
    boxed: bool,
}

fn row_line<'a>(row: usize, item: impl Into<ListItem<'a>>) -> Line2<'a> {
    Line2 { item: item.into(), row: Some(row), boxed: false }
}

fn box_line<'a>(row: usize, item: impl Into<ListItem<'a>>) -> Line2<'a> {
    Line2 { item: item.into(), row: Some(row), boxed: true }
}

fn label<'a>(text: impl Into<String>) -> Line2<'a> {
    Line2 { item: ListItem::new(text.into()).style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)), row: None, boxed: false }
}

fn detail_lines<'a>(detail: &[String]) -> impl Iterator<Item = Line2<'a>> + '_ {
    detail.iter().map(|d| Line2 { item: ListItem::new(format!("      {d}")).style(Style::default().fg(Color::DarkGray)), row: None, boxed: false })
}

/// A page's lines, with the cursor on row `sel`.
fn render_page(frame: &mut Frame, area: Rect, state: &State, lines: Vec<Line2>, sel: usize, scroll: &Scroll) {
    let at = lines.iter().position(|l| l.row == Some(sel));
    let map = lines.iter().map(|l| l.row.map(|r| (r, l.boxed))).collect();
    let items = lines.into_iter().map(|l| l.item).collect();
    let block = Block::default().borders(Borders::ALL).border_style(border(true));
    let indent = if at.is_some() { 2 } else { 0 };
    render_list(frame, area, block, items, at, scroll, Some("▶ "));
    state.hits.borrow_mut().push((inside(area), Hit::List { list: ListId::Page, top: scroll.offset.get(), indent, map }));
}

fn draw_profile(frame: &mut Frame, state: &State, area: Rect, n: &Name) {
    let me = state.profiles.iter().find(|p| p.name == *n);
    let kits = me.map(|p| p.kits.clone()).unwrap_or_default();
    let overview = state.page_overview.as_ref().and_then(|o| o.as_ref().ok());
    let sel = state.page_sel();
    let mut lines = Vec::new();
    for (row, item) in state.profile_items().into_iter().enumerate() {
        match &item {
            ProfileItem::Launch => {
                let launch = format!("⏎ Launch {}{}", n.as_str(), kits.iter().map(|k| format!(" +{k}")).collect::<String>());
                lines.push(row_line(row, Line::from(vec![Span::styled(launch, Style::default().add_modifier(Modifier::BOLD))])));
                lines.push(label(""));
                lines.push(label("Kits it starts with"));
                if state.kits.is_empty() {
                    lines.push(label("  no kits yet: on Home, tab to the kits and press n"));
                }
            }
            ProfileItem::Kit(k) => {
                lines.push(box_line(row, tick(kits.contains(k), k)));
                if row == sel {
                    let i = state.kits.iter().position(|x| x == k).unwrap_or(0);
                    lines.extend(detail_lines(state.kit_details.get(i).map(Vec::as_slice).unwrap_or_default()));
                }
            }
            ProfileItem::Variables => {
                lines.push(label(""));
                let vars = overview.map(|o| o.env.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" · ")).unwrap_or_default();
                let vars = if vars.is_empty() { "none yet".to_string() } else { vars };
                lines.push(row_line(row, format!("Variables  {vars}")));
            }
            ProfileItem::Default if me.is_some_and(|p| p.is_default) => lines.push(row_line(row, "It is the default")),
            ProfileItem::Default => lines.push(row_line(row, "Make it the default")),
            ProfileItem::Copy => lines.push(row_line(row, "Copy to a new profile")),
            ProfileItem::Shell => lines.push(row_line(row, "Open a shell in its folder")),
        }
    }
    // Below the rows: where it lives and what the doctor says.
    let mut info: Vec<Line> = Vec::new();
    if let Some(o) = overview {
        info.push(Line::styled(o.segment.clone(), Style::default().add_modifier(Modifier::BOLD)));
        info.push(Line::from(format!("Folder: {}", o.dir.display())));
        info.extend(o.findings.iter().map(|f| match f {
            Finding::Pass(m) => Line::styled(format!("✓ {m}"), Style::default().fg(Color::Green)),
            Finding::Warn(m) => Line::styled(format!("! {m}"), Style::default().fg(Color::Yellow)),
            Finding::Fail(m) => Line::styled(format!("✗ {m}"), Style::default().fg(Color::Red)),
        }));
    }
    let want = u16::try_from(info.len() + 2).unwrap_or(u16::MAX).min(area.height / 2);
    let [list, below] = Layout::vertical([Constraint::Min(3), Constraint::Length(want)]).areas(area);
    render_page(frame, list, state, lines, sel, &state.page_scroll);
    frame.render_widget(Paragraph::new(info).block(Block::default().borders(Borders::ALL).title(" doctor ")), below);
}

fn draw_kit(frame: &mut Frame, state: &State, area: Rect, kit: &str) {
    let sel = state.page_sel();
    let width = state.kit_items(kit).iter().filter_map(|r| if let KitItem::Entry(e) = r { Some(e.line().chars().count()) } else { None }).max().unwrap_or(0);
    let mut lines = vec![label("Holds")];
    let mut people = false;
    for (row, item) in state.kit_items(kit).into_iter().enumerate() {
        match item {
            KitItem::Entry(e) => {
                let line = match resolve_kit(std::slice::from_ref(&e), &state.library).items.len() {
                    0 => format!("{:width$}  not in the library", e.line()),
                    n => format!("{:width$}  {n}", e.line()),
                };
                lines.push(row_line(row, line));
            }
            KitItem::Add => {
                if row == 0 {
                    lines.push(label("  nothing yet"));
                }
                lines.push(row_line(row, Span::styled("+ Add from the library", Style::default().fg(Color::Green))));
            }
            KitItem::Instructions => {
                let first = state.page_text.as_deref().and_then(|t| t.lines().find(|l| !l.trim().is_empty())).unwrap_or("none yet; ⏎ writes them");
                lines.push(label(""));
                lines.push(row_line(row, format!("CLAUDE.md  {first}")));
            }
            KitItem::Profile(p) => {
                if !people {
                    lines.push(label(""));
                    lines.push(label("Used by"));
                    people = true;
                }
                let on = state.profiles.iter().any(|x| x.name == p && x.kits.iter().any(|k| k == kit));
                lines.push(box_line(row, tick(on, p.as_str())));
            }
            KitItem::Rename => {
                lines.push(label(""));
                lines.push(row_line(row, "Rename the kit"));
            }
            KitItem::Delete => lines.push(row_line(row, Span::styled("Delete the kit", Style::default().fg(Color::Red)))),
        }
    }
    render_page(frame, area, state, lines, sel, &state.page_scroll);
}

/// The library page: a line of kinds, then the tree.
fn draw_library(frame: &mut Frame, state: &State, area: Rect) {
    let [tabs_area, body] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
    let titles = vec![format!("Skills ({})", state.counts[0]), format!("Agents ({})", state.counts[1]), format!("Plugins ({})", state.counts[2])];
    let at = KINDS.iter().position(|k| *k == state.library_kind()).unwrap_or(0);
    // Tabs draw " title " then "│" between them; each title is a click target.
    let mut x = tabs_area.x;
    for (i, t) in titles.iter().enumerate() {
        let w = u16::try_from(t.chars().count()).unwrap_or(0) + 2;
        state.hits.borrow_mut().push((Rect { x, y: tabs_area.y, width: w, height: 1 }, Hit::Kind(i)));
        x = x.saturating_add(w + 1);
    }
    let tabs_widget = Tabs::new(titles).select(at).highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    frame.render_widget(tabs_widget, tabs_area);
    draw_body(frame, state, body);
}

/// The pager, a picker, or path completions, over whatever page is open.
fn draw_overlay(frame: &mut Frame, state: &State, area: Rect) -> bool {
    let block = Block::default().borders(Borders::ALL).border_style(border(true));
    if let Mode::View { title, text, scroll } = &state.mode {
        let visible = area.height.saturating_sub(2) as usize;
        state.view_scroll.page.set(visible);
        let lines = text.lines().count();
        let at = format!(" {}–{} of {lines} ", *scroll as usize + 1, (*scroll as usize + visible).min(lines));
        let block = block.title(format!(" {title} · esc back ")).title_bottom(Line::from(at).right_aligned());
        frame.render_widget(Paragraph::new(text.as_str()).block(block).scroll((*scroll, 0)), area);
        scrollbar(frame, area, lines, visible, *scroll as usize);
        return true;
    }
    if let Mode::Input { candidates, .. } = &state.mode {
        if !candidates.is_empty() {
            let rows: Vec<ListItem> = candidates.iter().map(|c| ListItem::new(c.as_str())).collect();
            frame.render_widget(List::new(rows).block(block.title(" Completions ")), area);
            return true;
        }
    }
    if let Mode::Pick { purpose, filter, sel } = &state.mode {
        let kind = state.library_kind();
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
        let map = (0..rows.len()).map(|r| Some((r, false))).collect();
        render_list(frame, area, block.title(title), rows, Some(*sel), &state.pick_scroll, None);
        state.hits.borrow_mut().push((inside(area), Hit::List { list: ListId::Picker, top: state.pick_scroll.offset.get(), indent: 0, map }));
        return true;
    }
    false
}

fn draw_body(frame: &mut Frame, state: &State, area: Rect) {
    let block = Block::default().borders(Borders::ALL).border_style(border(state.focus == Focus::Body));
    let selected = (state.focus == Focus::Body).then_some(state.body_sel);
    // Name what the body shows, so a tick or an edit plainly acts on it (§17).
    let block = match state.row() {
        Row::Kit(k) => block.title(format!(" kit {k}: space picks ")),
        Row::Library(kind) => block.title(format!(" library · {} ", kind.dir())),
    };
    let (row, tab) = (state.row(), state.tab());
    {
            let Some(kind) = tab.kind() else { return };
            let tree = state.visible_tree();
            if tree.is_empty() {
                // Say the step from where the reader stands: only the library to manage has `a`.
                let empty = if !state.body_filter.is_empty() {
                    format!("No {} matches '{}'.", kind.label(), state.body_filter)
                } else if matches!(row, Row::Kit(_)) {
                    format!("No {}s in the library yet. Add one from ◆ Library on the start screen.", kind.label())
                } else if kind == ItemKind::Plugin {
                    "No plugins yet: press a, then type owner/repo, for example google/elephant-goldfish-model.".to_string()
                } else {
                    format!("No {}s yet: press a, then type the path of a folder that holds them.", kind.label())
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
            // Every tree row starts with its mark: [ ] for a kit's picks, ● for the library's marks.
            let map = (0..rows.len()).map(|r| Some((r, true))).collect();
            render_list(frame, area, block, rows, selected, &state.body_scroll, None);
            state.hits.borrow_mut().push((inside(area), Hit::List { list: ListId::Library, top: state.body_scroll.offset.get(), indent: 0, map }));
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

// ── Home, the launch screen (ADR-2610261643 §10, §19) ───────────────────────

/// What the profile list offers below the profiles: the workflow, on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HomeAction {
    NewProfile,
    Import,
    Library,
}

const HOME_ACTIONS: [HomeAction; 3] = [HomeAction::NewProfile, HomeAction::Import, HomeAction::Library];

impl HomeAction {
    fn label(self) -> &'static str {
        match self {
            HomeAction::NewProfile => "+ New profile",
            HomeAction::Import => "⇣ Import (AWS profiles, ~/.claude)",
            HomeAction::Library => "◆ Library: skills, agents, plugins",
        }
    }
}

/// Home: step 1, a profile, on the left; step 2, its kits, on the right;
/// step 3, Enter. Each list ends with its actions.
struct Picker {
    profiles: Vec<ProfileRow>,
    /// Every kit, with how many items it picks.
    kits: Vec<(String, usize)>,
    /// What each kit holds, as `kit_detail` lines, in `kits` order.
    details: Vec<Vec<String>>,
    /// A profile, or past them, one of `HOME_ACTIONS`.
    sel: usize,
    /// A kit, or past them, "+ New kit".
    kit_sel: usize,
    focus: Focus,
    /// The kits to launch with, in order: the profile's own first.
    ticked: Vec<String>,
    /// The profile the ticks belong to: the last one the cursor was on.
    ticks_for: Option<Name>,
    /// That profile's saved kits when `ticked` was set from them.
    saved: Vec<String>,
    message: Option<String>,
    profile_scroll: Scroll,
    kit_scroll: Scroll,
}

impl Picker {
    /// `preselect` is the resolved profile, when there is one.
    fn new(b: &dyn Browser, preselect: Option<&str>) -> Picker {
        let trees = KINDS.map(|kind| {
            let (items, folders) = b.library(kind);
            (kind, library_tree(&items, &folders))
        });
        let library = whole_library(b);
        let (kits, details) = b
            .kits()
            .into_iter()
            .map(|k| {
                let uses = b.kit_uses(&k).unwrap_or_default();
                let n = trees.iter().map(|(kind, t)| t.iter().filter(|r| r.item.is_some() && pick(&uses, *kind, r) != Pick::No).count()).sum();
                let detail = kit_detail(&uses, &library, b.kit_instructions(&k).is_some());
                ((k, n), detail)
            })
            .unzip();
        let profiles = b.profiles();
        let sel = preselect.and_then(|p| profiles.iter().position(|r| r.name.as_str() == p)).unwrap_or(0);
        let mut picker = Picker {
            profiles,
            kits,
            details,
            sel,
            kit_sel: 0,
            focus: Focus::List,
            ticked: vec![],
            ticks_for: None,
            saved: vec![],
            message: None,
            profile_scroll: Scroll::default(),
            kit_scroll: Scroll::default(),
        };
        picker.tick_defaults();
        picker
    }

    /// The profile under the cursor; `None` on an action row.
    fn profile(&self) -> Option<&ProfileRow> {
        self.profiles.get(self.sel)
    }

    /// The action under the cursor, on the profile list.
    fn action(&self) -> Option<HomeAction> {
        self.sel.checked_sub(self.profiles.len()).and_then(|i| HOME_ACTIONS.get(i).copied())
    }

    /// The kit under the cursor; `None` on "+ New kit".
    fn kit(&self) -> Option<&str> {
        self.kits.get(self.kit_sel).map(|(k, _)| k.as_str())
    }

    /// Read everything again, on the same rows. The ticks follow their profile's
    /// saved kits only when those changed (on its page, say).
    fn reload(&mut self, b: &dyn Browser) {
        let (old_profiles, old_sel) = (self.profiles.len(), self.sel);
        let kit = self.kit().map(String::from);
        let fresh = Picker::new(b, self.ticks_for.as_ref().map(Name::as_str));
        let (ticked, saved) = (std::mem::take(&mut self.ticked), std::mem::take(&mut self.saved));
        let (focus, profile_scroll, kit_scroll) = (self.focus, std::mem::take(&mut self.profile_scroll), std::mem::take(&mut self.kit_scroll));
        *self = Picker { focus, profile_scroll, kit_scroll, ..fresh };
        if let Some(i) = old_sel.checked_sub(old_profiles) {
            self.sel = self.profiles.len() + i;
        }
        self.kit_sel = match kit {
            Some(k) => self.kits.iter().position(|(n, _)| *n == k).unwrap_or(0),
            None => self.kits.len(),
        };
        if saved == self.saved {
            self.ticked = ticked.into_iter().filter(|k| self.kits.iter().any(|(n, _)| n == k)).collect();
        }
    }

    /// Tick the selected profile's own kits, as long as they exist.
    fn tick_defaults(&mut self) {
        let Some((name, kits)) = self.profile().map(|p| (p.name.clone(), p.kits.clone())) else { return };
        self.ticks_for = Some(name);
        self.saved = kits;
        self.ticked = self.saved.iter().filter(|k| self.kits.iter().any(|(n, _)| n == *k)).cloned().collect();
        self.kit_scroll.reveal(self.kits.iter().position(|(k, _)| self.ticked.contains(k)));
    }

    /// Apply one key. `Some` ends the TUI.
    fn handle(&mut self, b: &dyn Browser, key: Key) -> Option<Exit> {
        self.message = None;
        match key {
            Key::Char('q') | Key::Esc => return Some(Exit::Quit),
            k if movement(k, 0).is_some() => match self.focus {
                Focus::List => {
                    let len = self.profiles.len() + HOME_ACTIONS.len();
                    let delta = movement(k, self.profile_scroll.page.get()).unwrap_or(0);
                    self.profile_scroll.follow(k, delta, len);
                    self.sel = shifted(self.sel, delta, len);
                    // An action row keeps the ticks; another profile brings its own.
                    if self.profile().is_some_and(|p| Some(&p.name) != self.ticks_for.as_ref()) {
                        self.tick_defaults();
                    }
                }
                Focus::Body => {
                    let len = self.kits.len() + 1;
                    let delta = movement(k, self.kit_scroll.page.get()).unwrap_or(0);
                    self.kit_scroll.follow(k, delta, len);
                    self.kit_sel = shifted(self.kit_sel, delta, len);
                }
            },
            Key::Tab => {
                self.focus = if self.focus == Focus::List { Focus::Body } else { Focus::List };
                let ticked = self.kits.iter().position(|(k, _)| self.ticked.contains(k));
                self.kit_sel = self.kit_scroll.in_view(ticked, self.kit_sel);
            }
            Key::Char(' ') if self.focus == Focus::Body => {
                if let Some(k) = self.kit().map(String::from) {
                    if !self.ticked.contains(&k) {
                        self.ticked.push(k);
                    } else {
                        self.ticked.retain(|t| *t != k);
                    }
                }
            }
            Key::Char('s') => {
                let p = self.profile().map(|p| p.name.clone())?;
                self.message = Some(match b.set_kits(&p, &self.ticked) {
                    Ok(()) if self.ticked.is_empty() => format!("{} now starts with no kits", p.as_str()),
                    Ok(()) => format!("{} now starts with {}", p.as_str(), self.ticked.iter().map(|k| format!("+{k}")).collect::<Vec<_>>().join(" ")),
                    Err(e) => e.to_string(),
                });
                self.profiles = b.profiles();
                self.saved = self.ticked.clone();
            }
            Key::Enter => {
                let p = self.ticks_for.clone()?;
                return Some(Exit::LaunchWith { profile: p.as_str().into(), kits: self.ticked.clone() });
            }
            _ => {}
        }
        None
    }

    /// What Enter starts, spelled as the command line would be (§17).
    fn launch_line(&self) -> Option<String> {
        let p = self.ticks_for.as_ref()?;
        Some(format!("{}{}", p.as_str(), self.ticked.iter().map(|k| format!(" +{k}")).collect::<String>()))
    }
}

/// Paint Home: the three steps, each list ending with its actions.
fn draw_home(frame: &mut Frame, main: Rect, state: &State) {
    let p = &state.home;
    let [left, right] = if main.width >= NARROW {
        Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).areas(main)
    } else {
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(main)
    };
    let opens = || Span::styled("  ›", Style::default().fg(Color::DarkGray));
    let action = |text: &str| ListItem::new(Span::styled(text.to_string(), Style::default().fg(Color::Green)));

    let block = Block::default().borders(Borders::ALL).title(" 1 · Choose a profile ").border_style(border(p.focus == Focus::List));
    let width = p.profiles.iter().map(|r| r.name.as_str().chars().count()).max().unwrap_or(0);
    let mut rows: Vec<ListItem> = p
        .profiles
        .iter()
        .map(|r| {
            // The › sits by the name, so a long provider never pushes it out of sight.
            ListItem::new(Line::from(vec![
                Span::raw(format!("{:width$}", r.name.as_str())),
                opens(),
                Span::raw("  "),
                Span::styled(r.provider.clone(), Style::default().fg(Color::Cyan)),
                Span::styled(if r.is_default { "  (default)" } else { "" }, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();
    rows.extend(HOME_ACTIONS.iter().map(|a| action(a.label())));
    let map = (0..rows.len()).map(|r| Some((r, false))).collect();
    render_list(frame, left, block, rows, Some(p.sel), &p.profile_scroll, Some("▶ "));
    state.hits.borrow_mut().push((inside(left), Hit::List { list: ListId::HomeProfiles, top: p.profile_scroll.offset.get(), indent: 2, map }));

    let title = match &p.ticks_for {
        Some(n) => format!(" 2 · Choose kits for {} ", n.as_str()),
        None => " 2 · Choose kits ".into(),
    };
    let block = Block::default().borders(Borders::ALL).title(title).border_style(border(p.focus == Focus::Body));
    let width = p.kits.iter().map(|(k, _)| k.chars().count()).max().unwrap_or(0);
    let mut rows: Vec<ListItem> = p
        .kits
        .iter()
        .map(|(k, n)| {
            let mut line = tick(p.ticked.contains(k), &format!("{k:width$}  "));
            line.spans.push(Span::styled(format!("{n:>3}"), Style::default().fg(Color::DarkGray)));
            line.spans.push(opens());
            ListItem::new(line)
        })
        .collect();
    rows.push(action("+ New kit"));
    let open = (p.focus == Focus::Body).then_some(p.kit_sel);
    let detail = open.and_then(|i| p.details.get(i)).map(Vec::as_slice).unwrap_or_default();
    // The detail lines under the open kit are not rows.
    let mut map: Vec<Option<(usize, bool)>> = (0..rows.len()).map(|r| Some((r, r < p.kits.len()))).collect();
    if let Some(i) = open.filter(|i| *i < map.len()) {
        map.splice(i + 1..i + 1, detail.iter().map(|_| None));
    }
    // Without the cursor in it, a list draws no "▶ " margin, so its boxes start at column 0.
    let indent = if open.is_some() { 2 } else { 0 };
    render_list(frame, right, block, with_detail(rows, open, detail), open, &p.kit_scroll, Some("▶ "));
    state.hits.borrow_mut().push((inside(right), Hit::List { list: ListId::HomeKits, top: p.kit_scroll.offset.get(), indent, map }));
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

/// Home and its pages (ADR-2610261643 §18), until the user quits or launches.
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
                    MouseEventKind::Down(MouseButton::Left) => Key::Click(m.column, m.row),
                    MouseEventKind::Down(MouseButton::Right) => Key::Back,
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
