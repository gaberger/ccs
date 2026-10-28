# ccs

Switch Claude Code between logins — a personal claude.ai account, AWS Bedrock accounts — and choose which skills, agents and instructions each session gets.

<a href="https://github.com/gaberger/hexa"><img src="https://raw.githubusercontent.com/gaberger/hexa/main/.github/assets/badges/built-with-hexa.svg" alt="built with hexa" height="20"></a>

## What This Does

ccs keeps two things apart:

- A **profile** is a login: its provider variables (Bedrock, model, region) and
  its own Claude Code config directory, which holds login state, history
  and the `settings.json` that ccs seeds for the status line.
- A **kit** is a named set of skills and agents from your **library**, plus
  optional instructions (a CLAUDE.md).

A launch is a profile plus kits: `ccs work +net`. ccs builds each kit as a
Claude Code plugin and starts `claude --plugin-dir …`, for that one process
only. Nothing is written into the login's config directory, so one login can
run with different kits in two terminals. ccs never runs on, or writes to,
your stock `~/.claude`. The decisions are in
[ADR-2610261212](docs/adrs/ADR-2610261212-profile-per-config-directory.md) and
[ADR-2610261643](docs/adrs/ADR-2610261643-logins-kits-library-sync.md).

## Prerequisites

- Claude Code (`claude` on your PATH), with `--plugin-dir`
- macOS (Apple silicon or Intel) or Linux (x86_64) for a release; any Unix from source
- From a release: the GitHub CLI, `gh`, logged in (the repository is private)
- From source: Rust 1.88+ (`cargo`), because `ratatui` 0.30, the one dependency, declares it

## Install

From a release, with no Rust needed:

```bash
gh release download --repo gaberger/ccs -p "ccs-*-$(uname -m | sed s/arm64/aarch64/)-$( [ "$(uname)" = Darwin ] && echo apple-darwin || echo unknown-linux-gnu).tar.gz" -O - | tar xz -C ~/.local/bin ccs
ccs version
```

Then `ccs upgrade` keeps it current: it installs the latest release over itself,
after checking the archive against the release's `SHA256SUMS.txt` and running
the new binary once. `ccs upgrade --check` only says what is available;
`ccs upgrade 0.1.0` installs that version. A tag `vX.Y.Z` builds and publishes
a release; see [ADR-2610271000](docs/adrs/ADR-2610271000-release-and-upgrade.md).

From source: `./start.sh --install`.

## Quick Start

```bash
./start.sh --install                 # build, copy ccs to ~/.local/bin
ccs import aws claude-bedrock        # a login per AWS profile (or: ccs new work)
ccs import                           # copy ~/.claude's skills and agents into the library, kit 'imported'
ccs kit new net                      # a kit
ccs kit use net skills/imported/     # what it holds: a folder, or single items
ccs kit attach net claude-bedrock    # launch with it by default
ccs claude-bedrock                   # Claude Code, on Bedrock, with kit net
```

Already used an earlier ccs, with `shared/` and a `native` profile? Run
`ccs migrate` once. `ccs list` says so when it is needed.

A Bedrock `profile.env`:

```sh
CLAUDE_CODE_USE_BEDROCK=1
AWS_PROFILE=work-sso
AWS_REGION=us-east-1
ANTHROPIC_MODEL=us.anthropic.claude-opus-5-5-v1:0
```

The file is parsed as `KEY=VALUE` lines and never run by a shell. Keep secrets
out of it: AWS credentials come from `~/.aws` or SSO, and a claude.ai login comes
from `/login` inside the profile.

## Usage

```
ccs [claude args…]             Home: choose a profile and kits, then ⏎; e opens a page to change them
ccs <profile> [+kit…] [args…]  start claude with <profile>; +kit replaces its kits for this run
ccs list                       list profiles with their kits (* = resolved)
ccs new <name> [--from <p>]    make a profile (copies variables, kits, settings; never login)
ccs default [<name>]           show or set the default profile
ccs env [<name>]               print exports: eval "$(ccs env work)"
ccs doctor [<name>]            check a profile and its kits; exits 1 on a failure
ccs which                      print the resolved profile
ccs ui                         the same Home, from anywhere
ccs statusline [-- <cmd…>]     status line segment; wraps an existing status line command
ccs statusline --auto          what every launch sets: the segment, then the project's or profile's own status line
ccs import                     copy ~/.claude skills, agents, CLAUDE.md into the library (kit 'imported')
ccs import aws [<p>…] [--all] [--region <r>]
                               list AWS CLI profiles, or import them as Bedrock profiles
ccs migrate                    convert the shared/ and native layout of earlier versions
ccs upgrade [--check | <ver>]  install the latest release (or <ver>) over this ccs
ccs version                    print the version
ccs sync                       reserved; not built yet, so it refuses and says what to copy instead

ccs library list [skills|agents|plugins] | add <path|owner/repo|git URL> [--in <folder>] | mkdir skills/<folder>
            mv <entry> <folder|/> | rename skills/<folder>/ <name> | rm <entry> | show <entry> | edit <entry>
            update [plugins/<name>]
ccs kit     list | new <k> | rm <k> | show <k> | edit <k> | rename <k> <new> | use <k> <entry>… | drop <k> <entry>…
            attach <k> <profile> | detach <k> <profile>
```

An **entry** is `skills/<name>`, `agents/<name>`, `plugins/<name>`, or a folder
such as `skills/net/` (everything under it, including what you add later).

**The launch screen.** A bare `ccs` in a terminal shows your profiles with
their provider (`bedrock claude-bedrock@us-east-1`, `anthropic`) and every kit
with its item count. The resolved profile is preselected and its kits ticked,
so one Enter starts it as usual. Space ticks other kits for this launch; `s`
saves the ticks as that profile's default. `ccs <profile>` (with or without
`+kit`) skips the screen, and so does anything run without a terminal.

**Which profile runs.** The first of: the name you type, a `.claude-profile`
file in the current directory or a parent, `$CCS_PROFILE`, the default. ccs
sets no default by itself. With none of these, a bare `ccs` opens the picker
in a terminal and is an error anywhere else; it never falls back to
`~/.claude`.

**The library.** Every skill and agent lives once, in
`~/.claude-profiles/library/`, in folders you choose:

```bash
ccs library add ./find-nqe-query --in net/nqe   # a skill folder, or an agent .md file
ccs library mkdir skills/writing
ccs library mv skills/humanize writing          # / is the top; a moved folder's kits follow it
ccs library rename skills/net/nqe/ queries      # a folder, in place; kits follow it
ccs library edit skills/find-nqe-query          # $VISUAL, else $EDITOR, else vi
ccs library rm skills/old-thing                 # to ~/.claude-profiles/.trash; names the kits that used it
```

A name is unique in the library, whatever its folder, because Claude Code
knows a skill by its name and a kit picks items by name: moving an item
between folders changes no kit.

**Plugins.** A whole Claude Code plugin can live in the library too, so a kit
can bring it to every profile instead of installing it into one:

```bash
ccs library add google/elephant-goldfish-model   # cloned from GitHub; any git URL or local folder works
ccs kit use dev plugins/egm                      # every profile with kit dev gets egm:egm_review, …
ccs library update                               # git pull every plugin (or: update plugins/egm)
```

A plugin keeps its manifest name (`egm`, not the repository name) and its
skill prefix (`egm:`). It is loaded as it is, with one more `--plugin-dir`;
nothing is built. A local path wins over reading the source as `owner/repo`.
The repository root must be the plugin: a marketplace whose plugins live in
subfolders is not supported yet.

**Kits.** `ccs kit use net skills/net/ agents/rev` picks from the library;
`ccs kit edit net` writes the kit's instructions. At launch, each kit is
rebuilt into `~/.claude-profiles/.build/<kit>/` as a plugin: skills are links
into the library (so edits are live), agents are copies (Claude Code does not
read an agent through a link, so an agent edit applies from the next launch),
and the instructions are appended to the system prompt. In Claude Code a kit's
skills are named `<kit>:<skill>`, for example `/net:find-nqe-query`.

**The TUI.** `ccs` (or `ccs ui`) opens Home, the launch screen. Every other
screen is a page you open from it, and Esc goes back. The top line always says
where you are and which profile is current:
`ccs › claude-bedrock-gary › kit network › library        ⬡ claude-bedrock-gary +base`.

```
Home           1 · Choose a profile, 2 · Choose kits, 3 · ⏎ start; space ticks a kit · tab switches lists
               each list ends with its actions: + New profile · ⇣ Import · ◆ Library · + New kit
               → (or e) opens the profile or kit · s saves the ticks as its default
               n new profile (new kit on the kit list) · i import · l the library
a profile      ⏎ Launch · its kits (space adds or removes; the highlighted one shows what it holds;
               ⏎ opens it) · Variables (⏎ edits profile.env) · default · copy · shell · folder, doctor
a kit          what it holds, one line per entry with its count (space removes; ⏎ views or opens)
               + Add from the library · CLAUDE.md (⏎ edits) · Used by (space switches) · rename · delete
the library    from a kit: the tree with [x] picks for it (space picks, p picked only)
               from Home (l): manage it: space mark · m move · + to kit · a add · n folder
               r rename · x delete · e edit · v view · u update plugins
everywhere     ←→ skills, agents, plugins (in the library) · ↑↓ (or k j) PgUp/PgDn Home/End (or g G) · / search
               ← or esc back (esc in the library) · q or Ctrl-C quit
the mouse      click a row to select it, click it again to open or run it · click a [ ] box to tick it
               click 3 · ⏎ start to launch · click the breadcrumb to go back · right click: back · wheel scrolls
```

With the mouse on, a plain drag no longer selects terminal text; hold Shift (or
Option in some macOS terminals) to select.

See [ADR-2610261643 §18](docs/adrs/ADR-2610261643-logins-kits-library-sync.md).

**MCP servers.** Per profile: `ccs bedrock-work mcp add -s user forward -- forward-mcp`.
Per kit: put a plugin-format `.mcp.json` in `~/.claude-profiles/kits/<kit>/`.

**AWS profiles.** `ccs import aws` lists your AWS CLI profiles and changes
nothing. Name the ones that are Bedrock accounts to import them:

```bash
ccs import aws                                         # list; → marks imported ones
ccs import aws claude-bedrock claude-bedrock-gary      # one ccs profile each
ccs import aws --all --region us-east-1                # every profile; region only where AWS has none
```

Each gets `CLAUDE_CODE_USE_BEDROCK=1`, its `AWS_PROFILE`, and the region from
your AWS config. The AWS profile `default` becomes `aws-default`. Existing
profiles are skipped, never overwritten. See
[ADR-2610261238](docs/adrs/ADR-2610261238-import-aws-profiles.md).

**Status line.** Every launch shows `⬡ <profile> +<kits> · <provider>` in
the status line, for example
`⬡ claude-bedrock +net · bedrock claude-bedrock@us-east-1`. ccs passes
`--settings` with the status line `ccs statusline --auto`, which ranks above a
project's own. `--auto` prints the segment, then runs the first other status
line it finds in the project's `.claude/settings.local.json`, the project's
`.claude/settings.json`, or the profile's `config/settings.json`. A status line
script you already have keeps working, with nothing to wrap.

To wrap a command by hand, for example in a settings file that ccs does not
launch:

```json
{ "statusLine": { "type": "command",
  "command": "ccs statusline -- ~/.claude/statusline-bedrock.sh" } }
```

The script gets the same JSON on stdin, and its first line is shown after the
segment. See [ADR-2610261237](docs/adrs/ADR-2610261237-statusline-segment.md).

**Migrating from an earlier ccs.** `ccs migrate` moves `shared/` into
`library/…/shared/` and a kit `shared` attached to every profile; each
profile's own items into `library/…/<profile>/` and a kit named after the
profile (lowercased, `_` and `.` turned into `-`, and a `p-` prefix when that
is still not a valid kit name); and
the old `native` registration into `ccs import`. An item whose name the
library already has (a profile's own copy of a shared skill) is not moved: it
stays in that profile's config dir, where Claude Code still loads it, and
`migrate` and `doctor` name it. Originals go to the trash; `~/.claude` is only
read. Running it again does nothing.

## File Structure

```
src/
  domain/mod.rs                        — rules: names, profile.env, resolution, launch plan, entries, kits, trees, CLI words
  ports/mod.rs                         — ProfileStore, Library, KitStore, Legacy, Editor, Browser, …
  usecases/mod.rs                      — one function per command; App, the TUI's port
  adapters/secondary/fs_store.rs       — profiles as directories under $CCS_HOME
  adapters/secondary/fs_library.rs     — the library, in folders; rm moves to .trash
  adapters/secondary/fs_kits.rs        — kits, and building them into plugins
  adapters/secondary/fs_legacy.rs      — reads the old layout and ~/.claude; retires old paths
  adapters/secondary/fs_paths.rs       — paths typed into the TUI
  adapters/secondary/fs_pins.rs        — finds .claude-profile
  adapters/secondary/exec_launcher.rs  — replaces the process with claude
  adapters/secondary/git_fetcher.rs    — clones and pulls plugins with git
  adapters/secondary/gh_releases.rs    — downloads releases with gh; installs over the running binary
  adapters/secondary/fs_text.rs        — reads settings files for the status line
  adapters/secondary/process_editor.rs — opens $VISUAL / $EDITOR
  adapters/secondary/process_runner.rs — runs a wrapped status line command
  adapters/secondary/aws_cli.rs        — AWS profile names and regions, via the aws CLI
  adapters/primary/tui.rs              — the ratatui TUI; talks to ports::Browser only
  lib.rs                               — composition root
  main.rs                              — entry point
tests/                                 — the gate: domain, profiles, kits, plugins, migrate, upgrade, assets, boundaries, the TUI, the real binary
.github/workflows/                     — CI (the gate on pushes to main and on pull requests) and Release (on a vX.Y.Z tag)
docs/adrs/                             — decisions
start.sh                               — build and run, or --install
```

## How It Works

`ccs` reads the profile and its kits, rebuilds each kit from the library, then
builds a launch plan: unset the provider variables inherited from your shell
(so a Bedrock export cannot leak into a personal profile), set the profile's
variables, `CLAUDE_CONFIG_DIR`, `CCS_PROFILE` and `CCS_KITS`, and put these
in front of your arguments: `--settings` with the status line, `--plugin-dir`
for each kit and for each plugin the kits pick, and
`--append-system-prompt-file` when a kit has instructions. Then it `exec`s
`claude`, so there is no wrapper process left between you and Claude Code.

The code is hexagonal, built with [hexa](https://github.com/gaberger/hexa); see
[docs/architecture.md](docs/architecture.md) for the layers, ports and a launch
end to end. The gate is:

```bash
cargo test && hexa analyze . --grade A+
```

`A+` and not `A`: one boundary violation costs 10 points, which still grades
`A`. `tests/boundaries.rs` adds the one rule `hexa analyze` cannot express:
the TUI (a primary adapter) imports `ports/` only.

## Environment

| Variable | Effect |
|---|---|
| `HOME` | Required; ccs stops if it is unset. `~/.claude-profiles` and `~/.claude` are found from it. |
| `CCS_HOME` | Where profiles, the library and kits live. Default `~/.claude-profiles`. |
| `CCS_PROFILE` | Selects a profile when no argument or pin file does. |
| `CCS_CLAUDE_BIN` | Program to launch instead of `claude`. The tests use a script that prints its arguments. |
| `CCS_AWS_BIN` | Program to run instead of `aws`. The tests use a fake. |
| `CCS_GIT_BIN` | Program to run instead of `git`, for plugins. |
| `CCS_GH_BIN` | Program to run instead of `gh`, for `ccs upgrade`. The tests use a fake. |
| `CCS_REPO` | Where `ccs upgrade` looks for releases. Default `gaberger/ccs`. |
| `VISUAL`, `EDITOR` | The editor `edit` and the TUI's `e` open. Default `vi`. |
| `SHELL` | The shell a profile's `shell` opens in the TUI. Default `/bin/sh`. |

## System Overview

This section and the ones after it are the repository blueprint: the contract
each part of the code keeps, for a person or an agent about to change it.
[What This Does](#what-this-does) says what ccs is for. In code terms, ccs is
one Rust binary in hexagonal layers. `src/domain` holds the rules, `src/ports`
the contracts, `src/usecases` one function per command, and `src/adapters` the
TUI and the code that touches files, `git`, `gh`, `aws` and `claude`.
`src/lib.rs` wires them once.

## Local Interface & Invariants

- `start.sh`: builds a release with `cargo build --release`, then runs
  `target/release/ccs` with your arguments. `./start.sh --install` copies the
  binary to `$CCS_INSTALL_DIR`, default `~/.local/bin`, with mode 0755.
- `start.sh` stops with a message if `cargo` is not on your PATH.

## Subdirectories & System Boundaries

- src: [./src/README.md](./src/README.md) - the program; `lib.rs` is the composition root.
  - src/domain: [./src/domain/README.md](./src/domain/README.md) - pure rules: names, `profile.env`, resolution, the launch plan, kits, the CLI words. No I/O.
  - src/ports: [./src/ports/README.md](./src/ports/README.md) - 13 driven ports and the driving port `Browser`; re-exports the domain types that adapters need.
  - src/usecases: [./src/usecases/README.md](./src/usecases/README.md) - one function per command; `App` implements `Browser` for the TUI.
  - src/adapters: [./src/adapters/README.md](./src/adapters/README.md) - the ratatui TUI ([primary](./src/adapters/primary/README.md)) and 13 driven adapters ([secondary](./src/adapters/secondary/README.md)).
- tests: the gate. Not a contract package.
- docs: [architecture](./docs/architecture.md) and the decisions in `docs/adrs/`.

## Global Invariants

- The layers keep the hexagonal import rules in `CLAUDE.md`. `hexa analyze .`
  checks them, and `tests/boundaries.rs` checks that the TUI imports `ports/` only.
- ccs never writes to `~/.claude`. A profile's `config/` holds login state,
  history and the `settings.json` that ccs seeds for the status line; kits and the library live beside it under `$CCS_HOME`.
- A launch changes the environment of one `claude` process only. It unsets the
  inherited provider variables first, so one login cannot leak into another.
- Removal moves things to `$CCS_HOME/.trash`. It does not erase them.
- `ccs upgrade` installs a release only after its checksum matches and the new
  binary reports exactly the expected version.

## Global Side Effects

- Files under `$CCS_HOME` (default `~/.claude-profiles`): profiles, the
  library, kits, `.build/` and `.trash/`.
- Reads `~/.claude` for import and migration, and `.claude-profile` pin files.
- External programs: `claude` (by exec), `$SHELL` (by exec, for a profile's
  shell in the TUI), `git`, `gh`, `aws`, `tar`, a checksum
  tool, the editor and a wrapped status line command.
- Network through `git` and `gh`. `ccs upgrade` replaces the running binary.

## System Verification

- Run: `cargo test`
- Then: `cargo clippy --all-targets -- -D warnings` and `hexa analyze . --grade A+ --exit-code`
