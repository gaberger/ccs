# ADR-2610281305: The TUI becomes four columns, profile first

**Status:** Accepted
**Date:** 2026-10-28
**Drivers:** The user: "the TUI is confusing, it's hard to see what skills,
agents and plugins are in a kit, it's hard to select what I want, it's hard to
know what is available, it's hard to create new kits, or understand what is in
the library." Then, after trying prototypes: "it was harder to navigate around
profiles", "could not read the skill so not sure what was in it", and "you
should select the profile first then select a kit".
**Supersedes:** ADR-2610261643 §18-§21 (the TUI pages), and the page layout of
ADR-2610261255. Both stay in force for everything else.

## Context

The TUI fails at seven jobs:

1.  Show what a kit holds.
2.  Show what the library holds, and what is inside each entry.
3.  Show what is available but not yet picked.
4.  Pick and unpick items with no guessing.
5.  Make a new kit.
6.  Move between profiles.
7.  Read an item before picking it.

The first five are the user's report. The last two came from user testing.

**How the design was reached (the "diverge" step).**
- Three designers each proposed a design: `tui_designs/design_1_dual_pane_builder.md`, `design_2_catalog_matrix.md` and `design_3_profile_first_board.md`.
- Six independent reviewers read all three against the code.
- The user then tried all of them, and two revisions, in a working prototype, `tui_designs/prototype.html`. It runs on a snapshot of the real library.
- The record of that work is `docs/designs/ccs-tui-profile-first-columns-brief.md`.

The reviewers chose a kit-centred builder. The user's testing overruled that choice: profiles were hard to reach in it.

**Three faults in today's code block any new screen.** The reviewers found them, and I checked each one against the code.

- **B1. Item lines.** All three designers wrote item lines such as `skills/writing/humanize-writing`. `Use::parse` (`src/domain/mod.rs:413`) refuses them. An item line is `skills/<name>`. Only a folder line, ending in `/`, carries a path. A screen that echoes the designers' form teaches CLI commands that fail.
- **B2. No preview without a build.** `prepare_launch` (`src/usecases/mod.rs:69`, `:84`) rebuilds `$CCS_HOME/.build/<kit>` and writes the prompt file while it plans. A screen that shows "what this profile loads" through it would rebuild kits on every cursor move.
- **B3. Unsafe kit saves.** `FsKitStore::set_uses` (`src/adapters/secondary/fs_kits.rs:121`) writes the `uses` file in one plain `fs::write`. It is not atomic, and it drops `#` comments. A screen that saves on every pick makes both more likely to bite.

**Faults the prototype found that no reviewer saw.**

- **Substring search lists the wrong match first.** `egm` listed the skill `plan-segmentation-check` first ("s-egm-entation"). `writing` listed `validate-nqe-query`, because its description mentions writing.
- **`←` climbed the folder tree before it left a column.** Getting back to the profile took many presses, and the screen gave no reason.

## Decision

### 1. One screen of four columns, left to right

```text
 1 PROFILE         │ 2 KITS FOR p        │ 3 IN kit / p LOADS   │ 4 ADD FROM LIBRARY
 ● sticksandstrings│ [x] research        │ ◆ plugins/egm        │ ▾ SKILLS
 ○ claude-bedrock… │ + new kit for p     │   egm_draft, …       │   ▸ writing/   2/2
 + new profile     │ OTHER KITS          │ ▣ skills/writing/    │ ▾ PLUGINS
 import …          │ [ ] base  [ ] dev … │   humanize-writing … │   [x] egm
```

- **Top line.** It names where you are: `1 sticksandstrings › 2 research › 3 in research › 4 add from library`. Each step is a click target.
- **Moving.** `→` (or Enter, except in column 1) moves right. `←` moves left. In column 4, `←` closes an open folder first. Below that, it always leaves the column.
- **Column 1, profiles.** The screen opens on the profile `ccs` would launch with no name: the pin file, then `$CCS_PROFILE`, then the default. While the cursor is on a profile, column 3 shows everything that profile loads, grouped by kit, with totals.
  - Enter launches that profile (`Exit::Launch`), as Enter on Home does today.
  - `e` edits `profile.env`, `d` makes it the default, `c` copies it, and `!` opens its shell (`Exit::Shell`).
  - Rows under the profile list: "+ new profile (n)" and "import (i)". The import row opens one list: migrate an old layout (first, when there is one), `~/.claude`, each AWS profile, and every AWS profile.
- **Column 2, kits.** The profile's kits come first, in launch order, each with `[x]`. Every other kit follows under "other kits", with `[ ]`.
  - Space attaches or detaches a kit.
  - `n` makes a kit, attaches it to this profile, and names the profile in the message.
  - `e` edits the kit's instructions. `r` renames it, and `x` deletes it after a confirmation.
- **Column 3, in the kit.** Each `uses` line, with the items it gives listed under it. The first row is "instructions (CLAUDE.md)", and `e` opens it.
  - Space drops a line.
  - When the kit is on another profile, a line above the columns says so: "a change to it changes that profile too".
- **Column 4, the library.** All kinds, as one tree. Folders are folded by default, so the 76 claude-flow agents are one row. Marks against the open kit:
  - `[x]` picked;
  - `[^]` comes with a picked folder or plugin;
  - `[-]` a folder partly picked, with picked/total;
  - `[ ]` available.
  - A plugin is one switch. Opening it shows rows that cannot be toggled.
  - Space picks or unpicks.
  - `/` searches, `v` reads the item, and `e a n m r x u` keep their meanings from today's library page.
- **Reading.** A two-line strip under column 4 always shows the full description of the current row. `v` opens the item's own text: for a plugin, its skills and agents; for a folder, its items and their descriptions.
- **Search order.** An exact name comes first, then a name that starts with the text, then a name that contains it, then the path, then the description. At an equal rank, plugins come before folders, and folders before items.
- **Messages.** After each change, the key line shows the `ccs` command that has the same effect. `z` undoes one step. `u` keeps its meaning: update plugins.
- **Quitting.** `q` and Ctrl-C quit. Esc goes back one column, and quits only from column 1.
- **Widths.** At 110 columns or more, all four columns show. From 70 to 109, three show and slide with the focus. Below 70, one column shows at a time. At 80×24, column 3 scrolls, and its totals line stays fixed.

### 2. Rules behind the screen

- **R1. Item lines stay name-only.** The screen shows the folder as context only. This fixes B1.
- **R2. One source for "what a profile loads".** `load_plan` is a pure value. Column 3, the new `ccs loads <profile>`, and the launch all read it. Only `prepare_launch` builds: it runs `load_plan`, then the build. This fixes B2.
- **R3. Picks are written at once, through the CLI's own use cases.** A pick runs `kit_use` or `kit_drop`, which re-read the file before each change. `set_uses` writes a temporary file and renames it over `uses`, and a save that cannot be made that way is refused. Comments and blank lines stay where they are. A line still picked stays where it is; a line that changed (a renamed folder) takes the place of the one it replaced; a new pick goes at the end. This fixes B3.
- **R4. A kit cannot take a library plugin's name.** `kit_new` and `kit_rename` check, so the CLI and the TUI share the rule.
- **R5. Plugin contents come from the files Claude Code loads.** That is `skills/*/SKILL.md` and `agents/*.md`. Each name is read from front matter, because that name, not the file name, is the one Claude Code shows.
- **R6. Duplicate agent names are a warning.** It shows in column 3's load list and in `ccs doctor`. Nothing fixes it automatically. Two claude-flow agents both declare `name: pr-manager` today.
- **R7. Nothing that works today is lost.** That covers launch, a profile's shell, `profile.env`, new profile, both imports, migrate, and kit instructions. ccs never writes `~/.claude`.

### 3. Not in this decision

- A grid of kits against items.
- A board that edits several kits.
- A command palette, or a right-click menu.
- Hiding a library source.
- Near-duplicate marks.
- Splitting or merging folder picks.
- Copying a kit.
- More than one step of undo.
- Saved screen state.
- A change to the `uses` format.
- Any change to Claude Code, the plugins, or `~/.claude`.

The rejected designs, and why each was rejected, are in the brief. Briefly:

- **The builder** hid profiles.
- **The grid** let a pick land in the wrong kit.
- **The board** showed one profile at a time, and edited shared kits in place.
- **The rail** mixed profiles and kits in one list.
- **Changing the `uses` format** would cost a migration, and break picks when items move between folders.

## Consequences

- **The page-based TUI goes.** The Home, profile, kit and library pages are removed.
  - About a third of `src/adapters/primary/tui.rs` survives: `Key`, `map_key`, `Exit`, `Scroll`, `render_list`, `Hit`, the terminal loop, and the import and AWS overlays. Those parts move into `src/adapters/primary/tui/`, unchanged.
  - Most of `tests/tui.rs` is rewritten.
- **Enter means two things.** It launches in column 1 and moves right elsewhere. The key line names it on every profile row. This is the decision most likely to surprise.
- **Two writers no longer collide** (changed by `hexa harden`, see "What hardening changed"). The ADR first accepted that two saves of one kit at the same moment could lose one change. Hardening found it a real bug and fixed it: `set_uses` locks the kit's folder and merges a concurrent save.
- **The duplicate `pr-manager` agent shows on day one.** It appears as a warning on any profile that loads kit `claude-flow`.
- **Moving the cursor never writes.** `load_plan` runs on each move to another profile and reads a few hundred small files. That is acceptable at 137 items. If it is ever slow, read the library once per refresh. Do not add a cache.
- **Rollback is safe.** Each slice is a release. `ccs upgrade <old version>` rolls back, and no file format changes.

## Implementation

Two slices, each its own release and its own `hexa loop`.

Each gate is written and seen to fail before the code it checks. Every test-based gate fails on zero tests. "0 passed" is a failed gate.

### Slice 0: behind the screen (no screen changes)

- **Domain** (`src/domain/mod.rs`):
  - `LineGives` and `resolve_lines`: for each kit line, the items it gives. `resolve_kit` stays as it is (the launch depends on its order: library order, each item once); both share one matching rule, `picks`.
  - `LibItem.shown_name` and `front_matter_name`.
  - `LoadPlan` and `load_plan`, with warnings for a missing line, a kit and a plugin with one name, and two agents with one shown name.
  - `Pick::Partly`, and `rank`.
  - `Command::Loads`, with `loads` added to `RESERVED` and a line in `HELP`.
- **Ports** (`src/ports/mod.rs`):
  - `KitStore`: `build_dir` and `prompt_path` (paths only), and the new `set_uses` contract.
  - `Library::plugin_contents`, returning `PluginContents`.
  - `Browser`: `load_plan`, `kit_lines`, `plugin_contents`.
  - Re-export the new types.
- **Use cases** (`src/usecases/mod.rs`):
  - `load_plan`.
  - `planned_launch`: the launch `prepare_launch` would make, with nothing built. It exists so G4 can compare the two.
  - `prepare_launch` becomes `load_plan`, then build, then the prompt. The paths it names come from `KitStore::build_dir` and `prompt_path`, not from what `build` returns, so G4 also checks that every path the launch names was written.
  - `loads`, to format the plan.
  - `kit_new` and `kit_rename` refuse a plugin's name.
  - `kit_show` returns lines with the items they give, and `library_show` lists a plugin's contents.
  - `doctor` warns about duplicate agent names.
  - The `App` delegates for the new `Browser` methods.
  - The old TUI treats `Pick::Partly` as `Pick::No`.
- **Adapters:**
  - `fs_kits.rs`: atomic, comment-keeping `set_uses`; `build_dir`; `prompt_path`.
  - `fs_library.rs`: `shown_name` and `plugin_contents`.
- **Composition root** (`src/lib.rs`): `ccs loads`, the grouped `kit show`, and `library show plugins/<name>`.
- **Docs:** the domain, ports, usecases and secondary-adapter READMEs; `README.md`; `docs/architecture.md`.
- **Gate checks, each red first:**
  - **G4**, in `tests/loads.rs` (new): the launch arguments from `load_plan` equal those from `prepare_launch`, and `.build` is byte-for-byte unchanged after `load_plan`. `ccs loads` prints each kit, line, item, total and warning.
  - **G5**, in `tests/kits.rs`: comments and blank lines survive a save, `skills/a/b` is refused, and a failed write leaves the old file.
  - **G8**, in `tests/kits.rs`: `kit new egm` and `kit rename x egm` are refused while plugin `egm` exists.
  - **Plugin contents**, in `tests/plugins.rs`: one fixture agent has a front-matter name that differs from its file name.
  - **Ranking**, in `tests/domain.rs`: `rank` puts `plugins/egm` first for `egm` and `skills/writing/` first for `writing`.
- **The gate command** (recorded with `hexa loop gate`):

  ```sh
  cargo test 2>&1 | tee /tmp/ccs-test.log >/dev/null; \
  awk '/^test result:/ {p+=$4; f+=$6} END {exit (p == 0 || f > 0)}' /tmp/ccs-test.log && \
  cargo test --test loads 2>&1 | grep -Eq 'test result: ok\. [1-9]' && \
  cargo clippy --all-targets -- -D warnings && \
  hexa analyze . --grade A+ --exit-code
  ```

  It fails today, because `tests/loads.rs` does not exist yet.
- **Harden:** `hexa harden` on `src/usecases/mod.rs` and `src/adapters/secondary/fs_kits.rs`, under the same gate.

### Slice 1: the column screen

- **Remove `src/adapters/primary/tui.rs`.** First run `hexa graph consumers` on it, and grep for `adapters::primary::tui`. The module path stays the same, so `src/lib.rs` and the tests keep their imports.
- **Create `src/adapters/primary/tui/`:**
  - `mod.rs`: the loop, `Key`, `Exit`, and the editor handoff, moved unchanged.
  - `columns.rs`: `State` and the keys for each column.
  - `draw.rs`: the top line, the columns, the description strip, and the key line.
  - `overlays.rs`: text input, confirm, the `v` reader, the imports, and migrate.
  - `widgets.rs`: `Scroll`, `render_list`, `Hit`, moved unchanged.
- **`tests/tui.rs`.** The `Fake` Browser gains the new methods and a fixture shaped like the real data: 41 skills, 96 agents, `egm` holding 7 skills and 9 agents, 8 kits, and 2 profiles. Tests for the removed pages go. Gates:
  - **G1:** the success test at 120×36 and at 80×24. The script makes kit `research`, puts plugin `egm` and `skills/writing/` in it, attaches it to `sticksandstrings`, and goes back to the profile's load list, in 40 keys or fewer.
  - **G1b:** `/egm` and `/writing` each put the plugin or the folder first.
  - **G2:** each key the script sends is visible on screen before it is sent.
  - Column behaviour: Enter launches only in column 1; `←` leaves column 4; `n` attaches the new kit and names the profile; the "also on" warning; `z` undoes; the instructions row.
- **G3**, in `tests/parity.rs` (new). For each TUI change, take the `ccs` command the TUI *displayed*, run it against a second temporary `$CCS_HOME`, and compare the two trees file by file. This catches a displayed command that ccs refuses, which is B1 in another form.
- **`tests/boundaries.rs`** checks every file under `src/adapters/primary/`, and fails if it finds none.
- **Docs:** the primary adapter README, with the columns, the keys, the exits, and "Esc quits only from column 1"; `README.md`; `docs/architecture.md`; `src/README.md`; `src/adapters/README.md`.
- **See it work.** Run `ccs` in a real terminal at 120×36 and at 80×24, and do the success test by hand, with the pty grid replay. "It compiles" is not "it works".
- **Harden:** `hexa harden src/adapters/primary/tui/`.
- **When slice 1 ships:** `hexa adr accept`, then `hexa adr complete` once its loop is done. Supersede ADR-2610261643 §18-§21 and ADR-2610261255's page layout with `hexa adr supersede`.

### What hardening changed (2026-10-28)

`hexa harden` ran on `src/adapters/secondary/fs_kits.rs` and `src/usecases/mod.rs` under the slice 0 gate. It confirmed 29 bugs (11 and 18), refuted 4, and left 1 unfixed. Each fix came with a test (24 new tests; 261 in all). Two of its fixes go past this ADR's first scope, and are kept because each is a confirmed bug with a test:

- **Saving a kit.** `set_uses` takes a lock on the kit's folder (`std::fs::File::lock`, so Rust 1.89+). It merges a save with any change another process made since this one last read the kit: this save's order wins; what the other dropped stays dropped; what it added stays. The "last read" is a per-process map in `FsKitStore` (`seen`). It is a merge base, not a cache that answers reads: every read goes to the file. A `uses` file that cannot be read is an error, not an empty kit.
- **Building a kit.** `build` assembles into a staging folder and swaps it with the live one in one call (`renamex_np` on macOS, `renameat2` on Linux), so a running session never loses its plugin and two launches that build at once both succeed. `write_prompt` writes a temporary file and renames it. Two picked agents with one file name fail the build and name both sources; this was `draft-2610281257`, now done.
- **Use cases that changed several files** now finish whole or put things back: `kit rename`, `kit rm`, folder move and rename, `import aws`, and `migrate`. `doctor` reports an unreadable kit as a failure and checks the rest. `kit drop` accepts an entry named twice. `import` adds folder lines only for kinds it copied.
- **Left unfixed:** `library_remove` ignores unreadable kits when it reports which kits used an item; its fix did not pass the gate. Recorded as a workplan draft.

Slice 1 (2026-10-28): `hexa harden src/adapters/primary/tui` confirmed 9 findings, 5 distinct, each fixed with a test under the slice 1 gate. Column 1 now holds the chosen profile by name, so an import that adds a profile sorting earlier cannot move the selection to another profile. A click in columns 3 and 4 acts on the kit the screen shows. Import-list clicks allow for scrolling. Leaving a search reopens PLUGINS to keep the cursor inside a plugin. Making a kit is two writes; when the second fails, column 2 still shows the kit. One change was made by hand after review: harden showed that partial failure with a `✓` and the failure inside the command list; it now reads as a failure, and a test checks that.

Seen working (2026-10-28): the success test, run on the real `ccs` binary in a pseudo-terminal on a copy of the real library (no logins copied), at 120×36 and at 80×24. Everything `sticksandstrings` loads fits on one screen at both sizes; the terminal is restored on exit.

## References

- `docs/designs/ccs-tui-profile-first-columns-brief.md`: the designers, the review, the code checks, and the user's test notes.
- The prototype: all designs on a snapshot of the real library, with the timed success test, published at https://claude.ai/artifact/Gvs3AtKNVPEoCaKm8ozQtE. Its built page embeds the library's text, so it is not in git: `tui_designs/prototype_snapshot.py` takes the snapshot and `tui_designs/prototype.template.html` is the page to put it in.
- `tui_designs/design_1_dual_pane_builder.md`, `design_2_catalog_matrix.md`, `design_3_profile_first_board.md`: the three designer proposals.
- ADR-2610261643 (logins, kits, library), ADR-2610261255 (the ratatui TUI and the `Browser` port), and ADR-2610271000 (releases, for rolling back).
