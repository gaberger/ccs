//! Painting: the step line, the "also on" line, the columns, the description
//! strip, the notice, the key line, and whatever sits on top for a moment.

use super::columns::{Col, ContRow, KitRow, LibRow, ProfRow, State};
use super::overlays::{Mode, Purpose};
use super::widgets::{inside, render_list, Hit};
use crate::ports::{Entry, ItemKind, LineGives, LoadPlan, Pick};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn bold() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

/// A count as screen cells; a count too large for the screen is cut to the screen anyway.
fn cells(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

pub fn draw(frame: &mut Frame, state: &State) {
    state.hits.borrow_mut().clear();
    let area = frame.area();
    let strip = if state.col == Col::Library { 2 } else { 0 };
    let [top, also, body, strip_area, notice, keys] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(3), Constraint::Length(strip), Constraint::Length(1), Constraint::Length(1)])
            .areas(area);
    steps(frame, state, top);
    frame.render_widget(Paragraph::new(also_line(state)).style(Style::default().fg(Color::Yellow)), also);
    columns(frame, state, body);
    if strip > 0 {
        frame.render_widget(Paragraph::new(strip_text(state)).wrap(Wrap { trim: true }).style(Style::default().fg(Color::Gray)), strip_area);
    }
    let text = state.notice.clone().unwrap_or_default();
    let style = if text.starts_with('✓') { Style::default().fg(Color::Green) } else { Style::default().fg(Color::Yellow) };
    frame.render_widget(Paragraph::new(text).style(style), notice);
    frame.render_widget(Paragraph::new(hints(state)).style(Style::default().fg(Color::Cyan)), keys);
    overlay(frame, state, body);
}

/// `ccs  1 work › 2 research › 3 in research › 4 add from library`: where you
/// are, each step a click target.
fn steps(frame: &mut Frame, state: &State, area: Rect) {
    let kit = state.shown_kit();
    let labels = [
        format!("1 {}", state.profile_label()),
        format!("2 {}", kit.as_deref().unwrap_or("its kits")),
        kit.as_ref().map_or_else(|| "3 what it loads".to_string(), |k| format!("3 in {k}")),
        "4 add from library".to_string(),
    ];
    let mut spans = vec![Span::styled(" ccs  ", bold())];
    let mut x = area.x + 6;
    for (i, label) in labels.iter().enumerate() {
        let style = match i.cmp(&state.col.index()) {
            std::cmp::Ordering::Equal => Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD),
            std::cmp::Ordering::Less => Style::default(),
            std::cmp::Ordering::Greater => dim(),
        };
        let w = cells(label.chars().count());
        let width = w.min(area.right().saturating_sub(x));
        state.hits.borrow_mut().push((Rect { x, y: area.y, width, height: 1 }, Hit::Step(i)));
        spans.push(Span::styled(label.clone(), style));
        x = x.saturating_add(w);
        if i < 3 {
            spans.push(Span::styled(" › ", dim()));
            x = x.saturating_add(3);
        }
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// A kit on more than one profile: a change to it changes them all.
fn also_line(state: &State) -> String {
    let Some(kit) = state.shown_kit() else { return String::new() };
    let others: Vec<&str> =
        state.profiles.iter().enumerate().filter(|(i, p)| *i != state.profile && p.kits.contains(&kit)).map(|(_, p)| p.name.as_str()).collect();
    if others.is_empty() {
        String::new()
    } else {
        format!(" {kit} is also on {}: a change to it changes that profile too", others.join(" "))
    }
}

fn columns(frame: &mut Frame, state: &State, area: Rect) {
    let (c, w) = (state.col.index(), area.width);
    let shown: Vec<(usize, u16)> = if w >= 110 {
        vec![(0, 24), (1, 28), (2, 32), (3, w - 84)]
    } else if w >= 70 {
        if c <= 1 {
            vec![(0, 24), (1, 24), (2, w - 48)]
        } else {
            vec![(1, 22), (2, 26), (3, w - 48)]
        }
    } else {
        vec![(c, w)]
    };
    let rects = Layout::horizontal(shown.iter().map(|(_, w)| Constraint::Length(*w))).split(area);
    for ((col, _), r) in shown.iter().zip(rects.iter()) {
        match col {
            0 => profiles_col(frame, state, *r),
            1 => kits_col(frame, state, *r),
            2 => contents_col(frame, state, *r),
            _ => library_col(frame, state, *r),
        }
    }
}

/// Rows of a column, each with the cursor position it stands for, or `None` for a label.
type Rows = Vec<(Line<'static>, Option<usize>)>;

fn list(frame: &mut Frame, state: &State, area: Rect, col: Col, title: String, rows: Rows) {
    let focused = state.col == col;
    let selected = if focused { rows.iter().position(|(_, s)| *s == Some(state.cursor(col))) } else { None };
    let border = if focused { Style::default().fg(Color::Cyan) } else { dim() };
    let block = Block::bordered().title(Span::styled(title, if focused { bold() } else { Style::default() })).border_style(border);
    let map: Vec<Option<usize>> = rows.iter().map(|(_, s)| *s).collect();
    let items: Vec<ListItem> = rows.into_iter().map(|(l, _)| ListItem::new(l)).collect();
    let scroll = &state.scroll[col.index()];
    render_list(frame, area, block, items, selected, scroll);
    state.hits.borrow_mut().push((inside(area), Hit::Rows { col: col.index(), top: scroll.offset.get(), map }));
}

fn profiles_col(frame: &mut Frame, state: &State, area: Rect) {
    let mut rows: Rows = Vec::new();
    for (i, row) in state.prof_rows().into_iter().enumerate() {
        match row {
            ProfRow::Profile(p) => {
                let on = p == state.profile;
                let mark = if on { "● " } else { "○ " };
                let style = if on { bold() } else { Style::default() };
                rows.push((Line::from(Span::styled(format!("{mark}{}", state.profiles[p].name.as_str()), style)), Some(i)));
            }
            ProfRow::NewProfile => {
                rows.push((Line::raw(""), None));
                rows.push((Line::styled("+ new profile (n)", Style::default().fg(Color::Cyan)), Some(i)));
            }
            ProfRow::Import => rows.push((Line::styled("import (i)", Style::default().fg(Color::Cyan)), Some(i))),
        }
    }
    list(frame, state, area, Col::Profiles, "1 PROFILE".into(), rows);
}

fn kits_col(frame: &mut Frame, state: &State, area: Rect) {
    let mut rows: Rows = vec![(Line::styled("ITS KITS", bold()), None)];
    let all = state.kit_rows();
    if !all.iter().any(|r| matches!(r, KitRow::Kit { on: true, .. })) {
        rows.push((Line::styled("  none yet", dim()), None));
    }
    for (i, row) in all.into_iter().enumerate() {
        match row {
            KitRow::Kit { name, on } => {
                let size = state.kit_sizes.get(&name).copied().unwrap_or(0);
                let open = state.kit.as_deref() == Some(name.as_str()) && state.col > Col::Kits;
                let style = if open { bold() } else { Style::default() };
                let tick = if on { Span::styled("[x] ", Style::default().fg(Color::Green)) } else { Span::styled("[ ] ", dim()) };
                rows.push((Line::from(vec![tick, Span::styled(name, style), Span::styled(format!("  {size}"), dim())]), Some(i)));
            }
            KitRow::New => {
                rows.push((Line::styled("+ new kit (n)", Style::default().fg(Color::Cyan)), Some(i)));
                rows.push((Line::raw(""), None));
                rows.push((Line::styled("OTHER KITS", bold()), None));
            }
        }
    }
    list(frame, state, area, Col::Kits, format!("2 KITS FOR {}", state.profile_label()), rows);
}

/// One `uses` line, as column 3 and the load list show it.
fn line_heading(state: &State, g: &LineGives) -> Line<'static> {
    let line = g.line.line();
    if g.missing {
        return Line::styled(format!("✗ {line}  not in the library"), Style::default().fg(Color::Red));
    }
    match &g.line {
        Entry::Item(ItemKind::Plugin, n) => {
            let inside = state.contents.get(n).map(|c| format!("  {} skills, {} agents", c.skills.len(), c.agents.len())).unwrap_or_default();
            Line::from(vec![Span::styled(format!("◆ {line}"), Style::default().fg(Color::Magenta)), Span::styled(inside, dim())])
        }
        Entry::Folder(kind, _) => Line::from(vec![Span::styled(format!("▣ {line}"), bold()), Span::styled(format!("  {}", plural(g.items.len(), kind.label())), dim())]),
        Entry::Item(kind, _) => Line::from(vec![Span::raw(format!("• {line}")), Span::styled(format!("  {}", kind.label()), dim())]),
    }
}

fn line_items(g: &LineGives) -> Vec<Line<'static>> {
    match g.line {
        Entry::Folder(..) => g.items.iter().map(|i| Line::styled(format!("    {}", i.name), dim())).collect(),
        Entry::Item(..) => vec![],
    }
}

fn contents_col(frame: &mut Frame, state: &State, area: Rect) {
    if state.col == Col::Profiles {
        let title = format!("3 {} loads", state.profile_label());
        let rows = match &state.plan {
            Some(Ok(plan)) => plan_rows(state, plan),
            Some(Err(e)) => vec![(Line::styled(e.clone(), Style::default().fg(Color::Red)), None)],
            None => vec![(Line::styled("n in column 1 makes a profile", dim()), None)],
        };
        return list(frame, state, area, Col::Contents, title, rows);
    }
    let Some(kit) = state.shown_kit() else {
        let rows = vec![(Line::styled(format!("n makes a kit for {}", state.profile_label()), dim()), None)];
        return list(frame, state, area, Col::Contents, "3 a new kit".into(), rows);
    };
    let mut rows: Rows = Vec::new();
    for (i, row) in state.cont_rows().into_iter().enumerate() {
        match row {
            ContRow::Instructions => {
                let text = if state.instructions { "instructions (CLAUDE.md)" } else { "instructions: none · e writes them" };
                rows.push((Line::raw(text), Some(i)));
            }
            ContRow::Line(n) => {
                let g = &state.lines[n];
                rows.push((line_heading(state, g), Some(i)));
                rows.extend(line_items(g).into_iter().map(|l| (l, None)));
            }
        }
    }
    if state.lines.is_empty() {
        rows.push((Line::styled("picks nothing yet: → goes to the library", dim()), None));
    }
    list(frame, state, area, Col::Contents, format!("3 in {kit}"), rows);
}

/// Everything a profile loads (ADR-2610281305 R2), kit by kit.
fn plan_rows(state: &State, plan: &LoadPlan) -> Rows {
    let mut seen: Vec<(ItemKind, String)> = Vec::new();
    for i in plan.kits.iter().flat_map(|k| &k.build) {
        if !seen.contains(&(i.kind, i.path())) {
            seen.push((i.kind, i.path()));
        }
    }
    let count = |k| seen.iter().filter(|(kind, _)| *kind == k).count();
    let (s, a) = plan.plugins.iter().filter_map(|p| state.contents.get(&p.name)).fold((0, 0), |(s, a), c| (s + c.skills.len(), a + c.agents.len()));
    let mut rows: Rows = vec![(Line::styled(format!("{} · {}", plural(count(ItemKind::Skill), "skill"), plural(count(ItemKind::Agent), "agent")), bold()), None)];
    let plugins = plural(plan.plugins.len(), "plugin");
    rows.push(if plan.plugins.is_empty() {
        (Line::styled(plugins, bold()), None)
    } else {
        (Line::from(vec![Span::styled(plugins, bold()), Span::styled(format!(" (+{s} skills, {a} agents)"), dim())]), None)
    });
    if plan.kits.is_empty() {
        rows.push((Line::raw(""), None));
        rows.push((Line::styled("loads nothing from the library yet", Style::default().fg(Color::Yellow)), None));
        rows.push((Line::styled("→ then n makes a kit for it", dim()), None));
    }
    for k in &plan.kits {
        rows.push((Line::raw(""), None));
        rows.push((Line::styled(format!("kit {}", k.kit.as_str()), Style::default().fg(Color::Cyan)), None));
        if k.instructions.is_some() {
            rows.push((Line::raw("  instructions (CLAUDE.md)"), None));
        }
        for g in &k.lines {
            let mut l = line_heading(state, g);
            l.spans.insert(0, Span::raw("  "));
            rows.push((l, None));
            rows.extend(line_items(g).into_iter().map(|mut l| {
                l.spans.insert(0, Span::raw("  "));
                (l, None)
            }));
        }
    }
    for w in &plan.warnings {
        rows.push((Line::styled(format!("! {w}"), Style::default().fg(Color::Yellow)), None));
    }
    rows
}

/// The box for a row; blank while no kit is open, because there is nothing to pick for.
fn mark(state: &State, p: &Pick) -> Span<'static> {
    if state.shown_kit().is_none() {
        return Span::raw("   ");
    }
    match p {
        Pick::Direct => Span::styled("[x]", Style::default().fg(Color::Green)),
        Pick::Through(_) => Span::styled("[^]", Style::default().fg(Color::Cyan)),
        Pick::Partly { .. } => Span::styled("[-]", Style::default().fg(Color::Yellow)),
        Pick::No => Span::styled("[ ]", dim()),
    }
}

fn library_col(frame: &mut Frame, state: &State, area: Rect) {
    let searching = state.mode == Mode::Search;
    let title = if searching {
        format!("4 LIBRARY · /{}▏", state.filter)
    } else if !state.filter.is_empty() {
        format!("4 LIBRARY · \"{}\" · esc clears", state.filter)
    } else {
        "4 LIBRARY · / finds".to_string()
    };
    let flat = !state.filter.is_empty();
    let rows: Rows = state.lib_rows().into_iter().enumerate().map(|(i, row)| (lib_line(state, &row, flat), Some(i))).collect();
    list(frame, state, area, Col::Library, title, rows);
}

/// `[x] ▸ writing/  2/2`: mark, arrow, name, then a count.
fn lib_line(state: &State, row: &LibRow, flat: bool) -> Line<'static> {
    let arrow = |open: bool| if open { "▾ " } else { "▸ " };
    match row {
        LibRow::Kind(k) => {
            let n = state.trees.iter().find(|(kind, ..)| kind == k).map_or(0, |(_, items, _)| items.len());
            Line::from(vec![Span::styled(format!("{}{}", arrow(state.is_open(row)), k.dir().to_uppercase()), bold()), Span::styled(format!("  {n}"), dim())])
        }
        LibRow::Tree(k, r) => {
            let indent = if flat { String::new() } else { "  ".repeat(r.depth) };
            let m = mark(state, &state.mark(row));
            match &r.item {
                None => {
                    let (picked, total) = state.folder_count(*k, &r.path);
                    let label = if flat { row.path() } else { format!("{}/", r.leaf()) };
                    Line::from(vec![Span::raw(indent), m, Span::raw(format!(" {}{label}", arrow(state.is_open(row)))), Span::styled(format!("  {picked}/{total}"), dim())])
                }
                Some(i) if *k == ItemKind::Plugin => {
                    let inside = state.contents.get(&i.name).map(|c| format!("  {} skills, {} agents", c.skills.len(), c.agents.len())).unwrap_or_default();
                    let a = if state.openable(row) { arrow(state.is_open(row)) } else { "  " };
                    Line::from(vec![Span::raw(indent), m, Span::styled(format!(" {a}{}", i.name), Style::default().fg(Color::Magenta)), Span::styled(inside, dim())])
                }
                Some(i) => {
                    let label = if flat { row.path() } else { i.name.clone() };
                    Line::from(vec![Span::raw(indent), m, Span::raw(format!("   {label}"))])
                }
            }
        }
        LibRow::Inside { plugin, part, agent } => {
            let what = if *agent { "agent" } else { "skill" };
            let label = if flat { row.path() } else { part.file.clone() };
            Line::styled(format!("{}    · {label}  {what} in {plugin}", if flat { "" } else { "  " }), dim())
        }
    }
}

/// The two lines under column 4: the row's path and its whole description.
fn strip_text(state: &State) -> String {
    let Some(row) = state.lib_row() else { return String::new() };
    let about = match &row {
        LibRow::Kind(k) => format!("every {} in the library; enter opens or closes", k.label()),
        LibRow::Tree(k, r) => match &r.item {
            Some(i) => i.description.clone().unwrap_or_else(|| "(no description)".into()),
            None => {
                let (_, total) = state.folder_count(*k, &r.path);
                format!("a folder of {}; picking it picks them all, and any added later", plural(total, k.label()))
            }
        },
        LibRow::Inside { plugin, part, .. } => format!("inside the plugin {plugin}: {}", part.description.as_deref().unwrap_or("(no description)")),
    };
    format!(" {}  {about}", row.path())
}

/// The key line: what the keys do on the row under the cursor, in words.
pub(super) fn hints(state: &State) -> String {
    let p = state.profile_label();
    match &state.mode {
        Mode::Search => return " type to find · enter keeps the results · esc clears".into(),
        Mode::Input { purpose, .. } => {
            let verb = match purpose {
                Purpose::NewKit | Purpose::NewProfile { .. } | Purpose::Mkdir { .. } => "makes it",
                Purpose::RenameKit { .. } | Purpose::RenameFolder { .. } => "renames",
                Purpose::Add { .. } => "adds it",
            };
            let tab = if matches!(purpose, Purpose::Add { .. }) { " · tab completes" } else { "" };
            return format!(" enter {verb}{tab} · esc cancels");
        }
        Mode::Confirm(_) => return " y yes · any other key no".into(),
        Mode::View { .. } => return " ↑↓ PgUp PgDn scroll · esc closes".into(),
        Mode::Import { .. } => return " ↑↓ choose · enter imports · esc closes".into(),
        Mode::MoveTo { .. } => return " type a folder · ↑↓ choose · enter moves · esc closes".into(),
        Mode::Normal => {}
    }
    match state.col {
        Col::Profiles => match state.prof_rows().get(state.psel) {
            Some(ProfRow::Profile(_)) => " enter starts claude · → its kits · ↑↓ profile · e edit · d default · c copy · ! shell · n new · i import · q quit".into(),
            Some(ProfRow::NewProfile) => " enter makes a profile · ↑↓ profile · i import · q quit".into(),
            _ => " enter opens the import list · ↑↓ profile · q quit".into(),
        },
        Col::Kits => match state.kit_rows().get(state.ksel) {
            Some(KitRow::Kit { name, on: true }) => {
                format!(" space takes {name} off {p} · → open · ← back · n new kit · e instructions · r rename · x delete · z undo")
            }
            Some(KitRow::Kit { name, on: false }) => {
                format!(" space puts {name} on {p} · → open · ← back · n new kit · e instructions · r rename · x delete · z undo")
            }
            _ => format!(" enter makes a kit for {p} · n new kit · ← back · ↑↓"),
        },
        Col::Contents => match state.cont_rows().get(state.csel) {
            Some(ContRow::Line(n)) => format!(" space drops {} · → library · ← back · z undo", state.lines[*n].line.line()),
            _ => " e edits the instructions · → library · ← back · ↑↓".into(),
        },
        Col::Library => {
            let kit = state.kit.clone().unwrap_or_default();
            let Some(row) = state.lib_row() else { return " / finds · a adds to the library · ← back".into() };
            match (&row, state.mark(&row)) {
                (LibRow::Kind(_), _) => " enter opens or closes · / finds · ← back".into(),
                (LibRow::Inside { plugin, .. }, _) => format!(" inside {plugin}: pick {plugin} to load it · / finds · ← back"),
                (_, Pick::Direct) => format!(" space drops {} from {kit} · / finds · ← back · v reads · z undo", row.entry().map(|e| e.line()).unwrap_or_default()),
                (_, Pick::Through(f)) => format!(" comes with {f} · / finds · ← back · v reads"),
                _ => format!(" space adds {} to {kit} · / finds · ← back · enter opens · v reads · a add · z undo", row.entry().map(|e| e.line()).unwrap_or_default()),
            }
        }
    }
}

/// A box in the middle of `area`.
fn centred(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect { x: area.x + (area.width - w) / 2, y: area.y + (area.height - h) / 2, width: w, height: h }
}

fn overlay(frame: &mut Frame, state: &State, area: Rect) {
    match &state.mode {
        Mode::View { title, text, scroll } => {
            state.view_scroll.page.set(area.height.saturating_sub(2) as usize);
            frame.render_widget(Clear, area);
            let p = Paragraph::new(text.clone()).block(Block::bordered().title(Span::styled(format!(" {title} "), bold()))).scroll((*scroll, 0));
            frame.render_widget(p, area);
        }
        Mode::Input { purpose, text, candidates } => {
            let mut lines = vec![Line::styled(state.input_label(purpose), bold()), Line::raw(format!("> {text}▏"))];
            lines.extend(candidates.iter().take(5).map(|c| Line::styled(format!("  {c}"), dim())));
            let r = centred(area, 76, cells(lines.len()).saturating_add(2));
            frame.render_widget(Clear, r);
            frame.render_widget(Paragraph::new(lines).block(Block::bordered().border_style(Style::default().fg(Color::Cyan))), r);
        }
        Mode::Import { items, sel } => {
            let r = centred(area, 90, cells(items.len()).saturating_add(2));
            frame.render_widget(Clear, r);
            let rows: Vec<ListItem> = items.iter().map(|i| ListItem::new(i.label())).collect();
            let list = List::new(rows).block(Block::bordered().title(" import ")).highlight_style(Style::default().add_modifier(Modifier::REVERSED));
            let mut list_state = ListState::default().with_selected(Some(*sel));
            frame.render_stateful_widget(list, r, &mut list_state);
            // The list scrolls to keep `sel` in view; screen line i shows item offset + i.
            let top = list_state.offset();
            let rows_area = inside(r);
            for i in 0..items.len().saturating_sub(top).min(rows_area.height as usize) {
                state.hits.borrow_mut().push((Rect { y: rows_area.y.saturating_add(cells(i)), height: 1, ..rows_area }, Hit::Choice(top + i)));
            }
        }
        Mode::MoveTo { entry, kind, filter, sel } => {
            let choices = state.move_choices(entry, *kind, filter);
            let r = centred(area, 70, cells(choices.len()).min(12) + 4);
            frame.render_widget(Clear, r);
            let [head, rest] = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).areas(r);
            frame.render_widget(Paragraph::new(format!("> {filter}▏")).block(Block::bordered().title(format!(" move {} to ", entry.line()))), head);
            let rows: Vec<ListItem> =
                choices.iter().map(|c| ListItem::new(if c.is_empty() { format!("{}/ (the top)", kind.dir()) } else { format!("{}/{c}/", kind.dir()) })).collect();
            let list = List::new(rows).highlight_style(Style::default().add_modifier(Modifier::REVERSED));
            frame.render_stateful_widget(list, rest, &mut ListState::default().with_selected(Some(*sel)));
        }
        Mode::Normal | Mode::Search | Mode::Confirm(_) => {}
    }
}
