//! Lists that keep their place between frames, and the click map.

use super::Key;
use ratatui::layout::{Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, List, ListItem, ListState, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;
use std::cell::Cell;

/// How far a key moves a cursor, given the page height; `None` when it is not a movement.
pub(super) fn movement(key: Key, page: usize) -> Option<isize> {
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
pub(super) fn shifted(sel: usize, delta: isize, len: usize) -> usize {
    (sel as isize).saturating_add(delta).clamp(0, len.saturating_sub(1) as isize) as usize
}

/// A list's scroll offset and page height, kept between frames.
#[derive(Default)]
pub(super) struct Scroll {
    pub(super) offset: Cell<usize>,
    pub(super) page: Cell<usize>,
}

/// Draw a list that keeps its place between frames and follows its cursor,
/// with a scrollbar when it overflows.
pub(super) fn render_list(frame: &mut Frame, area: Rect, block: Block, items: Vec<ListItem>, selected: Option<usize>, scroll: &Scroll) {
    let len = items.len();
    let inner = area.height.saturating_sub(2) as usize;
    scroll.page.set(inner);
    if let Some(row) = selected {
        let top = scroll.offset.get();
        if row < top {
            scroll.offset.set(row);
        } else if inner > 0 && row >= top + inner {
            scroll.offset.set(row + 1 - inner);
        }
    }
    let block = match selected {
        Some(i) if len > inner => block.title_bottom(Line::from(format!(" {}/{len} ", i + 1)).right_aligned()),
        _ => block,
    };
    let list = List::new(items).block(block).highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    let mut ls = ListState::default().with_selected(selected).with_offset(scroll.offset.get());
    frame.render_stateful_widget(list, area, &mut ls);
    scroll.offset.set(ls.offset());
    if len > inner {
        let mut state = ScrollbarState::new(len.saturating_sub(inner)).position(ls.offset());
        let bar = Scrollbar::new(ScrollbarOrientation::VerticalRight).begin_symbol(None).end_symbol(None);
        frame.render_stateful_widget(bar, area.inner(Margin { vertical: 1, horizontal: 0 }), &mut state);
    }
}

/// Something on screen a click can land on, recorded as each frame is drawn.
#[derive(Debug, Clone)]
pub(super) enum Hit {
    /// A column's rows. Screen line `n` of the area is row `map[top + n]`, or a
    /// label when `None`. A click on the first four characters is a click on the box.
    Rows { col: usize, top: usize, map: Vec<Option<usize>> },
    /// A step of the top line: go to that column.
    Step(usize),
    /// A line of a list box (the import list).
    Choice(usize),
}

/// The rows' area of a bordered box.
pub(super) fn inside(area: Rect) -> Rect {
    area.inner(Margin { vertical: 1, horizontal: 1 })
}
