# Primary Adapters

## Overview

Primary (driving) adapters start work in the application. There is one: the
ratatui terminal UI in `tui/`. It is one screen of four columns, used left to
right (ADR-2610281305): **1 profile → 2 its kits → 3 what is in the kit → 4 add
from the library**. When it ends, it returns an `Exit` that tells the
composition root (`src/lib.rs`) what to do once the terminal is restored:
quit, launch Claude Code, or open a shell.

The TUI does all of its work through the `ports::Browser` driving port. `State`
takes keys and `draw` paints the screen, so tests drive the screen with no
terminal (`tests/tui.rs`, `tests/parity.rs`). Only `run` and its private
`drive` loop touch the real terminal.

## Interface

- `mod.rs` declares `pub mod tui`. The folder `tui/` holds:
  - `mod.rs`: `run`, the event loop, `Key`, `map_key`, `Exit`.
  - `columns.rs`: `State`, the four columns, and what the keys do in each.
  - `overlays.rs`: the search, typed text, the y/n question, the `v` reader,
    the import list and the folder list; the editor handoff.
  - `draw.rs`: `draw`, the key line and the boxes.
  - `widgets.rs`: lists that keep their place, and the click map.
- `tui::run(b: &dyn Browser) -> std::io::Result<Exit>` is the entry point that
  `src/lib.rs` calls.
- `tui::Exit`: `Quit`, `Launch(String)`, `LaunchWith { profile, kits }`,
  `Shell(String)`.
- `tui::Key`: `Up`, `Down`, `Left`, `Right`, `Tab`, `Enter`, `Esc`,
  `Backspace`, `PageUp`, `PageDown`, `Home`, `End`, `WheelUp`, `WheelDown`,
  `Click(col, row)`, `Back` (right click) and `Char(char)`.
- `tui::State`: `new(b)`, `handle(&mut self, b, key) -> Option<Exit>`,
  `wants_editor()`, `run_editor(b)`.
- `tui::draw(frame: &mut Frame, state: &State)`.

### The screen, from top to bottom

1. The step line: `ccs  1 work › 2 research › 3 in research › 4 add from library`.
   Each step is a click target.
2. The "also on" line: the open kit is on another profile too, so a change to
   it changes that profile.
3. The columns. At 110 columns or more, all four show; from 70 to 109, three
   show and slide with the focus; below 70, one.
4. In column 4 only, two lines with the current row's path and its whole
   description.
5. The notice: the `ccs` command the last change matches, or what went wrong.
6. The key line: what the keys do on the current row, in words. The action
   comes first, then `→` and `← back`, so the way on and the way back stay in
   view at 80 columns.

### Keys

| Where | Keys |
|---|---|
| Everywhere | `↑↓` (or `k j`), `PgUp PgDn`, `Home End` (or `g G`), the wheel move; `→` goes right; `←` goes back; `z` undoes the last change; `i` imports; `q` or Ctrl-C quits |
| 1 profile | Enter starts Claude Code (only here); `e` edits `profile.env`; `d` makes it the default; `c` copies it; `!` opens its shell; `n` makes a profile; Esc quits |
| 2 kits | space puts a kit on the profile or takes it off; Enter opens it; `n` makes a kit and puts it on the profile; `e` edits its instructions; `r` renames; `x` deletes after asking |
| 3 in the kit | space drops a line; `e` on the first row edits the instructions |
| 4 library | space picks or unpicks; Enter opens a folder or plugin, or picks an item; `←` closes an open folder, else goes back; `/` searches; `v` reads; `e` edits; `a` adds; `n` makes a folder; `m` moves; `r` renames a folder; `x` deletes; `u` updates plugins; Esc clears a search |
| The mouse | a click selects a row; a click on the selected row, or on its box, acts on it; a click on a step goes there; a right click goes back and never quits |

## Invariants

- **Ports only.** Every file in `tui/` imports from `crate::ports`, `ratatui`
  and `std` only. `tests/boundaries.rs` checks every file under
  `src/adapters/primary/`, and `hexa analyze .` checks the dependency.
- **Every change goes through `Browser`**, and through the same use case as the
  CLI command it shows. `tests/parity.rs` runs each displayed command on a
  second home and compares the two (ADR-2610281305 G3).
- **Item lines stay name-only** (`skills/<name>`); the folder is shown, never
  written.
- **Nothing is cached between changes.** After each change `State` reads
  everything again from `Browser`. The cursor stays on the same row: a kit
  that moves from "other kits" to the profile's own keeps the cursor, and a
  cleared search opens the folders that hide the row.
- **Enter launches only in column 1.** Esc goes back one column, and quits only
  from column 1. A right click never quits.
- **The TUI does not launch anything.** It returns an `Exit`, and `src/lib.rs`
  launches once the terminal is restored.
- **The terminal is restored on every return path**, mouse capture first, so
  the shell receives no stray mouse input. Before an edit the editor gets the
  real terminal, after a key or a click.
- **Opening profile.** The screen opens on the profile `ccs` would launch with
  no name: the pin file, then `$CCS_PROFILE`, then the default.
- **No kit open, no boxes.** While column 1 has the focus, column 4 shows the
  library without marks, since there is nothing to pick for.

## Side Effects

- Terminal: the alternate screen, mouse capture, key and mouse events.
- Through `Browser`: profiles made and copied, the default set, kits made,
  renamed, deleted, put on and taken off profiles, picks added and dropped,
  library items added, moved, renamed and deleted, plugins updated, imports and
  migration run, and files opened in `$VISUAL` / `$EDITOR`.

## Verification

- Run: `cargo test`
- The column screen: `cargo test --test tui columns_` (G1, G1b, G2 and the
  column rules)
- The TUI and the CLI agree: `cargo test --test parity` (G3)
- Boundaries: `cargo test --test boundaries` and `hexa analyze . --grade A+ --exit-code`
