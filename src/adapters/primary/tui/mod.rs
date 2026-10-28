//! The ccs TUI: four columns, profile first (ADR-2610281305) — a primary adapter.
//!
//! 1 profile → 2 its kits → 3 what is in the kit → 4 add from the library.
//! Rule 4: it imports `ports/` only. Everything it does goes through the
//! `Browser` driving port. `State` takes keys and `draw` paints it, so tests
//! drive the screen without a terminal; `run` is the only part that touches one.

mod columns;
mod draw;
mod overlays;
mod widgets;

pub use self::columns::State;
pub use self::draw::draw;

use crate::ports::Browser;
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::crossterm::execute;

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
    /// A left click at a column and row of the screen.
    Click(u16, u16),
    /// A right click: back one column; never quits.
    Back,
    Char(char),
}

/// Why the TUI ended. Launching happens after the terminal is restored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exit {
    Quit,
    /// Launch with the profile's own kits.
    Launch(String),
    /// Launch with these kits, for this launch only.
    LaunchWith { profile: String, kits: Vec<String> },
    /// A shell in the profile's folder, with its variables (ADR-2610261643 §14).
    Shell(String),
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

/// The columns, until the user quits or launches. The terminal is restored
/// before this returns, on every path.
pub fn run(b: &dyn Browser) -> std::io::Result<Exit> {
    drive(b, &mut State::new(b))
}

/// The alternate screen with the mouse on; `ratatui::init` leaves the mouse off.
fn open_terminal() -> ratatui::DefaultTerminal {
    let terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
    terminal
}

fn close_terminal() {
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
}

/// Hand the terminal to the editor when a key asked for it, then take it back.
fn edit(b: &dyn Browser, state: &mut State) -> bool {
    if !state.wants_editor() {
        return false;
    }
    close_terminal();
    state.run_editor(b);
    true
}

fn drive(b: &dyn Browser, state: &mut State) -> std::io::Result<Exit> {
    let mut terminal = open_terminal();
    let result = loop {
        if let Err(e) = terminal.draw(|f| draw(f, state)) {
            break Err(e);
        }
        let key = match event::read() {
            Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => {
                if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
                    break Ok(Exit::Quit);
                }
                match map_key(k.code) {
                    Some(key) => key,
                    None => continue,
                }
            }
            Ok(Event::Mouse(m)) => match m.kind {
                MouseEventKind::ScrollDown => Key::WheelDown,
                MouseEventKind::ScrollUp => Key::WheelUp,
                MouseEventKind::Down(MouseButton::Left) => Key::Click(m.column, m.row),
                MouseEventKind::Down(MouseButton::Right) => Key::Back,
                _ => continue,
            },
            Ok(_) => continue,
            Err(e) => break Err(e),
        };
        if let Some(exit) = state.handle(b, key) {
            break Ok(exit);
        }
        // A key or a click can ask for an edit.
        if edit(b, state) {
            terminal = open_terminal();
        }
    };
    // Mouse capture off first, or the shell receives every later mouse move as text.
    close_terminal();
    result
}
