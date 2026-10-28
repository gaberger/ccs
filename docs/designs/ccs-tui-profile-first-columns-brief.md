# ccs TUI redesign: brainstorm brief

The decision that follows from this brief: [ADR-2610281305](../adrs/ADR-2610281305-profile-first-columns.md).
* **Author:** Gary Berger
* **Date:** 2026-10-28

## Overview

The ccs TUI fails at five jobs:

1. Showing what a kit holds.
2. Showing what the library holds and what is inside each entry.
3. Showing what is available but not yet picked.
4. Picking and unpicking items with no guessing.
5. Making a new kit.

Profiles, kits and the library stay as concepts. Only the ccs TUI changes. The redesign runs in the terminal, takes keyboard and mouse, has a CLI form for every TUI action, and never writes `~/.claude`. A plugin is one switch that can be opened to see what it holds.

**Success test:** make a kit `research` with plugin `egm` and two skills from `writing/`, attach it to profile `sticksandstrings` in under a minute with no help screen, then see on one screen everything that profile loads.

Three designs were reviewed:

- **D1, dual-pane builder** (`tui_designs/design_1_dual_pane_builder.md`): kit-first. Library tree on the left, the kit being edited on the right, a load bar at the bottom.
- **D2, catalog matrix** (`tui_designs/design_2_catalog_matrix.md`): library rows against kit columns of checkboxes.
- **D3, profile-first board** (`tui_designs/design_3_profile_first_board.md`): one profile's loadout, plus a quick-add search.

Six reviewers read all three designs and the code: design, codebase accuracy, scope, first-time-user comprehension, implementation readiness, and structure/maintainability. This brief merges their findings.

**Recommendation (revised after the user tried a prototype):** build the four-column design, called D4 below. You move left to right: **1 profile → 2 its kits → 3 what is in the kit → 4 add from the library**. Each column narrows the next. You still edit one kit at a time, as the review asked, but you reach the kit through the profile. On a profile, column 3 shows everything that profile loads, so the "what does it load" view is the first step and not a separate screen. It is backed by a side-effect-free load plan that the launcher also uses. D2 is still rejected. From D1, D4 keeps the library tree, the marks and the kit contents. From D3 it keeps one idea: a new kit goes onto the profile you are on, and the screen names that profile.

The first recommendation, a reduced D1 that centred on the kit with the profile as a read-only view, did not survive testing; see "What the prototype showed".

**Prototype:** `tui_designs/prototype.html`, published at https://claude.ai/artifact/Gvs3AtKNVPEoCaKm8ozQtE. It runs all four designs on a snapshot of the real library and times the success test. The snapshot comes from `tui_designs/prototype_snapshot.py`, and `tui_designs/prototype.template.html` is the editable source.

## Analysis

### What the prototype showed (user testing, 2026-10-28)

The user tried D1-D3 in the prototype, then two revisions. Their three notes changed the design more than the six reviewers did:

1. **"It was harder to navigate around profiles."** In D1-D3 the profile is a name in a bar, a pop-up list, or a screen you step through with `[` `]`. None shows the profiles and their kits together. A first revision, a rail that listed every profile and every kit with checkboxes, mixed the two lists.
2. **"Could not read the skill, so not sure what was in it."** Descriptions were cut to one row, and nothing opened the item. Picking by name alone fails: names such as `fn-parity` say little. The real TUI has the same gap today.
3. **"Select the profile first, then select a kit."** This gave D4's left-to-right columns. The user's verdict on D4: "much better".

Scripted runs of the prototype also found three faults that no reviewer saw. The build must avoid each one:

- **Search must rank name matches first.** A plain substring search for `egm` listed the skill `plan-segmentation-check` first ("s-egm-entation"), and `writing` listed `validate-nqe-query` (its description says "writing"). Rank an exact name, then a name that starts with the text, then a name that contains it, then the path, then the description. Plugins and folders come before items at the same rank.
- **`←` must leave the column.** When `←` first climbed the folder tree, getting back from the library to the profile took many presses with no visible reason. `←` closes an open folder; otherwise it goes back one column.
- **Every item must be readable where it is picked.** A two-line strip under the library always shows the full description of the current row, and `v` opens the item's own file text (the first 40 lines), with `esc` to close. For a plugin, `v` lists its skills and agents; for a folder, each item with its description.

Success-test runs with a script that knew every key: D1 35, D2 30, D3 27, D4 33. The script measures the shortest path, not a first try. The numbers show only that D4's extra structure costs about three keys.

### Blockers (decide before any UI work)

**B1. Every design writes item lines in a form ccs rejects.** All six reviewers found this.
- All three designs show item lines and CLI echoes such as `skills/writing/humanize-writing`.
- `Use::parse` (`src/domain/mod.rs:413-428`) accepts an item only as `kind/<one segment>`. Only folder picks carry a path, and they end in `/`. Items are picked by leaf name, which is unique per kind, so a pick survives a move between folders (ADR-2610261643 §2-§3).
- As written, `ccs kit use research skills/writing/forward-generate-report` fails. The promise that "the TUI teaches the CLI" would teach commands that do not work.
- **Decision needed:** keep name-only item lines and show the folder only as display context (recommended, no format change), or change the format. A format change needs an ADR, a reader that accepts both forms, and a compatibility rule.

**B2. `prepare_launch` cannot back a preview.**
- All designs say the load view must use the launcher's own function. But `prepare_launch` calls `kits.build` (`src/usecases/mod.rs:69`), which deletes and rebuilds `$CCS_HOME/.build/<kit>`, and it writes the prompt file (`:84`).
- If the load bar refreshes through it, ccs would rebuild kits on every pick.
- **Fix:** split it into a pure `load_plan` (resolve, dedupe plugins, warnings, argv) and a build step. `prepare_launch` becomes plan plus build, and the Load view and `ccs loads` call only the plan. Every design needs this refactor, and it is the main back-end piece.

**B3. Edit semantics are unstated, and today's writes are unsafe for a TUI that writes on every toggle.**
- `FsKitStore::set_uses` (`src/adapters/secondary/fs_kits.rs:121-125`) does a plain `fs::write` of the whole file:
  - it is not atomic;
  - it takes no lock, so a TUI and a CLI writing the same kit lose each other's changes;
  - it drops `#` comments, because `uses()` skips them (`:116`) and the rewrite does not restore them.
- No design says whether a pick is written at once or on save, how undo works, or what happens when the file changed on disk.
- **Required:**
  - Writes happen immediately, through a temp file and rename.
  - Re-read the file before each change.
  - Keep comments, or warn before dropping them.

### Designer 1's name-collision claims (verified against code)

**Kit and plugin with the same name: real, already warned, not prevented.**
- A kit is built as a plugin whose `plugin.json` name is the kit name (`fs_kits.rs:36-41`). A picked plugin loads from its own folder via `--plugin-dir` (`usecases/mod.rs:83`). So kit `egm` plus plugin `egm` load as two plugins with one name.
- `prepare_launch` already warns about this at launch (`usecases/mod.rs:79-82`). It does not refuse.
- `kit_new` (`usecases/mod.rs:702-709`) and `KitName::parse` (`domain/mod.rs:599-608`) never check plugin names.
- **Gap:** this check belongs where the kit is named. Add it to `kit_new` and `kit_rename` so the TUI and the CLI both get it. A separate `name_check` Browser method is not needed.

**Two same-named skills from different folders in one kit: not real through ccs.**
- `library_add` refuses a duplicate name per kind (`usecases/mod.rs:532-534`), and that is a documented invariant (`src/usecases/README.md:116-117`). Import and migrate skip duplicates (`usecases/mod.rs:363`). A `uses` line cannot even say which folder's copy it means.
- A duplicate can only arrive if someone copies files into the library by hand. Then:
  - **Skills:** the symlink fails (`fs_kits.rs:46`) and the launch aborts with an error. Nothing is lost silently.
  - **Agents:** `fs::copy` silently overwrites (`fs_kits.rs:50`). This is a real, if unlikely, silent loss.
- This is a library-integrity (`doctor`) check, not a TUI feature.

**A collision that exists today, which none of the designs saw:** agent front-matter names.
- `agents/claude-flow/github/pr-manager.md` and `agents/claude-flow/templates/github-pr-manager.md` both declare `name: pr-manager`, so kit `claude-flow` loads two agents under one name.
- Many front-matter names differ from file names (`data-ml-model.md` declares `ml-developer`).
- Any "name as Claude sees it" column built from file stems is therefore wrong. D1 §3e shows `egm:bootstrap_orchestrator`; the front matter says `egm-bootstrap-orchestrator`.
- A `front_matter_name` helper is needed. Only `front_matter_description` exists (`domain/mod.rs:298`).

### Comparison of the three designs

| | D1 dual-pane builder | D2 catalog matrix | D3 profile board |
|---|---|---|---|
| Job 1: what a kit holds | Best. The kit pane lists items under the line that brings them. | Weak. Scan 139 rows or filter. | Good for attached kits. The 4 unattached kits are read-only in a drawer. |
| Job 2: what the library holds | On screen, one tree | On screen | Needs a second view |
| Job 3: available, not picked | `[ ]` marks | `·` cells plus a filter | Spread over three places |
| Job 4: pick/unpick without guessing | Best. The footer says in words what Space will do on this row. | 2-D cursor can pick into the wrong kit (its own §10). Space on a row inside a plugin drops the whole plugin, which breaks "one switch". | Three keys (`a`, `x`, Space) whose meaning changes by screen |
| Job 5: new kit | Good. `n` with a live name check; the empty pane lists next steps. | Adequate | Smoothest, but attaches silently to the current profile |
| Plugin as one switch | Yes. Rows inside a plugin cannot be toggled. | No | Yes |
| Claimed / recounted keystrokes | 32 / 32 | 35 / 35 | 30 / 30, but attach is implicit and "see the load" is a key labelled "group by kind" |
| New Browser methods (as designed) | ~9-10 | ~8 | ~8 plus palette and context menu |
| New CLI commands/flags (as designed) | 4 | 5-6, including `kit matrix` | 7 |
| tui.rs that survives | Most of the three, because `library_tree` + `pick` already is its tree | Least useful reuse; needs a new grid widget | Low |

**How much of `tui.rs` survives.** Reviewers' estimates ranged from 15% to 50% (about 330 to 1100 of 2200 lines). They agree on what survives:
- `Key`/`map_key` (21-42, 1871);
- `Scroll`/`render_list`/`scrollbar` (67-137);
- `Exit` (141);
- the `Hit` click registry (314-338);
- `drive`/`run`, terminal open/close and the editor handoff (2145-2231);
- the import and AWS overlays;
- the Input, Pick, View, Confirm and Search modes.

`State` (371-1480), `draw_home`, `draw_profile`, `draw_kit`, `draw_library` and `Picker` are rewritten. Most of `tests/tui.rs` is rewritten, and the `Fake` Browser (`tests/tui.rs:106`) must implement every new method. Plan on roughly a third surviving.

**Center: the kit or the profile?** Five of six reviewers say the kit for editing and the profile for viewing.
- Profiles are two logins that rarely change. Kits are what the user edits.
- Editing one kit at a time removes the wrong-kit risk (D2) and the "this kit is shared, change both?" prompt (D3).
- Half the kits (4 of 8) are attached to no profile. A profile-centred editor makes them second-class.
- The success test edits one kit, then views one profile.

*Overruled by user testing on 2026-10-28:* the user found profiles hard to reach in a kit-centred screen and asked to choose the profile first. D4 keeps the reviewers' point that matters, one kit edited at a time with no two-way cursor, and lists every kit not on the profile under "other kits", so the unattached kits are not second-class.

**Hybrid.** A narrow hybrid is better than any single design: D1's builder as the only place to edit, plus one read-only, profile-centred Load view. Every design needs the same domain core, so the hybrid is clean in structure. Anything broader is scope creep:
- D3's quick-add duplicates D1's `/` filter.
- D3's palette and right-click menu are help screens under another name.
- Editing on two screens would split the model.

### Success-test honesty

- The arithmetic holds for all three designs. Every count assumes an expert, error-free path and a cursor that lands on the right row after filtering.
- **Wrong-profile trap (all three):** every design opens on `claude-bedrock-gary`. A newcomer who does not switch first attaches the kit to the wrong profile.
  - D3 is the worst case: `n` makes and attaches in one step.
  - D2 attaches to whichever profile is current, and does not say which kit column stays active after a profile switch.
  - D1 is safest: its hint reads "a attach research to sticksandstrings".
- **"No help screen":**
  - Honest for D1, with one weak step: the `3` (plugins only) key is shown only in the empty-kit pane. A newcomer types `/egm` instead, about 2 more keys.
  - Mostly honest for D2: the user must know to type a trailing slash in `/writing/`.
  - Least honest for D3: its palette and context menu do the job of a help screen.
- **"One screen":** about 20 rows (9 skills, 9 agents, 1 plugin). They fit at 120x36 but not at 80x24, where D2 and D3 fold rows. The test should name a terminal size.
- **Data errors in the mockups:**
  - egm has 7 skills and 9 agents. Its manifest lists 9 agents, so D3's "8 declared" is wrong and its manifest-vs-files edge case does not exist.
  - D1 labels kit `test` 31; it gives 29. 31 is the profile's total.
  - D3 says kit `base` has no CLAUDE.md; it has one.

### Vocabulary

- File-format words leak into all three designs: "line", "uses line", "own line", "split the folder".
- "Starts with" can be read as name matching.
- Each design adds its own nouns:
  - D1: Builder, load bar, Inspector.
  - D2: catalog, lens, rail, cell.
  - D3: board, loadout, drawer, quick-add, target kit.
- Keep to: profile, kit, library, plugin, folder, pick/unpick, attach, loads.

### Smaller Major findings

- **Contents of a plugin.**
  - No port lists them; `Library::read` returns `plugin.json`, and `library(kind)` returns the plugin as one row.
  - Add `Library::plugin_contents` (secondary side) and expose it through `Browser`.
  - Count files the way Claude Code finds them, and read front-matter names.
  - `manifest_field` returns None for arrays (`domain/mod.rs:451`), so it cannot be reused to read the manifest's agent list.
- **Key clashes.** `movement` binds `j`, `k`, `g`, `G` (`tui.rs:48-53`). D1's `k`, D2's `g`/`G` and D3's `g` all clash with it.
- **Right-click.** Right-click means Back today (`tui.rs:2213`). D3's context menu conflicts with that.
- **Contract and ADR drift.**
  - All three designs make Esc never quit; the `src/adapters/primary/README.md` invariants say Esc quits on Home.
  - D3 silently drops `Exit::Shell` and the profile page.
  - No design states "never writes `~/.claude`" as a top-level constraint.
  - CLAUDE.md requires an ADR before new port methods, so ADR-2610261643 §18-§21 must be superseded.
- **CLI parity has holes.**
  - D1's hide has no command.
  - D2's and D3's undo has no CLI form.
  - D3's make-and-attach and shared-kit copy-and-swap are multi-step and not atomic.
  - One-level undo is acceptable if it is defined as the inverse `kit use`/`kit drop`, which does have a CLI form.
- **Maintenance.** `tui.rs` is already about 2200 lines. Split it into a `tui/` module, and keep `tests/boundaries.rs` checking every file.

## Goals vs. Non-Goals

**Goals (smallest version that passes the five jobs and the success test)**

1. **One screen of four columns (D4), left to right.**
   - A top line shows where you are: `1 sticksandstrings › 2 research › 3 in research › 4 add from library`. Each step is clickable.
   - `→` or Enter goes right. `←` goes back, except that in the library it first closes an open folder. Mouse clicks on any row work too.
   - At 120 columns all four show. At 80 columns three show and slide with the focus.
2. **Column 1, profiles.** `↑↓` chooses one. While the focus is here, column 3 shows everything that profile loads, grouped by kit, with totals (the same data as `ccs loads <profile>`).
3. **Column 2, its kits.** The profile's own kits first, each with `[x]`. Below, under "other kits", every kit not on it, with `[ ]`; space puts one on or takes one off. `n` makes a kit and puts it on this profile at once; the message names the profile, and `u` takes it off.
4. **Column 3, what is in the kit.** Each pick, with the items it gives listed under it, so folder and plugin picks are shown opened up. Space drops a pick. A line above the columns warns when the kit is also on another profile: "a change to it changes that profile too".
5. **Column 4, the library.** All kinds as one tree, folders folded by default (this handles the 76 claude-flow agents). Marks against the open kit:
   - `[x]` picked;
   - `[^]` comes with a picked folder or plugin;
   - `[-]` a folder partly picked, with picked/total;
   - `[ ]` available.
   - A plugin is one box. Opening it shows rows that cannot be toggled.
   - Space on an item that comes with a folder does nothing and says so. There is no folder splitting.
6. **Reading before picking.** The two-line description strip, and `v` for the item's own text.
7. **A key line** that says in plain words what Space and `→` will do on the current row.
8. **`/` search** with the ranking described under "What the prototype showed", and one-level **`u` undo** (the inverse pick, applied only if the file is unchanged on disk).
9. **`l`** opens the same "what it loads" list full screen, with the front-matter names Claude will see and the launch warnings.
7. **Back end.**
   - A pure `load_plan`, split from `prepare_launch`.
   - Per-line kit resolution: extend `resolve_kit`/`pick` with "partly" and "brought by line X".
   - `plugin_contents`.
   - A `front_matter_name` helper.
   - The plugin-name check in `kit_new`/`kit_rename`.
   - Atomic, comment-safe `set_uses`.
8. **Browser additions:** `catalog` (tree plus plugin contents), `load_plan(profile)`, and `plugin_contents` (may fold into `catalog`).
9. **CLI.**
   - New: `ccs loads <profile>`.
   - Extended: `ccs kit show` groups items by pick; `ccs library show plugins/<p>` lists what the plugin holds.
   - Every echo uses name-only item lines.
10. **Contract.** Existing profile actions, import and AWS overlays, `Exit::Shell` and launch stay.

**Non-Goals (deferred; each can return if real use asks for it)**

- D2's matrix in any form, including `ccs kit matrix`, `view_state`, the `G` flat list and two-letter column tags.
- D3's board as an editor, quick-add with "into which kit?", the command palette, the right-click menu, "used by" columns, "unused only", shared-kit copy, `tidy`, and OSC 52 copy.
- D1's 160-column Inspector, the kit bar (the kit picker is enough), hidden sources and their saved setting, near-duplicate `≈` marks, the `usage()` index, copying a kit, `replace_use`, folding into or splitting a folder pick, drag-to-pick, and the `▷` matching cursor.
- Multi-level undo and a saved view state (no new settings file under `$CCS_HOME`).
- Changing the `uses` line format.
- Any change to Claude Code's UI, to the plugins, or to `~/.claude`.

## Key Decisions & Staging for Next Steps

**Decisions to record (one ADR superseding ADR-2610261643 §18-§21)**

1. Item lines stay name-only (`skills/<name>`). The TUI shows the folder only as context.
2. Navigation goes profile → kit → contents → library (D4). You edit one kit at a time. A profile's kits come first in column 2, and every other kit follows, so a kit can be reused. A new kit goes onto the current profile, with the profile named on screen.
3. Picks are written immediately, atomically (temp file and rename) and comment-safe. The file is re-read before each change. Undo is one level and is the inverse pick.
4. The Load view, `ccs loads` and the launcher share one pure `load_plan`. Only launch builds.
5. Plugin contents are counted from the files Claude Code loads, with names read from front matter.
6. Kit names are checked against plugin names when made or renamed, in the use case, not the TUI.
7. Esc behaviour on Home, `Exit::Shell` and the profile page are stated explicitly in the README contract.
8. CLI parity is literal. Any exception (view-only toggles) is listed by name.
9. Search ranks name, then path, then description. `←` leaves a column unless it closes an open folder. Every pickable row can be read in place (strip and `v`).

**Gates (written before code)**

- **G1, keystroke script.** A `tests/tui.rs` test feeds the success-test keys into `State` against a fake Browser loaded with the real data shape (41 skills, 96 agents, 2 plugins, 8 kits, two profiles). It follows D4's path: profile, then `n`, then search and pick, then `←` back to the profile. It asserts the kit's `uses`, the profile's kits, and that column 3 shows the profile's load, at 80x24 and 120x36. The prototype's scripted run is the reference path (33 keys).
- **G1b, search ranking.** `/egm` puts `plugins/egm` first; `/writing` puts `skills/writing/` first.
- **G2, no help screen.** At every step, the next key the script sends must appear in the visible screen text before it is sent. The budget is 40 keys or fewer, and `?` is not allowed.
- **G3, CLI parity.** A table maps each TUI action to its CLI argv. Run both against a temp `CCS_HOME` and compare the resulting files.
- **G4, view equals launch.** The `load_plan` argv equals the `prepare_launch` argv for the same profile, and `load_plan` writes nothing (check that `.build` is unchanged).
- **G5, format round trip.** `uses` read/write keeps comments, rejects `skills/a/b`, and survives a simulated crash mid-write.
- **Guard.** Each gate must fail on zero assertions or zero rows. A vacuous pass is a failed gate.

**Staging**

1. **Slice 0, no UI.**
   - ADR.
   - Atomic, comment-safe `set_uses`.
   - Split `prepare_launch` into `load_plan` and build.
   - `front_matter_name`, `plugin_contents`, the plugin-name check.
   - `ccs loads`, grouped `kit show`, and plugin contents in `library show`.
   - Gates: G4, G5.
2. **Slice 1, read-only views in the current TUI.** The Load view and the load bar. This covers jobs 1-3 and the "one screen" part of the success test.
3. **Slice 2, the column screen (D4).** Four columns, marks, the key line, the description strip and `v`, `n` with name check and attach to the current profile, ranked `/`, `u`. Split `tui.rs` into a `tui/` module. Retire the Home, profile, kit and library pages. Gates: G1, G1b, G2, G3.
4. **Then** run `hexa analyze .` to grade A+. Update the primary README contract and docs in the same change. Record deferred items in a workplan.

**Open risks**

- **Wrong-profile attach.** D4 lowers this risk, because you choose the profile before you reach a kit. The TUI still opens on the default profile, so column 1 must make the current profile obvious, and `n` must name it.
- **80x24.** Everything one profile loads (about 20 rows plus headers) does not fit on one 80x24 screen without folding. The success test should name 120x36, or accept scrolling within one view.
- **Duplicate agent front-matter names already exist** in `claude-flow` (`pr-manager`). The Load view will surface them as warnings on day one. Decide whether that is a warning or a `doctor` fix.
- **`fs::copy` overwrites silently** for agents with the same file name (`fs_kits.rs:50`). This is only reachable through hand edits, but it loses data silently.
- **Test rewrite.** Most of `tests/tui.rs` and the `Fake` Browser change. Until G1-G3 exist, there is no regression net for the new screens.
