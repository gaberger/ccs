# ccs

Switch Claude Code between logins — a personal claude.ai account, AWS Bedrock accounts — and choose which skills, agents and instructions each session gets.

<a href="https://github.com/gaberger/hexa"><img src="https://raw.githubusercontent.com/gaberger/hexa/main/.github/assets/badges/built-with-hexa.svg" alt="built with hexa" height="20"></a>

## What This Does

ccs keeps two things apart:

- A **profile** is a login: its provider variables (Bedrock, model, region) and
  its own Claude Code config directory, which holds only login state and
  history.
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

- Rust 1.88+ (`cargo`); `ratatui` is the one dependency
- Claude Code (`claude` on your PATH), with `--plugin-dir`
- macOS or Linux

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
ccs [claude args…]             the launch screen: choose a provider and kits, then ⏎
ccs <profile> [+kit…] [args…]  start claude with <profile>; +kit replaces its kits for this run
ccs list                       list profiles with their kits (* = resolved)
ccs new <name> [--from <p>]    make a profile (copies variables, kits, settings; never login)
ccs default [<name>]           show or set the default profile
ccs env [<name>]               print exports: eval "$(ccs env work)"
ccs doctor [<name>]            check a profile and its kits; exits 1 on a failure
ccs which                      print the resolved profile
ccs ui                         the TUI: profiles, kits, the library; launch from the list
ccs statusline [-- <cmd…>]     status line segment; wraps an existing status line command
ccs import                     copy ~/.claude skills, agents, CLAUDE.md into the library (kit 'imported')
ccs import aws [<p>…] [--all] [--region <r>]
                               list AWS CLI profiles, or import them as Bedrock profiles
ccs migrate                    convert the shared/ and native layout of earlier versions

ccs library list [skills|agents] | add <path> [--in <folder>] | mkdir skills/<folder>
            mv <entry> <folder|/> | rename skills/<folder>/ <name> | rm <entry> | show <entry> | edit <entry>
ccs kit     list | new <k> | rm <k> | show <k> | edit <k> | rename <k> <new> | use <k> <entry>… | drop <k> <entry>…
            attach <k> <profile> | detach <k> <profile>
```

An **entry** is `skills/<name>`, `agents/<name>`, or a folder such as
`skills/net/` (everything under it, including what you add later).

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

**Kits.** `ccs kit use net skills/net/ agents/rev` picks from the library;
`ccs kit edit net` writes the kit's instructions. At launch, each kit is
rebuilt into `~/.claude-profiles/.build/<kit>/` as a plugin: skills are links
into the library (so edits are live), agents are copies (Claude Code does not
read an agent through a link, so an agent edit applies from the next launch),
and the instructions are appended to the system prompt. In Claude Code a kit's
skills are named `<kit>:<skill>`, for example `/net:find-nqe-query`.

**The TUI.** `ccs ui` has a menu bar, `Profiles │ Kits │ Library`; `1` `2`
`3` switch section, and each section keeps its place. The list is on the left
and the selected row's tabs on the right; below 70 columns the list sits over
them. Everything the CLI does to profiles, kits and the library, the TUI does
too.

```
1 Profiles  Overview · Kits     ⏎ launch · t shell in the profile's folder, with its variables
                                n new (also on an empty section) · c copy (variables, kits, settings;
                                never the login) · e edit profile.env · d default
                                Kits tab: space attaches a kit
2 Kits      Skills · Agents · CLAUDE.md · Profiles
                                n new kit · r rename · x delete (asks y/n)
                                space pick: [x] picked, [·] picked by a folder above, [ ] not
                                p show only what it picks · e edit (an item, or the kit's CLAUDE.md) · v view
                                Profiles tab: space attaches the kit to a profile
3 Library   skills · agents     space mark (●) · m move to a folder · + add to a kit (both pick from
                                a list; type to filter, or type a new name) · x delete (asks y/n)
                                a add from a path (Tab completes) · n new folder · r rename a folder
                                e edit · v view · each row shows the kits that pick it: ‹network›
everywhere  i import: ~/.claude, an AWS profile, every AWS profile, or migrate an old layout
            ↑↓ move · PgUp/PgDn a page · Home/End (g/G) first/last · mouse wheel
            ←→ tab · tab focus · / search · q quit
            a list of ticks opens scrolled to its first ticked row
```

See [ADR-2610261643 §8](docs/adrs/ADR-2610261643-logins-kits-library-sync.md).

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

**Status line.** Every profile made by `ccs new` or `ccs import aws` shows
`⬡ <profile> +<kits> · <provider>` in the status line, for example
`⬡ claude-bedrock +net · bedrock claude-bedrock@us-east-1`. To keep a status
line script you already have, wrap it in the profile's `config/settings.json`:

```json
{ "statusLine": { "type": "command",
  "command": "ccs statusline -- ~/.claude/statusline-bedrock.sh" } }
```

The script gets the same JSON on stdin, and its first line is shown after the
segment. See [ADR-2610261237](docs/adrs/ADR-2610261237-statusline-segment.md).

**Migrating from an earlier ccs.** `ccs migrate` moves `shared/` into
`library/…/shared/` and a kit `shared` attached to every profile; each
profile's own items into `library/…/<profile>/` and a kit of that name; and
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
  adapters/secondary/process_editor.rs — opens $VISUAL / $EDITOR
  adapters/secondary/process_runner.rs — runs a wrapped status line command
  adapters/secondary/aws_cli.rs        — AWS profile names and regions, via the aws CLI
  adapters/primary/tui.rs              — the ratatui TUI; talks to ports::Browser only
  lib.rs                               — composition root
  main.rs                              — entry point
tests/                                 — the gate: domain, profiles, kits, migrate, the TUI, the real binary
docs/adrs/                             — decisions
start.sh                               — build and run, or --install
```

## How It Works

`ccs` reads the profile and its kits, rebuilds each kit from the library, then
builds a launch plan: unset the provider variables inherited from your shell
(so a Bedrock export cannot leak into a personal profile), set the profile's
variables, `CLAUDE_CONFIG_DIR`, `CCS_PROFILE` and `CCS_KITS`, and put
`--plugin-dir` for each kit in front of your arguments. Then it `exec`s
`claude`, so there is no wrapper process left between you and Claude Code.

The code is hexagonal, built with [hexa](https://github.com/gaberger/hexa). The
gate is:

```bash
cargo test && hexa analyze . --grade A+
```

`A+` and not `A`: one boundary violation costs 10 points, which still grades
`A`. `tests/boundaries.rs` adds the one rule `hexa analyze` cannot express:
the TUI (a primary adapter) imports `ports/` only.

## Environment

| Variable | Effect |
|---|---|
| `CCS_HOME` | Where profiles, the library and kits live. Default `~/.claude-profiles`. |
| `CCS_PROFILE` | Selects a profile when no argument or pin file does. |
| `CCS_CLAUDE_BIN` | Program to launch instead of `claude`. The tests use a script that prints its arguments. |
| `CCS_AWS_BIN` | Program to run instead of `aws`. The tests use a fake. |
| `VISUAL`, `EDITOR` | The editor `edit` and the TUI's `e` open. Default `vi`. |
