# ADR-2610261643: Logins and kits are separate; a library in folders; edit; sync across hosts

**Status:** Accepted
**Date:** 2026-10-26
**Drivers:** User feedback, shared by other users: skills are hard to manage in
one flat list; items cannot be edited from ccs; the stock `~/.claude` should
not be an environment ("we start with nothing until we choose a profile");
profiles must sync across hosts; and "we need to separate login profiles from
assets like skills, agents, CLAUDE.md".
**Supersedes:** the `native` profile of ADR-2610261212, and `shared/` of
ADR-2610261244, ADR-2610261902 and ADR-2610261933 (all replaced by kits);
and ADR-2610261720, copying items between profiles, deleted in phase 4
(added to this line 2026-10-27; ADR-2610261720 already named this ADR).

## Context

Measured on 2026-10-26 with Claude Code's `init` event
(`claude -p … --output-format stream-json --verbose`), in a temp
`CLAUDE_CONFIG_DIR`:

| Probe | Found? |
|---|---|
| `<config>/skills/flat/SKILL.md` | yes |
| `<config>/skills/grp/nested/SKILL.md` | **no** |
| `--plugin-dir A --plugin-dir B` (two plugins, one session) | yes, both; nothing written to the config dir's skills |
| plugin `skills/grp/nested/SKILL.md` | **no** |
| plugin `skills/x` as a symlink to a folder elsewhere | yes |
| plugin `agents/grp/a.md` (real folder) | yes, as `kit:grp:a` |
| plugin `agents/a.md` as a symlink to a file | **no** |
| plugin `agents/grp` as a symlink to a folder | **no** |
| plugin `agents/a.md` as a hard link or copy | yes |

A plugin's skills are named `<plugin>:<skill>`. `claude --help` lists
`--plugin-dir` (repeatable), `--settings` and `--append-system-prompt-file`.

So:

- A login and an asset set can be combined **per session**, with no change to
  the login's config dir. Two terminals can run one login with two kits.
- Folders cannot be shown to Claude Code. They live in a library ccs owns, and
  ccs builds a flat plugin from them.
- A profile today mixes login state (`.claude.json`, history, `projects/`) with
  chosen assets. Only the second can sync, and today nothing separates them.
- `native` points at `~/.claude`, which ccs may not change, so every write verb
  needs a refusal for it. `shared/` links everything into everything, the
  opposite of "start with nothing".

## Decision

### 1. A profile is a login, and nothing else

- `$CCS_HOME/<profile>/profile.env`: provider variables (Bedrock, model, …).
  Synced. No secrets (unchanged rule).
- `$CCS_HOME/<profile>/kits`: the kits it launches with by default, one per
  line. Synced. Empty for a new profile.
- `$CCS_HOME/<profile>/config/`: its `CLAUDE_CONFIG_DIR`. Login state, history,
  caches, its own `settings.json`. Host-local, never synced, and ccs puts no
  skills, agents or CLAUDE.md here.

### 2. A library, in folders

- `$CCS_HOME/library/skills/<folders…>/<skill>/SKILL.md` and
  `$CCS_HOME/library/agents/<folders…>/<agent>.md`. A folder is a directory
  that is not an item. Folders nest.
- An item's **name** is its leaf (`find-nqe-query`). It is unique in the library
  for its kind, because a kit picks items by name and a pick must survive a move
  between folders. Add, move and import refuse a clash and name both paths.

### 3. A kit is a named set of assets

- `$CCS_HOME/kits/<kit>/uses`: library entries, one per line —
  `skills/nqe/` (a folder: all skills under it, now and later),
  `skills/find-nqe-query` (one item), and the same for `agents/`.
- Optional, in the same dir: `CLAUDE.md` (instructions), `.mcp.json` (MCP
  servers, in the plugin format).
- At launch ccs **builds** each kit into `$CCS_HOME/.build/<kit>/`, a plugin:
  `.claude-plugin/plugin.json` named after the kit; `skills/<name>` linked to
  the library folder; `agents/<name>.md` copied (links are not read, per the
  table); `.mcp.json` copied. The build is generated, host-local, and rebuilt
  on every launch, so it cannot drift from the library for longer than a session.

### 4. A launch is a profile plus kits

- `ccs <profile> [+<kit>…] [claude args…]`. `+kit` words replace the profile's
  default kits for this launch.
- ccs runs `claude --plugin-dir .build/<kit>` once per kit, and
  `--append-system-prompt-file` with the kits' `CLAUDE.md` files joined (when
  any kit has one).
- The status line segment names the kits.

**Later (2026-10-27):** every launch also passes `--settings`, with
`ccs statusline --auto` as the status line, before the kit arguments
(ADR-2610261237 §2). Code: `prepare_launch` in `src/usecases/mod.rs`.

### 5. Start with nothing

- There is no `native` profile. ccs reads `~/.claude` only in `ccs import`,
  which copies its skills and agents into `library/imported/` and its CLAUDE.md
  into the kit `imported`, and changes nothing in `~/.claude`.
- ccs sets no default by itself. `ccs` with nothing resolved opens the picker
  on a terminal, and is an error otherwise. It never falls back to `~/.claude`.
- `ccs migrate` (idempotent) converts an existing `$CCS_HOME`: `shared/` items
  go to `library/shared/` and a kit `shared`, added to every profile's `kits`;
  each profile's own items go to `library/<profile>/` and a kit `<profile>`,
  added to that profile's `kits`; the `native` registration becomes the import
  above. Originals go to `.trash/`. `~/.claude` is not changed.

**Note (2026-10-27):** the folder and kit `<profile>` are the kit name made
from the profile name (§8, "Kit names from profile names";
`kit_name_for_profile`), not the profile name as typed. They differ for a
profile name with capitals, `_` or `.`.

### 6. Edit

- `e` in the TUI and `ccs skill|agent edit <name>` open the item's main file in
  `$VISUAL`, else `$EDITOR`, else `vi` (shipped in phase 1). After phase 2 the
  file is the library copy, so the edit reaches every kit that uses the item.

**Replaced (2026-10-27):** `ccs skill|agent edit <name>` was retired with the
rest of `ccs skill|agent …` in phase 4 (§8, "Old commands"). The command line
edit is `ccs library edit <entry>`.

### 7. Sync across hosts with git

**Not built (2026-10-27).** This section is the decision, not the present.
Until phase 5 lands, `ccs sync` is reserved and refuses with "ccs sync is not
built yet", and copying `$CCS_HOME/library/` by hand is the way to move the
library. Before that date, `sync` was not reserved and went to `claude sync`.

- `$CCS_HOME` is a git repo. ccs writes its `.gitignore`: `*/config/`,
  `.build/`, `.trash/`, `.default`. What syncs: `library/`, `kits/`, and each
  profile's `profile.env` and `kits`. What stays: login state, history, the
  default profile.
- `ccs sync init <remote>`; `ccs sync` commits local changes as
  `ccs sync from <host>`, pulls with rebase, pushes. On a conflict it stops and
  names the files; it never resolves one itself.
- Git, because every developer has it and it gives history and undo; a `Sync`
  port hides it.

### 8. Phase 3 and 4 details (decided 2026-10-26, before their code)

- **TUI rows:** profiles (each with its `+kits`), a `kits` heading, each kit,
  then `◆ library`. Tabs depend on the row: a profile has Overview and Kits; a
  kit has Skills, Agents and CLAUDE.md; the library has Skills and Agents.
- **Trees:** Skills and Agents show the library as a tree, folders first in
  path order. On a kit, each row is marked `[x]` (picked), `[·]` (picked
  through a folder above it) or `[ ]`. Space toggles `[x]`. Space on `[·]` names
  the folder that picks it and changes nothing.
- **Keys:** Enter on a profile launches it with its kits; on a profile's Kits
  tab space attaches or detaches. `e` edits an item, or a kit's CLAUDE.md. `v`
  views. On the library: `a` adds from a path into the highlighted folder, `m`
  moves to a typed folder, `n` makes a folder, `x` deletes after y/n. On the
  kits heading or a kit, `n` makes a kit; on a kit, `x` deletes it after y/n.
- **Migration clashes:** names are unique in the library, so an item whose name
  is already taken (a profile's own copy of a shared item, or the same name in
  two profiles) is **not moved**. It stays in the profile's config dir, where
  Claude Code still loads it, and `migrate` and `doctor` name it. Nothing is
  renamed and nothing is merged.
- **Kit names from profile names:** lowercased, `_` and `.` become `-`, and a
  name that still fails gets `p-` in front.
- **Old commands:** `ccs skill|agent|claude-md …` and `ccs link` fail with a
  line naming the new command. `ccs kit edit <kit>` edits a kit's CLAUDE.md.

  **Note (2026-10-27):** `ccs link` names no new command, because none
  replaces it: "kits are built at launch, so there is nothing to link".
- **`ccs new <p> --from <q>`** copies `profile.env`, `kits` and
  `config/settings.json`, never login state.
- **An old layout** (a `shared/` dir, a `native` registration, or ccs links in
  a config dir) makes `list`, `doctor` and `ui` print `run: ccs migrate`.

### 9. Organizing: folder moves keep kits, renames, and bulk work in the TUI (decided 2026-10-26)

- **A folder move or rename rewrites every kit line under it.** A kit names a
  folder by its path (`skills/net/`), so moving `net/` into `ops/` changes each
  `skills/net/…/` line to `skills/ops/net/…/` in the same step. Items are named
  by name and need nothing. (Before this, a folder move left kits pointing at
  nothing; found while planning a real reorganization.)
- `ccs library rename skills/<folder>/ <new-name>` renames a folder in place.
  `ccs kit rename <old> <new>` renames a kit and every profile's `kits` line.
- **TUI, library rows:** space marks rows (`●`); `m`, `x` and `+` act on the
  marked rows, or on the highlighted row when none is marked. `m` opens a folder
  picker: typing filters it, Enter takes the highlighted folder, and text that
  names no folder makes it (`+ new folder`). `+` opens a kit picker (not `k`, which is the vim key for up) the same way
  (`+ new kit`) and adds the rows to that kit. `r` renames a folder. Each row
  shows the kits that pick it.
- **TUI, kit rows:** a fourth tab, Profiles, attaches the kit to profiles with
  space. `p` shows only what the kit picks. `r` on a kit renames it. Tab titles
  count what they hold.

   **Done** (tests: 3 in `tests/kits.rs`, 8 in `tests/tui.rs`). Injected bugs,
   each caught: a folder move or rename leaving kits behind; a kit rename
   forgetting profiles; a folder offered as its own move target; marks kept
   after a move; labels missing picks made through a folder; the `+ new` line
   put first again. That last one was the first design, and the tests caught
   it: typing `nq` then Enter made a folder `nq` instead of taking `net/nqe`.
   Matches now come first. `+` and not `k` adds to a kit, because `k` is up.
   Used on the real library (2026-10-26): 41 skills and 96 agents moved out of
   `imported/` into `network/…`, `forward-dev/`, `horizon/`, `writing/`,
   `methods/`, `agents/network/`, `agents/dev/…`, `agents/claude-flow/…`; kit
   `imported` renamed `base` (it holds the global CLAUDE.md); kits `network`
   (37), `forward-dev` (4), `dev` (19), `writing` (1), `claude-flow` (76).

### 10. Starting ccs chooses a provider and kits (decided 2026-10-26)

- A bare `ccs` (no profile, no `+kit`) on a terminal opens a launch screen: the
  profiles on the left, each with its provider (`bedrock claude-bedrock@us-east-1`,
  `anthropic`), and every kit on the right, with its item count and `[x]`.
- The resolved profile (argument, pin, `$CCS_PROFILE`, default) is
  preselected, and its kits are ticked; moving to another profile ticks that
  profile's kits. The common case is one Enter.
- Space ticks a kit; Enter launches the highlighted profile with the ticked
  kits, for this launch only (as `+kit` does); `s` saves the ticked kits as the
  profile's defaults first. Arguments typed after a bare `ccs` go to `claude`.
- `ccs <profile>` and `ccs <profile> +kit` skip the screen, and so does any run
  without a terminal: scripts keep the resolution order of ADR-2610261212.
- `ccs ui` stays the manager; its Enter launches with the profile's own kits.

### 11. Scrolling and a clearer screen (decided 2026-10-26)

- Every list keeps its scroll position between frames, so moving up one row
  never jumps the view (before: the position was rebuilt each frame).
- Page Up / Page Down move a page; Home / End (and `g` / `G`) go to the first or
  last row; the mouse wheel moves three rows in the focused pane. This applies
  to the manager, the launch screen, and the `v` view.
- A list or text longer than its box shows a scrollbar, and the focused list
  shows its place in the bottom border: `31/66`.
- Folders are bold, descriptions dim, `[x]` green, `[·]` yellow, kit labels
  magenta. Text is unchanged, so nothing depends on colour.
- Mouse capture means a plain drag no longer selects terminal text; most
  terminals select with Shift (or Option on macOS) held.

§10 and §11 **done** (tests: 5 launch-screen and 6 scrolling tests in
`tests/tui.rs`, one in `tests/domain.rs`). Injected bugs, each caught: the
scroll position rebuilt each frame; the wheel moving one row; a launch ignoring
the ticks; ticks not reset on another profile; no scrollbar; page keys not
moving the view. Found while testing: Page Down first moved only the cursor,
leaving the view; it now moves the view a page too, as a pager does; and the
run loop's last restore left mouse capture on, which would have printed every
later mouse move into the shell. Real run (2026-10-26), a 110×24
pseudo-terminal on the user's setup: a bare `ccs` showed `bedrock
claude-bedrock-gary@us-east-1` and six kits; one tick and Enter launched with
`--plugin-dir .build/base --plugin-dir .build/writing` and
`CCS_KITS=base,writing`; the saved defaults did not change; the output turned
mouse capture on (`?1000h`) and off (`?1000l`).

Added 2026-10-27: a list of ticks scrolls to its first ticked row when it is
shown (a profile's kits, a kit's profiles and picks, the launch screen's kits),
up or down, even when that list is not focused. Tab into the list puts the
cursor on a row in view, so the view stays. Tests: 4 in `tests/tui.rs`.
Injected bugs, each caught: no scroll up to a row above the view; the cursor
not kept in view on Tab; no scroll when the row changes.

### 12. The TUI does what the CLI does (decided 2026-10-27)

**Replaced by §18/§19 (2026-10-27):** the keys `c` (copy), `e` (edit
`profile.env`) and `d` (default) on a profile are gone. They are rows on the
profile's page: its variables (Enter edits `profile.env`), "Make it the
default", "Copy to a new profile". On Home, `e` now opens the page. The
`── profiles ──` heading went in §13. What each CLI command maps to still holds.

The user: "in the ui there is no way to create a new profile. everything we
can do at cli should be available in tui". Each CLI command now has a TUI way,
through the `Browser` port, onto the use case the CLI calls:

| CLI | TUI |
|---|---|
| `ccs new <name>` | `n` on `── profiles ──` or a profile |
| `ccs new <name> --from <p>` | `c` on profile `<p>` |
| (edit `profile.env` by hand) | `e` on a profile |
| `ccs import`, `ccs import aws [<p>…] [--all]`, `ccs migrate` | `i`: one list of what can be imported here; migrate only when an old layout is here; results in the pager |
| `ccs default`, `doctor`, `env`, `which`, `list` | `d`; the Overview tab; `*` and `(default)` marks |
| `ccs library …`, `ccs kit …` | §8, §9 |

`ccs statusline` stays CLI only: Claude Code runs it, not a person.
`--region` is not offered: a profile without a region says so, and `e` sets
it.

A `── profiles ──` heading is always the first row, as `── kits ──` is for
kits, so `n` and `i` work before any profile exists. The cursor still starts
on the first profile. Found by the real run, not the tests: with no profiles,
`n` landed on `── kits ──` and made a kit.

`make_profile` (parse, create, seed the status line) moved from the CLI into
the use cases, so `ccs new` and `n` cannot drift. `ImportOutcome` formats
itself, for the same reason.

§12 **done** (tests: 11 in `tests/tui.rs`, one real-`App` test in
`tests/assets.rs` with real files). Injected bugs, each caught: a copy losing
its source; migrate never offered; the new profile not selected. Real run
(2026-10-27), a 130×30 pseudo-terminal, an empty `$CCS_HOME`, a fake `aws` and
a fake editor: `n zed` made a profile; `e` wrote its `profile.env`; `c zed2`
copied the edited variables; `i`, `dev`, Enter made a Bedrock profile with
`AWS_REGION=us-west-2`.

### 13. A menu bar of sections (decided 2026-10-27)

The user, with a screenshot of a 30-column terminal: "i think we need a menu
for a profile than a second vertical for the kits". Asked to choose between a
menu bar of sections and an action menu on a profile, they chose the menu bar.

- The list shows one section at a time: `Profiles │ Kits │ Library`, in a bar
  above it. `1` `2` `3` switch; each section keeps the row it was left on. `←→`
  still switches the body's tabs, as before.
- The Library section lists `skills` and `agents`, one tab each, so every
  section works the same way: a row on the left, its tabs on the right.
- An empty section shows one row, `none yet · n makes one`, so `n` and `i`
  work before anything exists. This replaces the `── profiles ──` and `── kits
  ──` headings of §12.
- Below 70 columns the list sits over the body instead of beside it. Before,
  the body was cut off, and with it every Overview.

§13 **done** (tests: 6 in `tests/tui.rs`; 31 older tests moved from counting
rows to section keys). Injected bugs, each caught: a section forgetting its
row; every section showing profiles; a new kit not selected (missed at first,
test added). A narrow launch-screen test first passed without the change,
because both panes put their first rows on one line; it now compares lines,
and fails without the change.

### 14. A shell in the profile's folder (decided 2026-10-27)

**Replaced by §18/§19 (2026-10-27):** the `t` key and the Overview hint
`Folder: <dir>  t opens a shell here` are gone. The profile's page has a row,
"Open a shell in its folder", which does the same (`Exit::Shell`). The shell
itself is unchanged.

The user: "in the overview add a start in terminal option put me in the
profile directory". The Overview shows `Folder: <dir>  t opens a shell here`.
`t` on a profile ends the TUI and replaces ccs with `$SHELL` (else `/bin/sh`),
started in the profile's folder with the variables a launch sets: the
provider variables cleared and the profile's set, `CLAUDE_CONFIG_DIR`,
`CCS_PROFILE`. `exit` returns to the shell that ran ccs. No kits: `ccs <name>`
launches with them, and the message says so.

`LaunchPlan` gains `dir`, `ProfileStore` gains `folder`, and `shell_plan` is the
use case. The shell goes through the same `ExecLauncher`, made for `$SHELL` in
the composition root.

§14 **done** (tests: one in `tests/profiles.rs`, two in `tests/tui.rs`).
Injected bugs, each caught: the shell not started in the folder; `t` working on
a kit. Real run (2026-10-27), a pseudo-terminal and a fake `$SHELL` that wrote
what it saw: `pwd` was `ccs/zed`, with `CCS_PROFILE=zed`, `AWS_PROFILE=sso` and
`CLAUDE_CONFIG_DIR=ccs/zed/config`. The same run at 30 columns found the cut-off
body that §13's narrow layout fixes.

### 15. Plugins in the library; kits pick them (decided 2026-10-27, before its code)

The user, on google/elephant-goldfish-model, installed as a marketplace plugin
in the stock `~/.claude` only: "we should enable that". A plugin installed
with `claude plugin install` belongs to one config dir, so to one profile.
Kits are the way one thing reaches many profiles.

Measured first (2026-10-27): a plain `git clone` of the repository, passed as
`--plugin-dir`, loads all 7 egm skills and 9 agents, named as the marketplace
install names them (`egm:egm_review`, `egm:egm-design`), version 0.1.0. A
profile with no plugins was used, so nothing came from elsewhere.

- **A third library kind, `plugins`.** `library/plugins/<name>/` is a whole
  plugin: a folder whose `.claude-plugin/plugin.json` names it. `<name>` is
  that manifest name. Plugins have no folders.
- **Adding.** `ccs library add <source>`: a local path is classified as before,
  and a folder with `.claude-plugin/plugin.json` is a plugin, copied in. A
  source that is not a local path but reads as `owner/repo`, an `https://` or
  `git@` URL, or ends in `.git` is cloned with git, then copied in with its
  `.git`. A local path wins over the `owner/repo` reading.
- **Kits.** The entry is `plugins/<name>`, picked like an item. At launch each
  picked plugin's folder is one more `--plugin-dir`, once per launch, after the
  kits' own. Nothing is built or copied at launch.
- **Updating.** `ccs library update [plugins/<name>]` runs `git pull
  --ff-only` in each plugin that is a clone, and says which are not.
- **Removing** is `ccs library rm plugins/<name>`, to the trash, naming the kits
  that used it, as for any entry.
- **The TUI.** Library gets a `plugins` row; a kit gets a Plugins tab after
  Agents, with the same `[x]` ticks; `a` there takes a source, `u` updates.
- **Clash.** A kit and a plugin with the same name both claim the skill prefix
  `<name>:`. The launch warns when one launch loads both.
- **Not done.** A marketplace whose plugins live in subfolders: the clone root
  must be the plugin, else the add says so. `ccs import` does not bring
  marketplace installs from `~/.claude`.
- **No new top-level word.** `ccs plugin …` is not taken, because a bare
  `ccs plugin …` already passes through to `claude plugin …` on the resolved
  profile.

Gate: `cargo test && hexa analyze . --grade A+ --exit-code`, plus a real launch
of a kit holding the egm clone whose init event lists `egm:egm_review`.

§15 **done** (tests: `tests/plugins.rs`, 12, with local git repositories for
GitHub; 3 in `tests/tui.rs`). A `file://` URL also reads as git, for those
tests and for local mirrors. Injected bugs, each caught: a plugin loaded twice;
plugins before the kits; a nested `name` taken for the manifest's; `a/b/c` read
as GitHub; no clash warning; `update plugins/egm` pulling every plugin; a
plugin put in a folder; git asked first for a local `vendor/egm`; `u` on one
plugin updating all. Three of those were missed at first, by tests too weak to
tell the bug from the code (one plugin, one JSON shape, no two-part local
path); the tests were strengthened. Found too: the bug harness took a syntax
error for a missed bug; it now checks that the tests ran. Real run
(2026-10-27): `ccs library add google/elephant-goldfish-model` cloned
`b3a3595`; kit `dev` picked `plugins/egm`; `ccs probe -p …` loaded all 7
`egm:` skills and 9 agents from `library/plugins/egm`, next to `.build/dev`.

### 16. The highlighted kit opens (decided 2026-10-27)

The user: "when selecting a kit i want to expand it". In the two lists that
tick kits, the launch screen and a profile's Kits tab, the kit under the
cursor opens in place: one dim line per `uses` entry with the items it gives
(`skills/network/  31`), `not in the library` for an entry that gives none,
`CLAUDE.md` when the kit has instructions, or `nothing picked yet`. Moving
away closes it. No key: selecting is opening. The lines go below the kit, so
the cursor, space and Enter still act on kits only. Entries, not items: one
kit holds 76 agents.

Tab into such a list now lands on the first ticked kit when it is in view,
else as before; the kit you use opens, and the view does not move. Found by
§11's tests: landing on the top row opened that kit and pushed the ticked kit
out of view.

**Done** (tests: 3 in `tests/tui.rs`). Injected bugs, each caught: the lines
above the kit; a kit open without the focus; no `CLAUDE.md` line; each entry
counted as the whole kit (missed at first, with one-entry kits only; test
added); Tab ignoring the tick; the Kits tab not opening.

### 17. The TUI always names the profile (decided 2026-10-27)

**Replaced by §18/§19 (2026-10-27):** the frame titles `ccs · ⬡ <profile>`,
`profile <name>` and `kit <name>` became §18's breadcrumb
(`ccs › <profile> › kit <name> › library`, with the current profile on its
right), and the footer `⏎ launch …` became §19's `3 · ⏎ start <profile> +kits`.
The goal, the profile always named on screen, still holds.

The user: "it isn't clear what profile we are working on" (asked where: the
TUI). Since §13 the profiles list hides in the Kits and Library sections, and
nothing else named the profile.

- The left frame's title is `ccs · ⬡ <profile>`, the profile a bare `ccs`
  launches (resolved: argument, pin file, `$CCS_PROFILE`, default), in every
  section; `ccs · no profile` when none resolves.
- The right frame's title names what it shows: `profile <name>`, `kit <name>`,
  `library · skills|agents|plugins`, so a tick or an edit plainly acts on it.
- The launch screen's footer starts with what Enter runs, as a command line:
  `⏎ launch claude-bedrock-gary +base`, following the cursor and the ticks.

**Done** (tests: 3 in `tests/tui.rs`). Injected bugs, each caught: a fixed left
title; no title on a kit; the footer without the kits. One older test found its
row by the word `work`, which the new title also holds; it now looks for
`▶ work`. Real run: `ccs ui` showed `ccs · ⬡ claude-bedrock-gary` and
`profile claude-bedrock-gary`, then `kit base` after `2`.

### 18. One start screen; drill in, Esc back (decided 2026-10-27; replaces §13)

The user, of §13's sections: "this 1 2 3 screens is not a workflow this is
still too confusing". Offered a start screen with pages, a task menu, or one
page per profile, they chose the start screen.

- **Home** is the launch screen (§10): profiles and kits; Enter starts Claude.
  `ccs` and `ccs ui` both open it. `e` opens the highlighted profile or kit;
  `n` makes one (a profile from the profile list, a kit from the kit list) and
  opens it; `i` imports; `l` opens the library.
- **A profile's page**: Launch; the kits it starts with (space switches one, Enter
  opens it, the highlighted one shows what it holds); its variables (Enter
  edits `profile.env`); make it the default; copy it; open a shell in its
  folder; then its folder and the doctor's findings.
- **A kit's page**: what it holds, one line per entry with its count (space
  removes one; Enter views an item or opens the library at a folder); "+ Add
  from the library"; its CLAUDE.md (Enter edits); the profiles that use it
  (space switches); rename; delete.
- **The library**, reached from a kit ("+ Add from the library": the tree with
  `[x]` picks for that kit) or from Home (`l`: manage it, with every key of §9).
  `←→` changes skills, agents, plugins.
- **Esc goes back** one page; `q` quits from anywhere. The top line is a
  breadcrumb, `ccs › claude-bedrock-gary › kit network › library`, and on its
  right the current profile, `⬡ claude-bedrock-gary +base`. A page made with
  `n` opens at once, since the next step is always to fill it.
- Opening the library from a kit puts the cursor on the kit's first pick, so
  what it holds is in view (as §16 does on Home).
- An empty kit list still takes the focus, so `n` there makes the first kit.
  Found by the test for that hint: Tab skipped an empty list, so the hint led
  nowhere.
- The menu bar, sections and `1 2 3` are gone. The profile Overview and Kits
  tabs became the profile page; the kit's Skills, Agents, Plugins tabs became
  the library picker; its CLAUDE.md and Profiles tabs became kit-page rows.

§18 **done** (tests: `tests/tui.rs` rewritten around Home and its pages, 52;
the library behaviours of §9, §11, §12 and §15 ported to the new way in).
Injected bugs, each caught: Esc quitting from a page; the breadcrumb without
the path; a new kit not opening; a copy stacking on its source; a deleted kit's
page left open; a renamed kit's page keeping the old name; a profile page that
could not switch a kit off (missed at first: the test only switched one on;
extended); a kit entry space could not remove; Home's ticks not following a
change made on the profile page; the library opening at the top. Real run
(2026-10-27), `ccs` in a 130×30 pseudo-terminal on the user's setup: Home,
`e` → the profile page, Esc, Tab `e` → kit base's page, Enter → the library
picker, Esc Esc → Home, `q`.

### 19. The workflow is on the screen (decided 2026-10-27)

The user, after §18 shipped as 0.3.0: "you did nothing for the workflow". They
were right: Home looked exactly like the old launch screen. The pages of §18
opened only through keys named in a grey footer, so nothing on screen showed
a next step.

- Home's lists are numbered steps: `1 · Choose a profile`, `2 · Choose kits for
  <profile>`, and the footer begins `3 · ⏎ start <profile> +kits`.
- Each list ends with its actions, as rows the cursor reaches and Enter runs:
  `+ New profile`, `⇣ Import (AWS profiles, ~/.claude)`, `◆ Library`, and
  `+ New kit`. With no profiles, the cursor starts on `+ New profile`.
- A profile or kit row ends with `›`; `→` opens its page, `←` (or Esc) comes
  back. In the library `←→` still change the kind, so Esc is its way back.
- An action row keeps the ticks: they belong to the last profile the cursor was
  on, and the footer says what Enter does on the row it is on.

**Done** (tests: 5 more in `tests/tui.rs`). Injected bugs, each caught: Enter on
an action doing nothing; `→` not opening; `←` not going back; the ticks reset
by a visit to an action row (missed at first: no test ticked before the visit;
added). Real run: a 104×18 capture of `ccs` on the user's setup showed the
steps, the five profile rows and actions, six kits and `+ New kit`; `→` opened
the profile page; Tab, Down×4, `→` opened kit network's page.

### 20. The mouse (decided 2026-10-27)

The user: "i still have a hard time navigating with my keyboard, can i use a
mouse". The TUI already captured the mouse for the wheel (§11).

- A click selects a row; a click on the selected row does what Enter does
  there: opens a profile or kit, runs an action, views an item, chooses in a
  picker. On Home a profile opens; it never launches, so a stray double click
  cannot start a session.
- A click on a `[ ]` box ticks it, as space does: a kit on Home, a kit on a
  profile page, a profile on a kit page, a pick or mark in the library.
- A click on `3 · ⏎ start …` launches; on a breadcrumb part, returns to that
  page; on a library kind (`Skills (3)`), shows it.
- A right click goes back one page, and on Home does nothing: it never quits.
- Each frame records where it drew its lists, boxes and buttons; a click asks
  what is there. A list records, per line, its row (or none, for a label or a
  kit's detail lines) and whether the row has a box, and the column its boxes
  start at: 2 with the `▶ ` margin, 0 without, since a list without the cursor
  draws no margin.

**Done** (tests: 8 in `tests/tui.rs`, clicking where the text is drawn).
Found by them: a box sat at column 0, not 2, in a list without the cursor; and
the first columns of every row counted as a box, so "Copy to a new profile"
could not be clicked. Injected bugs, each caught: a second click doing nothing;
boxes ignored; a right click quitting Home; a double click launching; the
breadcrumb ignored. Real run: SGR mouse codes into `ccs` in a pseudo-terminal;
two clicks on `sticksandstrings` opened its page, a right click went back.

### 21. An empty library says what to press (decided 2026-10-27)

The user, on a new host: "i cant see plugins from ccs ui". The library there
was empty, and its plugins tab said "Add one: ◆ library, then a" while already
in the library. The footer said "←→ kind", which does not say that the plugins
are two presses to the right.

- In the library to manage, an empty tab names the step on that page: for
  plugins, "press a, then type owner/repo", with
  `google/elephant-goldfish-model` as the example. In a kit's picker, which has
  no `a`, it points to ◆ Library on the start screen.
- The library footers start with "←→ skills, agents, plugins · esc back", so a
  narrow terminal cuts the rarest keys, not these.

Evidence: three tests in `tests/tui.rs`, each failing before the change. Real
run: the new build in an empty `$HOME`, library, → →, showed the new line.

## Consequences

- Skills from a kit are named `<kit>:<skill>` in Claude Code. A skill invoked
  by hand is `/network:find-nqe-query`. Model-invoked skills are unaffected.
- Agents are copied at build, so an agent edited mid-session takes effect on
  the next launch. Skills are linked, so their edits are live.
- Settings (permissions, hooks, status line) stay with the profile in this
  ADR. A kit-level `settings.json` (merged through `--settings`) is not decided
  here.
  **Clarified (2026-10-27):** this is about settings that come from a kit, which
  are still not decided. A launch does pass `--settings` now, for ccs's own
  status line only (§4 note, ADR-2610261237 §2).
- `shared/`, `unshared`, `share`, `copy` between profiles, and every `native`
  refusal are deleted in phase 4.

## Implementation

1. Edit on the current model: `Editor` port, `process_editor.rs`, TUI `e`,
   `ccs … edit`. **Done** (tests: `edit_*` in `tests/cli.rs`, `tests/tui.rs`,
   `tests/assets.rs`; a pseudo-terminal run edited a skill and quit with 0).
   **Note (2026-10-27):** only `tests/cli.rs` still has `edit_*` tests
   (`edit_opens_the_item_in_visual_with_its_arguments_and_saves_in_place`,
   `edit_falls_back_to_editor_and_edits_an_agent`). The TUI and port tests
   were rewritten with the screen (§18); edits are covered there by
   `e_and_v_act_on_items_not_folders`,
   `the_kit_page_edits_claude_md_switches_profiles_renames_and_deletes`
   (`tests/tui.rs`) and
   `the_app_makes_copies_edits_and_imports_profiles_through_the_port`
   (`tests/assets.rs`).
2. Library and kits: `Library` and `KitStore` ports; `ccs library …`,
   `ccs kit new|list|show|use|drop|rm`; build; `ccs <profile> +kit` launch.
   **Done** (tests: `tests/kits.rs`, 17; with §9's 3, 20. **Note (2026-10-27):**
   the file has 21; the extra one is
   `a_launch_sets_the_ccs_status_line_above_any_projects`, from ADR-2610261237
   §2). Injected bugs, each caught: agents
   linked instead of copied; a folder matching `network/` for `net/`; a stale
   build kept (caught only after the test was fixed to build the kit first);
   a duplicate name allowed. Real run (2026-10-26): a throwaway `$CCS_HOME`, a
   Bedrock profile, kits `net` (a skill in `net/nqe/`, an agent in `hex/`) and
   `eng` (a skill, and a CLAUDE.md rule "end every answer with banana");
   `ccs work +net +eng -p …` loaded `net:find-nqe-query`,
   `eng:elephant-goldfish-method`, `net:probe-agent`, and answered `ok banana`.
3. TUI: rows for profiles and kits, then `◆ library`; a kit shows the library
   tree with `[x]`; space toggles; `m` moves; `e` edits. **Done** (tests:
   `tests/tui.rs`, 22, against a fake `Browser`; the real `App` behind the port
   in `tests/assets.rs`).
4. Start with nothing: delete `native` and `shared/`; `ccs import` into the
   library; `ccs migrate`; bare `ccs` opens the picker. **Done** (tests:
   `tests/migrate.rs`, 14, on a hand-built copy of the old layout).

   Injected bugs, each caught: an inherited `[·]` drawn as `[ ]`; space on `[·]`
   writing a line; `a` ignoring the highlighted folder; any key confirming a
   delete; search dropping parent folders; migrated originals left behind; the
   shared kit not attached; the `native` default kept; `import` retiring
   `~/.claude` files; leftover clashes counted as an old layout.

   Real run (2026-10-26), first on a copy of the user's `~/.claude-profiles`,
   then on the real one: 40 skills and 96 agents from `~/.claude` into
   `library/…/imported/`; 8 agents whose names `~/.claude` holds twice were
   named "not imported" (found here, not by the tests: two tests were added);
   an empty `shared/` made an empty kit until fixed (also found here; test
   added); `~/.claude` and the Bedrock profile's files hashed the same before
   and after. In a 120×30 pseudo-terminal a bare `ccs` opened the picker,
   space edited a kit's `uses` on disk, and Enter launched with
   `--plugin-dir .build/imported` and `CCS_KITS=imported`. Also found: `ccs …
   | head` panicked on a closed pipe (an older bug); `main.rs` now restores the
   default SIGPIPE action (test added).
5. `ccs sync` through a `Sync` port and a git adapter. **Not built.** `ccs sync` refuses (test
   `sync_is_reserved_and_says_it_is_not_built`); draft workplan
   `draft-2610261805-ccs-phase-5-ccs-sync`.

Gate for every phase: `cargo test && hexa analyze . --grade A+ --exit-code`,
plus a run of the real binary in a throwaway `$HOME`. Phase 2 adds a launch
whose `claude` is a script that prints its arguments, and one real
`claude -p` probe like the table above.

## References

- ADR-2610261212, ADR-2610261244, ADR-2610261255, ADR-2610261720,
  ADR-2610261902, ADR-2610261933
