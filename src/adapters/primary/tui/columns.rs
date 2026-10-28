//! The four columns (ADR-2610281305): what each one lists, and what the keys
//! do there. Every change goes through `Browser`, shows the `ccs` command that
//! makes the same change, and can be undone once with `z`.

use super::overlays::{EditTarget, Mode, Pending, Purpose};
use super::widgets::{movement, shifted, Hit, Scroll};
use super::{Exit, Key};
use crate::ports::{
    library_tree, pick_in, rank, Browser, Entry, Found, ItemKind, LibraryItem, LineGives, LoadPlan, Name, Pick, PluginContents, PluginPart, ProfileRow, TreeRow,
};
use ratatui::layout::Rect;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

/// Every kind of library entry, in the order column 4 shows them.
pub(super) const KINDS: [ItemKind; 3] = [ItemKind::Skill, ItemKind::Agent, ItemKind::Plugin];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Col {
    Profiles,
    Kits,
    Contents,
    Library,
}

impl Col {
    pub(super) const ALL: [Col; 4] = [Col::Profiles, Col::Kits, Col::Contents, Col::Library];

    pub(super) fn index(self) -> usize {
        self as usize
    }
}

/// A row of column 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ProfRow {
    Profile(usize),
    NewProfile,
    Import,
}

/// A row of column 2 you can stand on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum KitRow {
    /// `on`: the profile starts with it.
    Kit { name: String, on: bool },
    New,
}

/// A row of column 3 you can stand on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ContRow {
    Instructions,
    Line(usize),
}

/// A row of column 4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LibRow {
    Kind(ItemKind),
    Tree(ItemKind, TreeRow),
    /// A skill or agent inside a plugin: shown, never picked on its own.
    Inside { plugin: String, part: PluginPart, agent: bool },
}

impl LibRow {
    /// Which row this is, so the cursor can find it again after a change.
    pub(super) fn key(&self) -> String {
        match self {
            LibRow::Kind(k) => format!("kind:{}", k.dir()),
            LibRow::Tree(k, r) if r.item.is_none() => format!("{}/{}/", k.dir(), r.path),
            LibRow::Tree(k, r) => format!("{}/{}", k.dir(), r.path),
            LibRow::Inside { plugin, part, .. } => format!("plugins/{plugin}/{}", part.file),
        }
    }

    pub(super) fn entry(&self) -> Option<Entry> {
        match self {
            LibRow::Tree(k, r) => Some(r.entry(*k)),
            _ => None,
        }
    }

    /// The path a person reads: `skills/writing/humanize-writing`, `plugins/egm`.
    pub(super) fn path(&self) -> String {
        match self {
            LibRow::Kind(k) => format!("{}/", k.dir()),
            LibRow::Tree(k, r) if r.item.is_none() => format!("{}/{}/", k.dir(), r.path),
            LibRow::Tree(k, r) => format!("{}/{}", k.dir(), r.path),
            LibRow::Inside { plugin, part, .. } => format!("plugins/{plugin} › {}", part.file),
        }
    }
}

/// A change `z` can undo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Change {
    Use { kit: String, entry: Entry, on: bool },
    Attach { kit: String, profile: Name, on: bool },
}

pub struct State {
    pub(super) col: Col,
    pub(super) profiles: Vec<ProfileRow>,
    /// The profile columns 2-4 work on: the last profile the cursor stood on.
    pub(super) profile: usize,
    pub(super) psel: usize,
    pub(super) all_kits: Vec<String>,
    /// Items each kit gives, for column 2.
    pub(super) kit_sizes: BTreeMap<String, usize>,
    pub(super) ksel: usize,
    /// The kit open in columns 3 and 4.
    pub(super) kit: Option<String>,
    pub(super) csel: usize,
    pub(super) lsel: usize,
    /// Open rows of column 4, by `LibRow::key`.
    pub(super) open: BTreeSet<String>,
    pub(super) filter: String,
    /// What the profile loads, for column 3 while column 1 has the focus.
    pub(super) plan: Option<Result<LoadPlan, String>>,
    /// The shown kit's lines, each with what it gives.
    pub(super) lines: Vec<LineGives>,
    pub(super) instructions: bool,
    pub(super) trees: Vec<(ItemKind, Vec<LibraryItem>, Vec<TreeRow>)>,
    pub(super) contents: BTreeMap<String, PluginContents>,
    pub(super) undo: Option<Change>,
    /// The last change's `ccs` command, or what went wrong.
    pub(super) notice: Option<String>,
    pub(super) mode: Mode,
    pub(super) editing: Option<EditTarget>,
    pub(super) scroll: [Scroll; 4],
    pub(super) view_scroll: Scroll,
    pub(super) hits: RefCell<Vec<(Rect, Hit)>>,
}

impl State {
    /// Opens on the profile `ccs` would launch with no name (ADR-2610281305).
    pub fn new(b: &dyn Browser) -> State {
        let mut s = State {
            col: Col::Profiles,
            profiles: vec![],
            profile: 0,
            psel: 0,
            all_kits: vec![],
            kit_sizes: BTreeMap::new(),
            ksel: 0,
            kit: None,
            csel: 0,
            lsel: 0,
            open: KINDS.iter().map(|k| LibRow::Kind(*k).key()).collect(),
            filter: String::new(),
            plan: None,
            lines: vec![],
            instructions: false,
            trees: vec![],
            contents: BTreeMap::new(),
            undo: None,
            notice: None,
            mode: Mode::Normal,
            editing: None,
            scroll: Default::default(),
            view_scroll: Scroll::default(),
            hits: RefCell::new(vec![]),
        };
        s.refresh(b);
        s.profile = s.profiles.iter().position(|p| p.current).unwrap_or(0);
        s.psel = s.profile;
        s.reload_view(b);
        s
    }

    pub(super) fn profile_name(&self) -> Option<&Name> {
        self.profiles.get(self.profile).map(|p| &p.name)
    }

    pub(super) fn profile_label(&self) -> String {
        self.profile_name().map(|n| n.as_str().to_string()).unwrap_or_else(|| "no profile".into())
    }

    pub(super) fn prof_rows(&self) -> Vec<ProfRow> {
        (0..self.profiles.len()).map(ProfRow::Profile).chain([ProfRow::NewProfile, ProfRow::Import]).collect()
    }

    /// The profile's own kits, in launch order.
    pub(super) fn own_kits(&self) -> Vec<String> {
        self.profiles.get(self.profile).map(|p| p.kits.clone()).unwrap_or_default()
    }

    /// The profile's kits, "+ new kit", then every other kit (ADR-2610281305).
    pub(super) fn kit_rows(&self) -> Vec<KitRow> {
        let own = self.own_kits();
        let mut rows: Vec<KitRow> = own.iter().map(|k| KitRow::Kit { name: k.clone(), on: true }).collect();
        rows.push(KitRow::New);
        rows.extend(self.all_kits.iter().filter(|k| !own.contains(k)).map(|k| KitRow::Kit { name: k.clone(), on: false }));
        rows
    }

    /// The kit columns 3 and 4 show: the one under the cursor in column 2, else the open one.
    pub(super) fn shown_kit(&self) -> Option<String> {
        match self.col {
            Col::Profiles => None,
            Col::Kits => match self.kit_rows().get(self.ksel) {
                Some(KitRow::Kit { name, .. }) => Some(name.clone()),
                _ => None,
            },
            Col::Contents | Col::Library => self.kit.clone(),
        }
    }

    pub(super) fn cont_rows(&self) -> Vec<ContRow> {
        std::iter::once(ContRow::Instructions).chain((0..self.lines.len()).map(ContRow::Line)).collect()
    }

    fn uses(&self) -> Vec<Entry> {
        self.lines.iter().map(|g| g.line.clone()).collect()
    }

    fn items(&self, kind: ItemKind) -> &[LibraryItem] {
        self.trees.iter().find(|(k, ..)| *k == kind).map(|(_, items, _)| items.as_slice()).unwrap_or(&[])
    }

    /// How the open kit picks a row of column 4.
    pub(super) fn mark(&self, row: &LibRow) -> Pick {
        let uses = self.uses();
        match row {
            LibRow::Kind(_) => Pick::No,
            LibRow::Tree(k, r) => pick_in(&uses, *k, r, self.items(*k)),
            LibRow::Inside { plugin, .. } if uses.contains(&Entry::Item(ItemKind::Plugin, plugin.clone())) => Pick::Through(format!("plugins/{plugin}")),
            LibRow::Inside { .. } => Pick::No,
        }
    }

    /// Picked and total items under a folder row.
    pub(super) fn folder_count(&self, kind: ItemKind, path: &str) -> (usize, usize) {
        let uses = self.uses();
        let inside: Vec<&LibraryItem> = self.items(kind).iter().filter(|i| i.in_folder(path)).collect();
        let tree = |i: &LibraryItem| TreeRow { path: i.path(), depth: 0, item: Some(i.clone()) };
        let picked = inside.iter().filter(|i| pick_in(&uses, kind, &tree(i), &[]) != Pick::No).count();
        (picked, inside.len())
    }

    pub(super) fn is_open(&self, row: &LibRow) -> bool {
        self.open.contains(&row.key())
    }

    /// Whether Enter or → can open the row.
    pub(super) fn openable(&self, row: &LibRow) -> bool {
        match row {
            LibRow::Kind(_) => true,
            LibRow::Tree(ItemKind::Plugin, r) => r.item.as_ref().and_then(|i| self.contents.get(&i.name)).is_some_and(|c| !c.skills.is_empty() || !c.agents.is_empty()),
            LibRow::Tree(_, r) => r.item.is_none(),
            LibRow::Inside { .. } => false,
        }
    }

    /// Column 4's rows: the tree with what is open, or, while searching, every
    /// match in rank order (ADR-2610281305: a name match before a path match
    /// before a description match).
    pub(super) fn lib_rows(&self) -> Vec<LibRow> {
        if !self.filter.is_empty() {
            return self.found();
        }
        let mut rows = Vec::new();
        for (kind, _, tree) in &self.trees {
            rows.push(LibRow::Kind(*kind));
            if !self.open.contains(&LibRow::Kind(*kind).key()) {
                continue;
            }
            for r in tree {
                let parts: Vec<&str> = r.path.split('/').collect();
                let hidden = (1..parts.len()).any(|n| !self.open.contains(&format!("{}/{}/", kind.dir(), parts[..n].join("/"))));
                if hidden {
                    continue;
                }
                let row = LibRow::Tree(*kind, r.clone());
                let inner = (*kind == ItemKind::Plugin && self.is_open(&row)).then(|| r.item.as_ref().and_then(|i| self.contents.get(&i.name).map(|c| (i.name.clone(), c))));
                rows.push(row);
                if let Some(Some((plugin, c))) = inner {
                    rows.extend(c.skills.iter().map(|p| LibRow::Inside { plugin: plugin.clone(), part: p.clone(), agent: false }));
                    rows.extend(c.agents.iter().map(|p| LibRow::Inside { plugin: plugin.clone(), part: p.clone(), agent: true }));
                }
            }
        }
        rows
    }

    fn found(&self) -> Vec<LibRow> {
        let mut hits: Vec<(u32, LibRow)> = Vec::new();
        for (kind, _, tree) in &self.trees {
            for r in tree {
                let row = LibRow::Tree(*kind, r.clone());
                let (name, about, found) = match &r.item {
                    Some(i) => (i.name.clone(), i.description.clone().unwrap_or_default(), if *kind == ItemKind::Plugin { Found::Plugin } else { Found::Item }),
                    None => (r.leaf().to_string(), String::new(), Found::Folder),
                };
                if let Some(score) = rank(&self.filter, &name, &row.path(), &about, found) {
                    hits.push((score, row));
                }
            }
        }
        for (plugin, c) in &self.contents {
            for (part, agent) in c.skills.iter().map(|p| (p, false)).chain(c.agents.iter().map(|p| (p, true))) {
                let row = LibRow::Inside { plugin: plugin.clone(), part: part.clone(), agent };
                if let Some(score) = rank(&self.filter, &part.file, &row.path(), part.description.as_deref().unwrap_or(""), Found::InPlugin) {
                    hits.push((score, row));
                }
            }
        }
        hits.sort_by_key(|(score, _)| *score);
        hits.into_iter().map(|(_, r)| r).collect()
    }

    pub(super) fn lib_row(&self) -> Option<LibRow> {
        self.lib_rows().get(self.lsel).cloned()
    }

    /// Reload everything from `Browser`. No copy is kept that a later read could
    /// contradict: every change ends here.
    pub(super) fn refresh(&mut self, b: &dyn Browser) {
        let keep = self.lib_row().map(|r| r.key());
        // Profiles come back sorted, so a new one can shift every index: keep
        // the profile and the cursor row by name, and the rows under the
        // profiles by their place below the last profile.
        let held = self.profile_name().cloned();
        let on = self.profiles.get(self.psel).map(|p| p.name.clone());
        let below = self.psel.checked_sub(self.profiles.len());
        self.profiles = b.profiles();
        self.all_kits = b.kits();
        let at = |n: &Name| self.profiles.iter().position(|p| &p.name == n);
        self.profile = held.as_ref().and_then(at).unwrap_or(self.profile.min(self.profiles.len().saturating_sub(1)));
        self.psel = match (on, below) {
            (Some(n), _) => at(&n).unwrap_or(self.profile),
            (None, Some(d)) => self.profiles.len() + d,
            (None, None) => self.psel,
        };
        self.trees = KINDS
            .iter()
            .map(|k| {
                let (items, folders) = b.library(*k);
                let tree = library_tree(&items, &folders);
                (*k, items, tree)
            })
            .collect();
        let plugins: Vec<String> = self.items(ItemKind::Plugin).iter().map(|p| p.name.clone()).collect();
        self.contents = plugins.into_iter().filter_map(|p| b.plugin_contents(&p).ok().map(|c| (p, c))).collect();
        self.kit_sizes = self
            .all_kits
            .iter()
            .map(|k| {
                let mut seen: Vec<String> = Vec::new();
                for g in b.kit_lines(k).unwrap_or_default() {
                    for i in g.items {
                        let key = format!("{}/{}", i.kind.dir(), i.path());
                        if !seen.contains(&key) {
                            seen.push(key);
                        }
                    }
                }
                (k.clone(), seen.len())
            })
            .collect();
        if self.kit.as_ref().is_some_and(|k| !self.all_kits.contains(k)) {
            self.kit = None;
            if self.col > Col::Kits {
                self.col = Col::Kits;
            }
        }
        self.reload_view(b);
        if let Some(k) = keep {
            self.find_lib(&k);
        }
    }

    /// Reload what depends on the cursor: the profile's plan and the shown kit.
    pub(super) fn reload_view(&mut self, b: &dyn Browser) {
        self.psel = self.psel.min(self.prof_rows().len() - 1);
        self.ksel = self.ksel.min(self.kit_rows().len() - 1);
        self.plan = self.profile_name().map(|n| b.load_plan(n).map_err(|e| e.to_string()));
        match self.shown_kit() {
            Some(k) => {
                self.lines = b.kit_lines(&k).unwrap_or_default();
                self.instructions = b.kit_instructions(&k).is_some();
            }
            None => {
                self.lines = vec![];
                self.instructions = false;
            }
        }
        self.csel = self.csel.min(self.cont_rows().len() - 1);
        self.lsel = self.lsel.min(self.lib_rows().len().saturating_sub(1));
    }

    /// Put the cursor of column 4 on the row with `key`, opening what hides it.
    pub(super) fn find_lib(&mut self, key: &str) {
        if let Some(i) = self.lib_rows().iter().position(|r| r.key() == key) {
            self.lsel = i;
            return;
        }
        let rows: Vec<LibRow> = self.trees.iter().flat_map(|(k, _, t)| t.iter().map(|r| LibRow::Tree(*k, r.clone()))).collect();
        let Some(LibRow::Tree(kind, r)) = rows.into_iter().find(|r| r.key() == key) else {
            if let Some(plugin) = key.strip_prefix("plugins/").and_then(|p| p.split('/').next()) {
                self.open.insert(LibRow::Kind(ItemKind::Plugin).key());
                self.open.insert(format!("plugins/{plugin}"));
                if let Some(i) = self.lib_rows().iter().position(|r| r.key() == key) {
                    self.lsel = i;
                }
            }
            return;
        };
        self.open.insert(LibRow::Kind(kind).key());
        let parts: Vec<&str> = r.path.split('/').collect();
        for n in 1..parts.len() {
            self.open.insert(format!("{}/{}/", kind.dir(), parts[..n].join("/")));
        }
        if let Some(i) = self.lib_rows().iter().position(|row| row.key() == key) {
            self.lsel = i;
        }
    }

    fn find_kit(&mut self, kit: &str) {
        if let Some(i) = self.kit_rows().iter().position(|r| matches!(r, KitRow::Kit { name, .. } if name == kit)) {
            self.ksel = i;
        }
    }

    /// Apply one key. `Some` ends the TUI.
    pub fn handle(&mut self, b: &dyn Browser, key: Key) -> Option<Exit> {
        if let Key::Click(x, y) = key {
            return self.click(b, x, y);
        }
        if self.mode != Mode::Normal {
            return self.mode_key(b, key);
        }
        self.notice = None;
        match key {
            Key::Char('q') => return Some(Exit::Quit),
            Key::Char('z') => {
                self.undo(b);
                return None;
            }
            Key::Char('i') => {
                self.ask_import(b);
                return None;
            }
            Key::Esc | Key::Back => {
                if self.col == Col::Library && !self.filter.is_empty() {
                    self.clear_filter();
                } else if self.col == Col::Profiles {
                    return (key == Key::Esc).then_some(Exit::Quit);
                } else {
                    self.go(b, Col::ALL[self.col.index() - 1]);
                }
                return None;
            }
            _ => {}
        }
        match self.col {
            Col::Profiles => self.profiles_key(b, key),
            Col::Kits => self.kits_key(b, key),
            Col::Contents => self.contents_key(b, key),
            Col::Library => self.library_key(b, key),
        }
    }

    /// Move the focus to a column. Columns 3 and 4 need a kit.
    pub(super) fn go(&mut self, b: &dyn Browser, to: Col) {
        if to >= Col::Contents {
            match self.shown_kit() {
                Some(k) => self.kit = Some(k),
                None => return,
            }
        }
        if to == Col::Kits {
            if let Some(k) = self.kit.clone() {
                self.find_kit(&k);
            }
        }
        self.col = to;
        self.reload_view(b);
    }

    fn page(&self) -> usize {
        self.scroll[self.col.index()].page.get()
    }

    fn profiles_key(&mut self, b: &dyn Browser, key: Key) -> Option<Exit> {
        let rows = self.prof_rows();
        if let Some(d) = movement(key, self.page()) {
            self.psel = shifted(self.psel, d, rows.len());
            if let ProfRow::Profile(i) = rows[self.psel] {
                if i != self.profile {
                    self.profile = i;
                    self.ksel = 0;
                    self.kit = None;
                }
            }
            self.reload_view(b);
            return None;
        }
        let name = self.profile_name().cloned();
        match (&rows[self.psel], key, name) {
            (ProfRow::Profile(_), Key::Enter, Some(n)) => return Some(Exit::Launch(n.as_str().into())),
            (ProfRow::Profile(_), Key::Char('!'), Some(n)) => return Some(Exit::Shell(n.as_str().into())),
            (ProfRow::Profile(_), Key::Char('e'), Some(n)) => self.editing = Some(EditTarget::Profile(n)),
            (ProfRow::Profile(_), Key::Char('d'), Some(n)) => {
                self.notice = Some(match b.set_default(&n) {
                    Ok(()) => format!("✓ ccs default {}", n.as_str()),
                    Err(e) => e.to_string(),
                });
                self.refresh(b);
            }
            (ProfRow::Profile(_), Key::Char('c'), Some(n)) => self.input(Purpose::NewProfile { from: Some(n) }),
            (ProfRow::NewProfile, Key::Enter, _) | (_, Key::Char('n'), _) => self.input(Purpose::NewProfile { from: None }),
            (ProfRow::Import, Key::Enter, _) => self.ask_import(b),
            (_, Key::Right | Key::Tab, Some(_)) => self.go(b, Col::Kits),
            _ => {}
        }
        None
    }

    fn kits_key(&mut self, b: &dyn Browser, key: Key) -> Option<Exit> {
        let rows = self.kit_rows();
        if let Some(d) = movement(key, self.page()) {
            self.ksel = shifted(self.ksel, d, rows.len());
            self.reload_view(b);
            return None;
        }
        match (rows[self.ksel].clone(), key) {
            (_, Key::Left) => self.go(b, Col::Profiles),
            (_, Key::Char('n')) | (KitRow::New, Key::Enter | Key::Right | Key::Char(' ')) => self.input(Purpose::NewKit),
            (KitRow::Kit { name, on }, Key::Char(' ')) => self.attach(b, &name, !on),
            (KitRow::Kit { .. }, Key::Enter | Key::Right | Key::Tab) => {
                self.csel = 0;
                self.go(b, Col::Contents);
            }
            (KitRow::Kit { name, .. }, Key::Char('e')) => self.editing = Some(EditTarget::Kit(name)),
            (KitRow::Kit { name, .. }, Key::Char('r')) => self.input(Purpose::RenameKit { old: name }),
            (KitRow::Kit { name, .. }, Key::Char('x')) => {
                self.notice = Some(format!("Delete kit {name}? It goes to the trash. y/n"));
                self.mode = Mode::Confirm(Pending::DeleteKit(name));
            }
            _ => {}
        }
        None
    }

    fn contents_key(&mut self, b: &dyn Browser, key: Key) -> Option<Exit> {
        let rows = self.cont_rows();
        if let Some(d) = movement(key, self.page()) {
            self.csel = shifted(self.csel, d, rows.len());
            return None;
        }
        let kit = self.kit.clone()?;
        match (rows[self.csel], key) {
            (_, Key::Left) => self.go(b, Col::Kits),
            (_, Key::Enter | Key::Right | Key::Tab) => self.go(b, Col::Library),
            (_, Key::Char('/')) => {
                self.go(b, Col::Library);
                self.search();
            }
            (ContRow::Instructions, Key::Char('e')) => self.editing = Some(EditTarget::Kit(kit)),
            (ContRow::Instructions, Key::Char(' ')) => self.notice = Some("e opens the instructions in your editor".into()),
            (ContRow::Line(i), Key::Char(' ')) => {
                if let Some(entry) = self.lines.get(i).map(|g| g.line.clone()) {
                    self.use_entry(b, &kit, entry, false);
                }
            }
            _ => {}
        }
        None
    }

    fn library_key(&mut self, b: &dyn Browser, key: Key) -> Option<Exit> {
        let rows = self.lib_rows();
        if let Some(d) = movement(key, self.page()) {
            self.lsel = shifted(self.lsel, d, rows.len());
            return None;
        }
        let row = rows.get(self.lsel).cloned();
        match (row, key) {
            (_, Key::Char('/')) => self.search(),
            (Some(r), Key::Left) if self.filter.is_empty() && self.openable(&r) && self.is_open(&r) && !matches!(r, LibRow::Kind(_)) => {
                self.open.remove(&r.key());
            }
            (_, Key::Left) => self.go(b, Col::Contents),
            (Some(r), Key::Right) if self.openable(&r) => {
                if self.filter.is_empty() {
                    self.open.insert(r.key());
                } else {
                    self.clear_filter();
                    self.open.insert(r.key());
                }
            }
            (Some(r), Key::Enter) if self.openable(&r) => {
                if !self.filter.is_empty() {
                    self.clear_filter();
                    self.open.insert(r.key());
                } else if self.is_open(&r) {
                    self.open.remove(&r.key());
                } else {
                    self.open.insert(r.key());
                }
            }
            (Some(r), Key::Enter | Key::Char(' ')) => self.toggle(b, &r),
            (Some(r), Key::Char('v')) => self.view(b, &r),
            (Some(LibRow::Tree(k, r)), Key::Char('e')) if r.item.is_some() && k != ItemKind::Plugin => self.editing = Some(EditTarget::Entry(r.entry(k))),
            (row, Key::Char('a')) => self.input(Purpose::Add { folder: row.and_then(|r| folder_of(&r)).unwrap_or_default() }),
            (Some(r), Key::Char('n')) => {
                if let Some(kind) = kind_of(&r).filter(|k| *k != ItemKind::Plugin) {
                    self.input(Purpose::Mkdir { kind, base: folder_of(&r).unwrap_or_default() });
                }
            }
            (Some(LibRow::Tree(kind, r)), Key::Char('r')) if r.item.is_none() => self.input(Purpose::RenameFolder { kind, folder: r.path }),
            (Some(r), Key::Char('m')) => {
                if let (Some(entry), Some(kind)) = (r.entry(), kind_of(&r).filter(|k| *k != ItemKind::Plugin)) {
                    self.ask_move(entry, kind);
                }
            }
            (Some(r), Key::Char('x')) => {
                if let Some(entry) = r.entry() {
                    self.notice = Some(format!("Delete {} from the library? It goes to the trash. y/n", entry.line()));
                    self.mode = Mode::Confirm(Pending::DeleteEntry(entry));
                }
            }
            (row, Key::Char('u')) => {
                let one = match row {
                    Some(LibRow::Tree(ItemKind::Plugin, r)) => r.item.map(|i| i.name),
                    _ => None,
                };
                self.update_plugins(b, one.as_deref());
            }
            _ => {}
        }
        None
    }

    fn search(&mut self) {
        self.filter.clear();
        self.mode = Mode::Search;
    }

    /// Leave a search, and keep the cursor on the row it was on.
    pub(super) fn clear_filter(&mut self) {
        let keep = self.lib_row().map(|r| r.key());
        self.filter.clear();
        self.lsel = 0;
        if let Some(k) = keep {
            self.find_lib(&k);
        }
    }

    /// Space in column 4: pick or unpick the row for the open kit.
    fn toggle(&mut self, b: &dyn Browser, row: &LibRow) {
        let Some(kit) = self.kit.clone() else { return };
        match (row, self.mark(row)) {
            (LibRow::Kind(_), _) => self.notice = Some("Pick a folder or an item; enter opens this kind".into()),
            (LibRow::Inside { plugin, part, .. }, _) => self.notice = Some(format!("{} is inside the plugin {plugin}: pick {plugin} to load it", part.file)),
            (LibRow::Tree(..), Pick::Through(f)) => {
                self.notice = Some(format!("{} comes with {f}; drop that line in column 3 to change it", row.path()));
            }
            (LibRow::Tree(..), Pick::Direct) => {
                if let Some(e) = row.entry() {
                    self.use_entry(b, &kit, e, false);
                }
            }
            (LibRow::Tree(..), _) => {
                if let Some(e) = row.entry() {
                    self.use_entry(b, &kit, e, true);
                }
            }
        }
    }

    /// Pick or drop one entry, through the same use case as `ccs kit use|drop`.
    pub(super) fn use_entry(&mut self, b: &dyn Browser, kit: &str, entry: Entry, on: bool) {
        match b.set_use(kit, &entry, on) {
            Ok(()) => {
                let verb = if on { "use" } else { "drop" };
                self.notice = Some(format!("✓ ccs kit {verb} {kit} {}   z undoes", entry.line()));
                self.undo = Some(Change::Use { kit: kit.into(), entry, on });
            }
            Err(e) => self.notice = Some(e.to_string()),
        }
        self.refresh(b);
    }

    /// Put a kit on the profile, or take it off, as `ccs kit attach|detach` does.
    pub(super) fn attach(&mut self, b: &dyn Browser, kit: &str, on: bool) {
        let Some(p) = self.profile_name().cloned() else {
            self.notice = Some("Make a profile first: n in column 1".into());
            return;
        };
        match b.attach(kit, &p, on) {
            Ok(()) => {
                let verb = if on { "attach" } else { "detach" };
                self.notice = Some(format!("✓ ccs kit {verb} {kit} {}   z undoes", p.as_str()));
                self.undo = Some(Change::Attach { kit: kit.into(), profile: p, on });
            }
            Err(e) => self.notice = Some(e.to_string()),
        }
        self.refresh(b);
        self.find_kit(kit);
        self.reload_view(b);
    }

    fn undo(&mut self, b: &dyn Browser) {
        let Some(change) = self.undo.take() else {
            self.notice = Some("Nothing to undo".into());
            return;
        };
        let done = match &change {
            Change::Use { kit, entry, on } => b.set_use(kit, entry, !on).map(|()| format!("↶ ccs kit {} {kit} {}", if *on { "drop" } else { "use" }, entry.line())),
            Change::Attach { kit, profile, on } => {
                b.attach(kit, profile, !on).map(|()| format!("↶ ccs kit {} {kit} {}", if *on { "detach" } else { "attach" }, profile.as_str()))
            }
        };
        self.notice = Some(done.unwrap_or_else(|e| e.to_string()));
        self.refresh(b);
        if let Change::Attach { kit, .. } = change {
            self.find_kit(&kit);
            self.reload_view(b);
        }
    }

    /// `v`: a skill's or agent's own text; a plugin's contents; a folder's items.
    fn view(&mut self, b: &dyn Browser, row: &LibRow) {
        let text = match row {
            LibRow::Kind(_) => return,
            LibRow::Tree(kind, r) if r.item.is_none() => self
                .items(*kind)
                .iter()
                .filter(|i| i.in_folder(&r.path))
                .map(|i| format!("{:<32} {}", i.name, i.description.as_deref().unwrap_or("")))
                .collect::<Vec<_>>()
                .join("\n"),
            LibRow::Tree(..) => match row.entry().map(|e| b.read(&e)) {
                Some(Ok(t)) => t,
                Some(Err(e)) => {
                    self.notice = Some(e.to_string());
                    return;
                }
                None => return,
            },
            LibRow::Inside { plugin, part, .. } => format!(
                "{}\n\n{}\n\nPart of the plugin {plugin}; pick {plugin} to load it.",
                part.shown_name.as_deref().unwrap_or(&part.file),
                part.description.as_deref().unwrap_or("")
            ),
        };
        self.mode = Mode::View { title: row.path(), text, scroll: 0 };
    }

    fn update_plugins(&mut self, b: &dyn Browser, one: Option<&str>) {
        match b.update_plugins(one) {
            Ok(lines) => self.mode = Mode::View { title: "update plugins".into(), text: lines.join("\n"), scroll: 0 },
            Err(e) => self.notice = Some(e.to_string()),
        }
        self.refresh(b);
    }

    /// A click selects a row; a click on the selected row, or on a box, acts on it.
    fn click(&mut self, b: &dyn Browser, x: u16, y: u16) -> Option<Exit> {
        let hit = self.hits.borrow().iter().rev().find(|(r, _)| r.contains((x, y).into())).cloned();
        let (rect, hit) = hit?;
        match hit {
            Hit::Step(i) => {
                if self.mode == Mode::Normal {
                    self.go(b, Col::ALL[i]);
                }
                None
            }
            Hit::Choice(i) => {
                if let Mode::Import { sel, .. } = &mut self.mode {
                    *sel = i;
                }
                self.mode_key(b, Key::Enter)
            }
            Hit::Rows { col, top, map } => {
                if self.mode != Mode::Normal {
                    return None;
                }
                let line = (y - rect.y) as usize;
                let Some(Some(sel)) = map.get(top + line).cloned() else { return None };
                let on_box = (x - rect.x) < 4;
                let target = Col::ALL[col];
                let again = self.col == target && self.cursor(target) == sel;
                if self.col != target {
                    // Columns 3 and 4 act on the kit they show, as `go` does.
                    if target >= Col::Contents {
                        self.kit = Some(self.shown_kit()?);
                    }
                    self.col = target;
                }
                self.set_cursor(b, target, sel);
                if on_box && matches!(target, Col::Kits | Col::Library) {
                    return self.handle(b, Key::Char(' '));
                }
                if again {
                    return self.handle(b, Key::Enter);
                }
                None
            }
        }
    }

    pub(super) fn cursor(&self, col: Col) -> usize {
        match col {
            Col::Profiles => self.psel,
            Col::Kits => self.ksel,
            Col::Contents => self.csel,
            Col::Library => self.lsel,
        }
    }

    fn set_cursor(&mut self, b: &dyn Browser, col: Col, sel: usize) {
        match col {
            Col::Profiles => {
                self.psel = sel;
                if let Some(ProfRow::Profile(i)) = self.prof_rows().get(sel).cloned() {
                    if i != self.profile {
                        self.profile = i;
                        self.ksel = 0;
                        self.kit = None;
                    }
                }
            }
            Col::Kits => self.ksel = sel,
            Col::Contents => self.csel = sel,
            Col::Library => self.lsel = sel,
        }
        self.reload_view(b);
    }
}

fn kind_of(row: &LibRow) -> Option<ItemKind> {
    match row {
        LibRow::Kind(k) | LibRow::Tree(k, _) => Some(*k),
        LibRow::Inside { .. } => None,
    }
}

/// The folder something added at this row goes into.
fn folder_of(row: &LibRow) -> Option<String> {
    match row {
        LibRow::Kind(_) => Some(String::new()),
        LibRow::Tree(_, r) => Some(r.folder()),
        LibRow::Inside { .. } => None,
    }
}
