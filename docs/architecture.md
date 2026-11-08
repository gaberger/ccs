# ccs architecture

How ccs is put together, and why. The README says what ccs does and how to use
it; this says where each part lives and how a launch moves through it. The
decisions behind it are in [docs/adrs/](adrs/), and each section names the one
it comes from.

## The shape

ccs is one Rust binary with one dependency, `ratatui`. There is no daemon and
no database. Every command reads files under `$CCS_HOME`, does its work, and
exits. A launch doesn't exit: it `exec`s `claude`.

The code is hexagonal, and `hexa analyze .` checks the boundaries:

```text
            domain            pure rules: names, profile.env, resolution, launch plans,
              ↑               entries, kits, library trees, CLI parsing, versions
            ports             traits: ProfileStore, Library, KitStore, Launcher, Browser, …
           ↗     ↖            (and re-exports of the domain types adapters need)
     usecases     adapters    usecases: one function per command, plus App
           ↖     ↗            adapters: filesystem, processes, git, gh, aws, the TUI
            lib.rs            composition root: builds the adapters, wires them, prints
              ↑
            main.rs           SIGPIPE, then ccs::run(args)
```

| Layer | File | May import |
|---|---|---|
| domain | `src/domain/mod.rs` | `domain` only |
| ports | `src/ports/mod.rs` | `domain`, for value types |
| use cases | `src/usecases/mod.rs` | `domain`, `ports` |
| secondary adapters | `src/adapters/secondary/*.rs` | `ports` only |
| primary adapter (TUI) | `src/adapters/primary/tui/` | `ports` only |
| composition root | `src/lib.rs` | everything |

Adapters never import each other. When an adapter needs a domain type, it gets
it from `ports`, which re-exports it, sometimes under a name that fits a port
better (`Profile as LoadedProfile`, `Use as Entry`). That way an adapter has
one edge into the core, not two.

`hexa analyze` lets a primary adapter import use cases. This project is
stricter: `tests/boundaries.rs` reads every `.rs` file under
`src/adapters/primary/` and fails on any line that names `crate::usecases`,
`crate::domain` or `crate::adapters::secondary`. It also fails if it finds no
file, so it can't pass over nothing.

## Domain

`src/domain/mod.rs` has no I/O. Everything in it is a function from values to
values, so `tests/domain.rs` tests it without a disk or a process. It holds:

- **Names.** `ProfileName` and `KitName` parse and reject reserved words
  (`list`, `kit`, `library`, …), so a profile can never shadow a command.
  `sync` is reserved too: `parse_command` refuses `ccs sync`, which is decided
  but not built, and says to copy `$CCS_HOME/library/` instead.
- **`profile.env`.** `parse_env` reads `KEY=VALUE` lines. The file is never run
  by a shell. `check_env` produces the `Finding`s that `ccs doctor` reports.
- **Resolution.** `resolve` picks the first non-blank of: argument, pin file,
  `$CCS_PROFILE`, default. If none of them names a profile, the result is
  `None`. It never falls back to `~/.claude`.
- **Launch plans.** `plan_launch` turns a `Profile` into a `LaunchPlan`:
  variables to unset (every name in `PROVIDER_VARS`), variables to set, and
  arguments. `kit_args` makes the `--plugin-dir` flags and the instructions
  flag, and `with_kits` puts arguments in front of the user's and sets
  `CCS_KITS`. `render_exports` prints a plan as shell for `ccs env`.
- **Entries and kits.** `Use` is an entry (`skills/x`, `agents/x`,
  `plugins/x`, or a folder `skills/net/`). `resolve_kit` matches a kit's
  entries against the library and returns what it found and what is missing.
  `library_tree`, `pick` and `rewrite_folder` feed the TUI's tree and keep
  kits right when a folder moves.
- **The command line.** `parse_command` turns `argv` into a `Command`. It takes
  an `is_profile` closure so that `ccs work` can mean "launch work" without the
  domain touching the disk.
- **Status line and releases.** `status_segment`, `compose_status`,
  `launch_settings`, and the version, target and checksum rules that
  `ccs upgrade` relies on.

## Ports

`src/ports/mod.rs` names what the core needs without saying how it's done.
A port has no idea that a profile is a directory, so a test can swap in a temp
dir.

| Port | Role | Adapter |
|---|---|---|
| `ProfileStore` | profiles: variables, default kits, config dir | `fs_store` |
| `Library` | skills, agents, plugins in folders | `fs_library` |
| `KitStore` | kits, and building them into plugins | `fs_kits` |
| `Legacy` | the old layout and stock `~/.claude`, read only | `fs_legacy` |
| `Fetcher` | clone and pull plugin repositories | `git_fetcher` |
| `Releases` | find, download, verify, install a release | `gh_releases` |
| `Launcher` | start Claude Code | `exec_launcher` |
| `PinLocator` | find `.claude-profile` | `fs_pins` |
| `PathInput` | expand and complete paths typed in the TUI | `fs_paths` |
| `Editor` | open `$VISUAL` / `$EDITOR` | `process_editor` |
| `AwsProfiles` | AWS CLI profile names and regions | `aws_cli` |
| `TextFiles` | read settings files that may be missing | `fs_text` |
| `CommandRunner` | run a wrapped status line command | `process_runner` |
| `Browser` | **driving port**: everything the TUI may ask | `usecases::App` |

`Browser` points the other way from the rest. The TUI calls it, and
`usecases::App` implements it. Most methods delegate to an existing use case;
the plain reads (`kits`, `kit_uses`, `kit_instructions`, `complete_path`) call
a port directly. Either way the TUI reaches only what the CLI reaches, so it
can't do anything the CLI can't, and `tests/tui.rs` drives it with a fake
`Browser`.

## Use cases

`src/usecases/mod.rs` has one public function per command: `prepare_launch`,
`doctor`, `import_aws`, `migrate`, `library_move`, `kit_use`, `upgrade`, and
so on. Each one takes its ports as `&dyn` parameters and never picks an
adapter. A rule that changes several stores together lives here. For example,
`library_move` moves an item and then rewrites every kit that named its folder.

## Composition root

`src/lib.rs` is the only file that names a concrete adapter, prints, or reads
configuration from the environment. An adapter touches the environment only
for its own mechanics: `ExecLauncher` looks for its program on `PATH`,
`GitFetcher` and `GhReleases` make folders under `temp_dir()`, and
`GhReleases` finds the running binary with `current_exe()`.

`lib.rs` also exports one factory per port it can build from a path or a
variable: `profile_store`, `legacy`, `library`, `kit_store`, `path_input`,
`pin_locator`, `fetcher` and `editor`. `dispatch` uses them, and so do
`tests/assets.rs` and `tests/profiles.rs`, to get the real adapter behind a
port. `dispatch`:

1. reads `HOME` and `CCS_HOME`, plus the `CCS_*_BIN` overrides (`claude`,
   `aws`, `git`, `gh`)
2. builds the adapters every command shares, once, rooted at `$CCS_HOME`
3. parses the command with `domain::parse_command`
4. calls the use case, prints its result, and returns an exit code

A command reads the other variables it needs and builds the adapters only it
uses, in its own branch:

- resolving a name reads `CCS_PROFILE`
- the editor is `$VISUAL`, else `$EDITOR`, else `vi`; `library` and `kit`
  build their own `ProcessEditor`
- the TUI's shell is a second `ExecLauncher`, running `$SHELL` (else `/bin/sh`)
- `upgrade` builds `GhReleases` from `CCS_GH_BIN` and `CCS_REPO` (default
  `gaberger/ccs`)
- `statusline` builds `ProcessRunner` and `FsText` on each call, and passes the
  whole environment to the use case

The `CCS_*_BIN` variables are how the tests replace `claude`, `aws`, `git` and
`gh` with scripts, while still running the real binary.

## A launch, end to end

`ccs work +net -- --resume`:

1. **Parse.** `parse_command` sees that `work` is a profile, so the command is
   a launch with kits `[net]` and the arguments `--resume`.
2. **Resolve.** `usecases::resolve_name` uses the argument. Without one, it
   tries `.claude-profile` (via `PinLocator`), then `$CCS_PROFILE`, then the
   default.
3. **Load.** `ProfileStore::load` reads `<home>/work/profile.env` and the
   config dir `<home>/work/config/`.
4. **Plan, then build.** `load_plan` takes the kits named on the command line,
   else the profile's own. A kit that doesn't exist stops the launch
   (`no kit '<name>'`). The pure `domain::load_plan` then, for each kit:
   - resolves its entries against the library, collecting a warning for each
     missing one (a missing entry doesn't stop the launch)
   - splits out picked plugins, which load as they are, and dedups them
     across kits
   - notes two agents with one front-matter name, as a warning

   This plan is all `ccs loads` and the screen read; it builds nothing
   (ADR-2610281305). `prepare_launch` then, for each kit, calls
   `KitStore::build`, which clears `<home>/.build/<kit>/` and rebuilds
     it as a plugin. Skills are links into the library, so edits show up
     immediately. Agents are copies, because Claude Code doesn't read an agent
     through a link.
   - appends the kit's instructions to one prompt file,
     `<home>/.build/<kit+kit…>.md`

   When a kit and a picked plugin share a name, both name their skills
   `<name>:…`, so the plan warns and says to rename the kit. The launch goes
   on. `ccs kit new` and `kit rename` refuse a plugin's name, so this only
   happens when the plugin arrives after the kit.
5. **Plan.** `plan_launch` unsets the provider variables inherited from the
   shell, so a Bedrock export can't leak into a personal login. It then sets
   the profile's variables, `CLAUDE_CONFIG_DIR` and `CCS_PROFILE`.
   `prepare_launch` builds the extra arguments: `--settings <status line>`,
   then, from `kit_args`, one `--plugin-dir` per kit and plugin and
   `--append-system-prompt-file`. `with_kits` adds `CCS_KITS` and puts those
   arguments in front of the user's.
6. **Exec.** `ExecLauncher::launch` replaces the ccs process with `claude`.
   Signals, the TTY and the exit code belong to Claude Code, with no wrapper in
   between. `launch` only returns if the exec failed.

Nothing is written into `work/config/` along the way, so two terminals can
run the same login with different kits (ADR-2610261643).

## On disk

```text
$CCS_HOME/                    default ~/.claude-profiles
  <profile>/
    profile.env               KEY=VALUE, never run by a shell
    kits                      the profile's default kits, one per line
    config/                   CLAUDE_CONFIG_DIR: login state, history, settings.json
  library/
    skills/<folders…>/<name>/ a skill folder
    agents/<folders…>/<name>.md
    plugins/<name>/           a git checkout, loaded as itself
  kits/<kit>/
    uses                      the kit's entries, one per line
    CLAUDE.md                 the kit's instructions
    .mcp.json                 optional
  .default                    the default profile's name
  .build/<kit>/               rebuilt on every launch; never edit
  .build/<kit+kit…>.md        one launch's joined instructions
  .trash/                     where rm, migrate and kit rm move things
```

`~/.claude` is only read, by `ccs import` and `ccs migrate`. ccs never writes
to it.

## The TUI

`src/adapters/primary/tui/` is a ratatui app: one screen of four columns,
used left to right (ADR-2610281305): 1 profile → 2 its kits → 3 what is in the
kit → 4 add from the library. It is built in parts:

- `State` (`columns.rs`) takes a `Key` (the adapter's own key type) or a mouse
  hit and updates itself, calling `Browser` for anything that reads or changes
  data. After every change it reads everything again; it keeps no copy that a
  later read could contradict.
- `draw(frame, &State)` (`draw.rs`) paints it: the step line, the columns, the
  description strip, the notice with the matching `ccs` command, and the key
  line.
- `overlays.rs` holds what sits on top for a moment: search, typed text, a y/n
  question, the `v` reader, the import list.
- `run` (`mod.rs`) is the only public part that touches a terminal. It calls
  the private `drive`, which opens the terminal (alternate screen, mouse
  capture), runs the event loop, and closes it on every path. To hand off to
  an editor, `drive` closes the terminal, waits for the editor, and opens it
  again.

Column 3 on a profile reads `Browser::load_plan`, the same plan the launch uses
and `ccs loads` prints. A change in the TUI runs the same use case as the CLI
command it displays, and `tests/parity.rs` runs that displayed command on a
second home to prove it.

Because only `run` needs a terminal, `tests/tui.rs` drives `State` with keys
and draws into ratatui's `TestBackend`. Leaving the TUI returns an `Exit`:
`Quit`, `Launch(name)` with the profile's kits, `LaunchWith { profile, kits }`,
or `Shell(name)`, a shell in the profile's folder with its variables. The
terminal is restored by then, and `lib.rs` launches exactly as `ccs <name>`
would, or execs `$SHELL` for `Shell`.

## Status line

Every launch passes `--settings` with a status line of
`ccs statusline --auto`. Command-line settings outrank a project's, so the
profile segment (`⬡ work +net · bedrock …`) always shows. `statusline_auto`
then runs the profile's own status line command, then the project's (its local
settings before its shared ones), each with the same stdin. The first output
line follows the segment and the rest follow in order; a command both name runs
once (ADR-2610261237 §3).

## Releases and upgrade

A `vX.Y.Z` tag runs `.github/workflows/release.yml`, which builds archives for
macOS (aarch64, x86_64) and Linux (x86_64) and publishes them with
`SHA256SUMS.txt`. `usecases::upgrade` downloads the archive for this platform
through `Releases` (the `gh` CLI). It installs
the new binary over the running one only if all of these hold:

- the checksum matches the published one
- the unpacked binary runs
- the binary reports the expected version

If any step fails, nothing is installed (ADR-2610271000). Whatever the
outcome, `Releases::discard` removes the download folder after every attempt;
if the download itself fails, `GhReleases` removes the folder it made.

## The gate

```bash
cargo test && hexa analyze . --grade A+ --exit-code
```

`.github/workflows/ci.yml` runs it, along with clippy, on pushes to `main`, on
pull requests, and when `release.yml` calls it. CI also adds up the passed
counts of every `test result:` line and fails on 0, so a run with no tests is a
failure, not a pass. The tests are split by what they guard:

| Test | Guards |
|---|---|
| `domain.rs` | pure rules, no disk or process |
| `profiles.rs`, `kits.rs`, `plugins.rs`, `migrate.rs` | the stores and the real binary, each in a temp `$CCS_HOME` |
| `cli.rs` | the real binary end to end, with a fake `claude` that prints its env and args |
| `upgrade.rs` | `ccs upgrade` against a fake `gh` serving archives built in the test |
| `assets.rs` | item front matter, typed paths, the real `App` behind `Browser` |
| `tui.rs` | the column screen through keys and `TestBackend`, with a fake `Browser` shaped like the real library: the success test (G1), every key on screen first (G2), search order (G1b) |
| `parity.rs` | each change made on the screen, its displayed `ccs` command run on a second home, the two homes compared (G3) |
| `loads.rs` | the planned launch equals the built one, and planning writes nothing (G4) |
| `boundaries.rs` | primary adapters import none of `usecases`, `domain`, secondary adapters |

No test touches the real `~/.claude`.

## Decisions

| ADR | Decision | Status |
|---|---|---|
| [2610261212](adrs/ADR-2610261212-profile-per-config-directory.md) | one config directory per profile, chosen at launch | Proposed |
| [2610261237](adrs/ADR-2610261237-statusline-segment.md) | the status line shows the profile, wrapping any existing one | Accepted |
| [2610261238](adrs/ADR-2610261238-import-aws-profiles.md) | import AWS CLI profiles as Bedrock profiles | Accepted |
| [2610261255](adrs/ADR-2610261255-ratatui-tui.md) | ratatui TUI as the first primary adapter | Accepted |
| [2610261643](adrs/ADR-2610261643-logins-kits-library-sync.md) | logins and kits separate; library in folders; plugins (its Home and pages are replaced by 2610281305) | Accepted |
| [2610271000](adrs/ADR-2610271000-release-and-upgrade.md) | releases built by CI, and `ccs upgrade` | Accepted |
| [2610281305](adrs/ADR-2610281305-profile-first-columns.md) | the TUI becomes four columns, profile first; one load plan; safe kit saves | Proposed |
| 2610261244, 2610261720, 2610261902, 2610261933 | per-profile items, `shared/`, copying between profiles | Superseded by 2610261643 |
