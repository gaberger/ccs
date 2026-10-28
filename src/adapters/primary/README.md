# Primary Adapters

## Overview

Primary (driving) adapters start work in the application. There is one: the
ratatui terminal UI in `tui.rs`. It shows profiles, kits and the library, and
it manages them. It returns an `Exit` value that tells the composition root
(`src/lib.rs`) what to do after the terminal is restored: quit, launch Claude
Code, or open a shell.

The TUI does all of its work through the `ports::Browser` driving port. Two
parts are kept apart: `State` takes keys and `draw` paints the screen. Because
of this split, tests drive the screen with no terminal (`tests/tui.rs`). Only
`run` and its private `drive` loop touch the real terminal.

## Interface

- `mod.rs` declares `pub mod tui`.
- `tui::run(b: &dyn Browser) -> std::io::Result<Exit>` is the entry point that
  `src/lib.rs` calls. It opens the alternate screen with mouse capture on,
  loops on terminal events, and restores the terminal before it returns, on
  every path.
- `tui::Exit` tells the caller why the TUI ended:
  - `Quit`
  - `Launch(String)`: launch a profile with its own kits.
  - `LaunchWith { profile, kits }`: launch a profile with these kits, for this
    launch only.
  - `Shell(String)`: open a shell in the profile's folder, with its variables.
- `tui::Key` is the adapter's own key type, so tests need no terminal events.
  Variants: `Up`, `Down`, `Left`, `Right`, `Tab`, `Enter`, `Esc`, `Backspace`,
  `PageUp`, `PageDown`, `Home`, `End`, `WheelUp`, `WheelDown`,
  `Click(col, row)`, `Back` (right click) and `Char(char)`.
- `tui::State` is the screen model:
  - `State::new(b)` loads profiles, kits and the library through the port.
  - `State::handle(&mut self, b, key) -> Option<Exit>` applies one key.
  - `State::wants_editor()` reports whether a key queued an edit.
  - `State::run_editor(b)` runs the queued edit. The caller must give the
    terminal to the editor first.
- `tui::draw(frame: &mut Frame, state: &State)` paints the breadcrumb, the
  page and the footer from `state`. It also records where each clickable area
  is, for mouse hits.
- These parts are private: the `Screen` trait, the `drive` loop, `map_key`,
  the `Picker` used on Home, and the page, overlay and list renderers.

## Invariants

- **Ports only.** `tui.rs` imports from `crate::ports` and external crates
  (`ratatui`, `std`) only. It never imports `crate::usecases`,
  `crate::domain` or `crate::adapters::secondary`. `tests/boundaries.rs`
  (`primary_adapters_import_ports_only`) checks this against the real source
  and is stricter than `hexa analyze`.
- Every action goes through a `Browser` method. Nothing in this directory
  touches the filesystem or starts a process directly. The only exception is
  terminal I/O.
- The TUI does not launch anything. When it ends, it returns an `Exit`, and the
  composition root launches once the terminal is restored.
- The terminal is restored on every return path from `run`, including errors.
  Mouse capture is turned off before `ratatui::restore()`. Otherwise the shell
  receives later mouse moves as text.
- Before an edit, the editor gets the real terminal. `Screen::edit` leaves the
  alternate screen, runs the edit, and the loop opens the terminal again.
  `drive` runs a queued edit after a mouse event as well as after a key, so a
  double-click that asks for an edit opens the editor at once.
- Ctrl-C always quits with `Exit::Quit`. On Home, `q` and `Esc` quit. On the
  profile, kit and library pages, `q` quits. In a text view, `q`, `Esc` and a right click close
  the view and do not quit. A right click (`Key::Back`) never quits.
- Below 70 columns (`NARROW`), the list and the body stack vertically so that
  neither is cut off.

## Side Effects

- Terminal: enters and leaves the alternate screen, turns mouse capture on and
  off, and reads key and mouse events (`crossterm`).
- Indirect, through `Browser`: the TUI can create profiles and kits, rename
  and remove kits, set the default profile, set a profile's kits, attach a
  kit to a profile, change a kit's `uses`, add, move
  and trash library items, import AWS profiles and stock `~/.claude` items,
  migrate an old layout, update plugins, and open files in `$VISUAL` /
  `$EDITOR`. The secondary adapters that `src/lib.rs` wires in do the actual
  work.

## Verification

- Run: `cargo test`
- TUI behaviour: `cargo test --test tui`
- Boundary gate: `cargo test --test boundaries`
