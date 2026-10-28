//! What sits on top of the columns for a moment: the search, a line of typed
//! text, a y/n question, the `v` reader, the import list and the folder list.

use super::columns::{Col, State};
use super::widgets::{movement, shifted};
use super::{Exit, Key};
use crate::ports::{AwsImport, Browser, Entry, Error, ItemKind, Name};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Mode {
    Normal,
    /// Typing narrows column 4.
    Search,
    /// A y/n question; the notice line asks it.
    Confirm(Pending),
    View { title: String, text: String, scroll: u16 },
    /// Text being typed, and the path completions last offered.
    Input { purpose: Purpose, text: String, candidates: Vec<String> },
    Import { items: Vec<ImportItem>, sel: usize },
    /// The folders `entry` can move into; typing narrows them or names a new one.
    MoveTo { entry: Entry, kind: ItemKind, filter: String, sel: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Pending {
    DeleteKit(String),
    DeleteEntry(Entry),
}

/// What a line of typed text is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Purpose {
    /// A kit, made and put on the profile at once (ADR-2610281305).
    NewKit,
    /// `ccs new`, or `ccs new --from` with `from`.
    NewProfile { from: Option<Name> },
    RenameKit { old: String },
    /// A path or git source to copy into the library, into `folder`.
    Add { folder: String },
    Mkdir { kind: ItemKind, base: String },
    RenameFolder { kind: ItemKind, folder: String },
}

/// What the run loop hands to the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum EditTarget {
    Entry(Entry),
    Kit(String),
    Profile(Name),
}

/// A line of the import list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ImportItem {
    Migrate,
    Claude,
    Aws { aws: String, imported_as: Option<String> },
    AllAws,
}

impl ImportItem {
    pub(super) fn label(&self) -> String {
        match self {
            ImportItem::Migrate => "migrate what an earlier ccs left (shared/, native)".into(),
            ImportItem::Claude => "~/.claude: its skills, agents and CLAUDE.md, into the library and kit imported".into(),
            ImportItem::Aws { aws, imported_as: None } => format!("aws {aws}: a Bedrock profile"),
            ImportItem::Aws { aws, imported_as: Some(p) } => format!("aws {aws} → {p}, imported"),
            ImportItem::AllAws => "every AWS profile not imported yet".into(),
        }
    }
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

impl State {
    pub(super) fn input(&mut self, purpose: Purpose) {
        let text = match &purpose {
            Purpose::RenameKit { old } => old.clone(),
            Purpose::RenameFolder { folder, .. } => folder.rsplit('/').next().unwrap_or(folder).to_string(),
            _ => String::new(),
        };
        self.mode = Mode::Input { purpose, text, candidates: vec![] };
    }

    /// What the box asks, naming the profile or kit it acts on.
    pub(super) fn input_label(&self, purpose: &Purpose) -> String {
        match purpose {
            Purpose::NewKit => format!("New kit for {}", self.profile_label()),
            Purpose::NewProfile { from: None } => "New profile".into(),
            Purpose::NewProfile { from: Some(f) } => format!("Copy {} to", f.as_str()),
            Purpose::RenameKit { old } => format!("Rename kit {old} to"),
            Purpose::Add { folder } if folder.is_empty() => "Add to the library: a folder, an .md file, or owner/repo".into(),
            Purpose::Add { folder } => format!("Add to the library, into {folder}/: a folder, an .md file, or owner/repo"),
            Purpose::Mkdir { kind, base } if base.is_empty() => format!("New folder in {}/", kind.dir()),
            Purpose::Mkdir { kind, base } => format!("New folder in {}/{base}/", kind.dir()),
            Purpose::RenameFolder { kind, folder } => format!("Rename folder {}/{folder}/ to", kind.dir()),
        }
    }

    /// Keys while something sits on top of the columns.
    pub(super) fn mode_key(&mut self, b: &dyn Browser, key: Key) -> Option<Exit> {
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Normal => {}
            Mode::Search => match key {
                Key::Enter => {}
                Key::Esc | Key::Back => self.clear_filter(),
                Key::Backspace => {
                    self.filter.pop();
                    self.lsel = 0;
                    self.mode = Mode::Search;
                }
                Key::Char(c) => {
                    self.filter.push(c);
                    self.lsel = 0;
                    self.mode = Mode::Search;
                }
                _ => {
                    if let Some(d) = movement(key, self.scroll[3].page.get()) {
                        self.lsel = shifted(self.lsel, d, self.lib_rows().len());
                    }
                    self.mode = Mode::Search;
                }
            },
            Mode::Confirm(pending) => match key {
                Key::Char('y') => self.confirmed(b, pending),
                _ => {
                    self.notice = Some(match pending {
                        Pending::DeleteKit(k) => format!("kept kit {k}"),
                        Pending::DeleteEntry(e) => format!("kept {}", e.line()),
                    })
                }
            },
            Mode::View { title, text, scroll } => {
                if !matches!(key, Key::Esc | Key::Char('q') | Key::Back | Key::Char('v')) {
                    let page = self.view_scroll.page.get();
                    let last = text.lines().count().saturating_sub(page);
                    let scroll = match movement(key, page) {
                        Some(d) => u16::try_from(shifted(scroll as usize, d, last + 1)).unwrap_or(u16::MAX),
                        None => scroll,
                    };
                    self.mode = Mode::View { title, text, scroll };
                }
            }
            Mode::Input { purpose, mut text, candidates } => match key {
                Key::Esc | Key::Back => {}
                Key::Enter => {
                    let t = text.trim().to_string();
                    if !t.is_empty() {
                        self.submit(b, purpose, &t);
                    }
                }
                Key::Tab if matches!(purpose, Purpose::Add { .. }) => {
                    let found = b.complete_path(&text);
                    let prefix = common_prefix(&found);
                    if prefix.len() > text.len() {
                        text = prefix;
                    }
                    let candidates = if found.len() > 1 { found } else { vec![] };
                    self.mode = Mode::Input { purpose, text, candidates };
                }
                Key::Backspace => {
                    text.pop();
                    self.mode = Mode::Input { purpose, text, candidates };
                }
                Key::Char(c) => {
                    text.push(c);
                    self.mode = Mode::Input { purpose, text, candidates };
                }
                _ => self.mode = Mode::Input { purpose, text, candidates },
            },
            Mode::Import { items, sel } => match key {
                Key::Esc | Key::Back | Key::Char('q') => {}
                Key::Enter => {
                    if let Some(item) = items.get(sel).cloned() {
                        self.import(b, item);
                    }
                }
                _ => {
                    let sel = movement(key, items.len()).map_or(sel, |d| shifted(sel, d, items.len()));
                    self.mode = Mode::Import { items, sel };
                }
            },
            Mode::MoveTo { entry, kind, mut filter, sel } => {
                let choices = self.move_choices(&entry, kind, &filter);
                match key {
                    Key::Esc | Key::Back => {}
                    Key::Enter => {
                        if let Some(to) = choices.get(sel).cloned() {
                            let result = b.move_entry(&entry, &to).map(|()| {
                                let dest = if to.is_empty() { "/".to_string() } else { to.clone() };
                                format!("✓ ccs library mv {} {dest}", entry.line())
                            });
                            self.notice = Some(result.unwrap_or_else(|e| e.to_string()));
                            self.refresh(b);
                        }
                    }
                    Key::Backspace => {
                        filter.pop();
                        self.mode = Mode::MoveTo { entry, kind, filter, sel: 0 };
                    }
                    Key::Char(c) if !matches!(c, 'j' | 'k') || !filter.is_empty() => {
                        filter.push(c);
                        self.mode = Mode::MoveTo { entry, kind, filter, sel: 0 };
                    }
                    _ => {
                        let sel = movement(key, choices.len()).map_or(sel, |d| shifted(sel, d, choices.len()));
                        self.mode = Mode::MoveTo { entry, kind, filter, sel };
                    }
                }
            }
        }
        None
    }

    /// The folders an entry can move into: the matches first, then a new one from the text.
    pub(super) fn move_choices(&self, entry: &Entry, kind: ItemKind, filter: &str) -> Vec<String> {
        let wanted = filter.trim().trim_matches('/');
        let moving = match entry {
            Entry::Folder(_, f) => Some(f.clone()),
            Entry::Item(..) => None,
        };
        let folders = self.trees.iter().filter(|(k, ..)| *k == kind).flat_map(|(_, _, t)| t.iter().filter(|r| r.item.is_none()).map(|r| r.path.clone()));
        let mut out: Vec<String> = std::iter::once(String::new())
            .chain(folders)
            .filter(|f| moving.as_ref().is_none_or(|m| f != m && !f.starts_with(&format!("{m}/"))))
            .filter(|f| wanted.is_empty() || f.to_lowercase().contains(&wanted.to_lowercase()))
            .collect();
        if !wanted.is_empty() && !out.iter().any(|f| f == wanted) {
            out.push(wanted.to_string());
        }
        out
    }

    pub(super) fn ask_move(&mut self, entry: Entry, kind: ItemKind) {
        self.mode = Mode::MoveTo { entry, kind, filter: String::new(), sel: 0 };
    }

    fn confirmed(&mut self, b: &dyn Browser, pending: Pending) {
        self.notice = Some(match pending {
            Pending::DeleteKit(k) => match b.remove_kit(&k) {
                Ok(_) => {
                    self.undo = None;
                    format!("✓ ccs kit rm {k}")
                }
                Err(e) => e.to_string(),
            },
            Pending::DeleteEntry(e) => match b.remove(&e) {
                Ok(r) if r.kits.is_empty() => format!("✓ ccs library rm {}", e.line()),
                Ok(r) => format!("✓ ccs library rm {} · kits that picked it: {}", e.line(), r.kits.join(" ")),
                Err(err) => err.to_string(),
            },
        });
        self.refresh(b);
    }

    fn submit(&mut self, b: &dyn Browser, purpose: Purpose, text: &str) {
        let notice = match purpose {
            Purpose::NewKit => {
                let Some(p) = self.profile_name().cloned() else {
                    self.notice = Some("Make a profile first: n in column 1".into());
                    return;
                };
                if let Err(e) = b.new_kit(text) {
                    self.notice = Some(e.to_string());
                    return;
                }
                // Two writes: the kit is on disk now, so column 2 must show it even if attach fails.
                match b.attach(text, &p, true) {
                    Ok(()) => {
                        // Filling it in is the next step: open it in the library.
                        self.undo = Some(super::columns::Change::Attach { kit: text.into(), profile: p.clone(), on: true });
                        self.refresh(b);
                        self.kit = Some(text.into());
                        self.col = Col::Library;
                        self.filter.clear();
                        self.lsel = 0;
                        self.ksel = self.kit_rows().iter().position(|r| matches!(r, super::columns::KitRow::Kit { name, .. } if name == text)).unwrap_or(0);
                        self.reload_view(b);
                        format!("✓ ccs kit new {text} · ccs kit attach {text} {}   z takes it off {}", p.as_str(), p.as_str())
                    }
                    Err(e) => {
                        self.refresh(b);
                        self.ksel = self.kit_rows().iter().position(|r| matches!(r, super::columns::KitRow::Kit { name, .. } if name == text)).unwrap_or(self.ksel);
                        format!("kit {text} is made (ccs kit new {text}), but putting it on {} failed: {e}", p.as_str())
                    }
                }
            }
            Purpose::NewProfile { from } => match b.new_profile(text, from.as_ref()) {
                Ok(()) => {
                    self.refresh(b);
                    if let Some(i) = self.profiles.iter().position(|p| p.name.as_str() == text) {
                        self.profile = i;
                        self.psel = i;
                        self.col = Col::Profiles;
                        self.reload_view(b);
                    }
                    match from {
                        Some(f) => format!("✓ ccs new {text} --from {}", f.as_str()),
                        None => format!("✓ ccs new {text}"),
                    }
                }
                // `make_profile` may fail after the profile is on disk (the status line): show what exists.
                Err(e) => {
                    self.refresh(b);
                    e.to_string()
                }
            },
            Purpose::RenameKit { old } => match b.rename_kit(&old, text) {
                Ok(()) => {
                    if self.kit.as_deref() == Some(old.as_str()) {
                        self.kit = Some(text.into());
                    }
                    self.undo = None;
                    self.refresh(b);
                    self.ksel = self.kit_rows().iter().position(|r| matches!(r, super::columns::KitRow::Kit { name, .. } if name == text)).unwrap_or(self.ksel);
                    self.reload_view(b);
                    format!("✓ ccs kit rename {old} {text}")
                }
                Err(e) => e.to_string(),
            },
            Purpose::Add { folder } => match b.add(text, &folder) {
                Ok((k, n)) => {
                    self.refresh(b);
                    let into = if folder.is_empty() { String::new() } else { format!(" --in {folder}") };
                    format!("✓ ccs library add {text}{into} · {}/{n} is in the library", k.dir())
                }
                Err(e) => e.to_string(),
            },
            Purpose::Mkdir { kind, base } => {
                let name = text.trim_matches('/');
                let folder = if base.is_empty() { name.to_string() } else { format!("{base}/{name}") };
                match b.mkdir(kind, &folder) {
                    Ok(()) => {
                        self.refresh(b);
                        format!("✓ ccs library mkdir {}/{folder}/", kind.dir())
                    }
                    Err(e) => e.to_string(),
                }
            }
            Purpose::RenameFolder { kind, folder } => match b.rename_folder(kind, &folder, text) {
                Ok(()) => {
                    self.refresh(b);
                    format!("✓ ccs library rename {}/{folder}/ {text} · kits follow it", kind.dir())
                }
                Err(e) => e.to_string(),
            },
        };
        self.notice = Some(notice);
    }

    /// `i`: what `ccs migrate`, `ccs import` and `ccs import aws` can do here.
    pub(super) fn ask_import(&mut self, b: &dyn Browser) {
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
        self.mode = Mode::Import { items, sel: 0 };
    }

    /// Run one import, then show every note it made.
    fn import(&mut self, b: &dyn Browser, item: ImportItem) {
        let outcome = |r: Result<Vec<AwsImport>, Error>| r.map(|o| o.iter().map(ToString::to_string).collect::<Vec<_>>());
        let (title, result) = match item {
            ImportItem::Migrate => ("ccs migrate", b.migrate().map(|n| if n.is_empty() { vec!["Nothing to migrate.".into()] } else { n })),
            ImportItem::Claude => (
                "ccs import",
                b.import_claude().map(|mut n| {
                    n.push("~/.claude is in the library and in kit imported; nothing in ~/.claude changed.".into());
                    n
                }),
            ),
            ImportItem::Aws { aws, .. } => ("ccs import aws", outcome(b.import_aws(&[aws], false))),
            ImportItem::AllAws => ("ccs import aws --all", outcome(b.import_aws(&[], true))),
        };
        match result {
            Ok(lines) => self.mode = Mode::View { title: title.into(), text: lines.join("\n"), scroll: 0 },
            Err(e) => self.notice = Some(e.to_string()),
        }
        self.refresh(b);
    }

    /// Whether the run loop must hand the terminal to the editor now.
    pub fn wants_editor(&self) -> bool {
        self.editing.is_some()
    }

    /// Run the queued edit. The caller has given the terminal to the editor.
    pub fn run_editor(&mut self, b: &dyn Browser) {
        let Some(target) = self.editing.take() else { return };
        self.notice = Some(match target {
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
}
