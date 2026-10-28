# usecases

## Overview

The application layer of `ccs`. Each public function is one `ccs` command (or
one step of one) and is built from `crate::domain` functions and `crate::ports`
traits. Every function takes its ports as `&dyn Trait` parameters; it never
picks or builds an adapter. The composition root (`src/lib.rs`) wires the
adapters in and calls these functions. The TUI reaches the same functions
through the `App` struct, which implements the `ports::Browser` driving port.

The whole layer is one file, `mod.rs`. It covers six areas:

1. Profiles: resolve, create, list, default, shell, env exports, doctor.
2. Launch: build a `LaunchPlan` with kits and hand it to the `Launcher`.
3. Status line: compose the `ccs` segment with a wrapped or discovered command.
4. AWS import and `~/.claude` import / legacy layout migration.
5. Library (skills, agents, plugins) and kits.
6. Self-upgrade from GitHub releases.

## Interface

All items live in `mod.rs` (`crate::usecases`).

### Profiles

| Symbol | Purpose |
|---|---|
| `resolve_name(store, pins, explicit, env_profile)` | Pick a profile name from, in order, `explicit`, the pinned name, `env_profile` (`CCS_PROFILE`), and the store's default (via `domain::resolve`). The profile must exist (`NoProfile`). If no source gives a name, `NoneResolved`. |
| `existing(store, raw)` | Parse a `ProfileName` and require it to exist. Otherwise `CcsError::NoProfile`. |
| `load(store, name)` | Load a `Profile`. |
| `list(store, current)` | `Vec<ListEntry>`: name, current/default flags, kits, provider label. A profile that fails to load shows its error text as its provider label. |
| `new_profile(store, name, from)` | Create a profile, optionally copied from `from`. Errors `Exists` / `NoProfile`. |
| `make_profile(store, raw, from, ccs_bin)` | `ccs new`. Creates the profile. A new empty profile also gets the status-line `settings.json`. |
| `shell_plan(store, name)` | A `LaunchPlan` for a shell in the profile's folder with the launch variables set. |
| `default_name(store)` / `set_default(store, name)` | Read or set the default profile. |
| `doctor(store, kits, lib, legacy, name, claude_available)` | `Vec<Finding>`: env checks, config dir, each kit (missing kit = Fail, missing library entry = Warn), legacy layout warnings, and `claude` on `PATH`. |
| `exports(store, name)` | Shell `export` lines for `eval "$(ccs env <name>)"`. |
| `seed_statusline(store, name, ccs_bin)` | Write the default `settings.json` only if none exists. Returns whether it wrote. |

### Launch

| Symbol | Purpose |
|---|---|
| `prepare_launch(store, kits, lib, name, chosen, args, ccs_bin)` | `load_plan`, then `KitStore::build` for each kit, then `write_prompt` when a kit has instructions. Returns `(LaunchPlan, warnings)`; the paths in the launch come from `build_dir` and `prompt_path`. |
| `launch(launcher, plan)` | Replaces the process with `claude`. Returns a `CcsError` only on failure. |

### Status line

| Symbol | Purpose |
|---|---|
| `statusline(runner, vars, stdin, wrap)` | The `ccs` segment followed by the output of the explicit `wrap` command. |
| `statusline_auto(runner, files, vars, stdin)` | `ccs statusline --auto`. Runs the first other status line it finds in the project's `.claude/settings.local.json`, then the project's `.claude/settings.json`, then `$CLAUDE_CONFIG_DIR/settings.json`. The command runs through `/bin/sh -c` with the same stdin. |

### AWS / import / migrate

| Symbol | Purpose |
|---|---|
| `aws_status(store, aws)` | `Vec<AwsRow>`: each AWS CLI profile and the `ccs` profile that already uses it, if any. Read-only. |
| `import_aws(store, aws, wanted, all, region, ccs_bin)` | Makes one Bedrock profile per AWS profile. It skips a profile and gives the reason, and never overwrites. |
| `import(lib, kits, legacy)` | Copies `~/.claude` skills, agents and `CLAUDE.md` into the library under `imported/` and into kit `imported`. |
| `migrate(store, lib, kits, legacy)` | Converts the layout of earlier versions (`shared/`, per-profile items and links, `native`) into library entries and kits. Returns notes. |
| `needs_migrate(legacy)` | Whether a legacy layout is present. |

### Library and kits

| Symbol | Purpose |
|---|---|
| `library_list(lib, kind)` | Items and folders of one `Kind`. |
| `library_add_source(lib, fetch, raw, local, folder)` | Adds a local path. If `local` does not exist and `raw` is a git source, it clones `raw` to a temp dir, adds the clone, then discards the temp dir. |
| `plugin_update(lib, fetch, which)` | `git pull` every plugin, or the one in `which`. Returns one line per plugin. |
| `library_mkdir`, `library_move`, `library_rename_folder` | Folder operations on skills and agents. Moving or renaming a folder rewrites the kit lines under it. Plugins have no folders. |
| `library_remove(lib, kits, what)` | Moves an item or folder to the trash. Returns `LibraryRemoved { trashed, kits }`, where `kits` lists the kits that used it. |
| `library_show`, `library_edit` | Reads an item (for a plugin: its skills and agents), or opens its main file in the editor. |
| `kit_new`, `kit_remove`, `kit_rename` | Kit lifecycle. Remove detaches the kit from every profile. Rename keeps the kit's place in each profile's list. |
| `kit_show(kits, lib, raw)` | The kit's lines, each with what it gives (`Vec<LineGives>`). |
| `load_plan(store, kits, lib, name, chosen)` | What a profile loads: a `LoadPlan`. Reads only; builds nothing. |
| `planned_launch(...)` | The launch `prepare_launch` would make, with nothing built. |
| `loads(profile, plan)`, `lines_text(lines)` | The text of `ccs loads` and `ccs kit show`. |
| `kit_use`, `kit_drop` | Add or remove entries. `kit_use` requires each entry to resolve in the library now. |
| `kit_attach(store, kits, kit, profile, attach)` | Attaches a kit to a profile (moving it to the end of the list) or detaches it. |
| `kit_edit(kits, editor, raw)` | Opens the kit's instructions file in the editor. |

### Upgrade

| Symbol | Purpose |
|---|---|
| `enum Upgraded { UpToDate(String), Available { current, latest }, Installed { from, to, at } }` | What `ccs upgrade` did. |
| `upgrade(rel, current, target, wanted, check)` | Downloads the release asset and `SHA256SUMS.txt`, checks the checksum, unpacks the binary, and runs it to check that it reports exactly that version before `Releases::install`. |

### TUI driving port

| Symbol | Purpose |
|---|---|
| `struct App<'a>` | Holds `store`, `kits`, `lib`, `legacy`, `paths`, `editor`, `aws`, `fetch` (all `&dyn` ports), plus `ccs_bin`, `claude_available` and `current`. |
| `impl ports::Browser for App` | Each method delegates to one use case above. Nothing in it is new behaviour. |
| `overview(app, name)` | `Overview`: segment, env, kits, folder, default flag and `doctor` findings. |

Private helpers: `profile_edit`, `imported_as`, `legacy_path`, `adopt`,
`fill_kit`, `attach_last`, `library_items`, `library_item`, `existing_kit`,
`library_add`, `no_folders`, `plan_kit_rewrites`, `write_kit_rewrites`,
`set_profile_kits`.

## Invariants

- **Layer rule (hexagonal rule 3):** this module imports only `crate::domain`,
  `crate::ports` and `std`. It never names an adapter. `hexa analyze .`
  enforces this. `tests/boundaries.rs` enforces the matching rule on the other
  side: primary adapters must not import `crate::usecases`, so the TUI reaches
  this layer only through `ports::Browser`.
- Every port arrives as a parameter (`&dyn Trait`) or through `App`. No use
  case builds a port or reads the process environment itself. Environment
  values such as `CCS_PROFILE` and `ccs_bin` are passed in.
- Name resolution order is: explicit, then pinned, then `CCS_PROFILE`, then
  default (`domain::resolve`). A resolved name must exist, otherwise
  `CcsError::NoProfile`. If no source gives a name, `CcsError::NoneResolved`.
- Only `prepare_launch` writes a build. `load_plan`, `planned_launch`, `loads`
  and `kit_show` read only (ADR-2610281305, gate `tests/loads.rs`).
- `kit_new` and `kit_rename` refuse a library plugin's name.
- `kit_rename` rewrites the profiles first and renames the kit folder last.
  If one step fails, it puts back the profiles it already rewrote, so every
  profile still names a kit that exists.
- `doctor` warns when two agents in one kit share a front-matter name.
- `new_profile`, `kit_new` and `kit_rename` never overwrite an existing profile
  or kit. `import_aws` skips existing profiles and gives a reason.
  `seed_statusline` writes only if `settings.json` is absent.
- A kit refers to library items by name, not path. Item names are unique per
  `Kind` (`library_add` rejects a duplicate with `AssetExists`). Moving an item
  therefore changes no kit. Moving or renaming a folder must rewrite the kit
  `Use::Folder` lines under it. Every kit is read before the folder moves
  (`plan_kit_rewrites`), so an unreadable kit refuses the move while a retry
  can still find the folder; after the move, a kit that cannot be written puts
  back the kits already written and moves the folder back
  (`write_kit_rewrites`), so no kit is left on a folder that is gone.
- Plugins have no folders. `library_mkdir`, `library_move`,
  `library_rename_folder`, and `library_add` with a non-empty folder return
  `no_folders()` for `Kind::Plugin`. A plugin's library name is its manifest
  name.
- A kit entry missing from the library does not stop a launch. It becomes a
  warning (`prepare_launch`) or a `Finding::Warn` (`doctor`). `kit_use` refuses
  to add an entry that does not resolve.
- In `prepare_launch`, a picked plugin is loaded once, as itself, however many
  kits pick it. A kit and a plugin with the same name produce a warning. The
  `--settings` from `launch_settings` comes before the kit args, so the `ccs`
  status line ranks above a project's.
- `upgrade` installs only after the SHA-256 matches the published
  `SHA256SUMS.txt` line and the last word of the unpacked binary's `version`
  output, less any leading `v`, equals the expected version (`ccs 0.5.10` is
  not 0.5.1).
  The download dir is always discarded, on success or failure. Without
  `wanted`, it installs only a strictly newer version.
- `import` and `migrate` only read `~/.claude`. `migrate` moves the legacy paths
  `ccs` made into the trash (`Legacy::retire`, which removes old links)
  instead of deleting them. An item whose
  name the library already has is left where it is, and a note names it.
- `App`'s `Browser` methods only delegate. A TUI action and the matching CLI
  command run the same use case.

## Side Effects

The functions here do almost no I/O themselves. The one exception is
`library_add_source`, which calls `Path::exists` on a typed local path. All
other effects go through the ports, and the adapters in `src/adapters/secondary/` carry them out:

- **Process replacement:** `launch` → `Launcher::launch` (execs `claude`).
- **Profile filesystem:** `ProfileStore::create`, `write_env`,
  `write_settings_if_absent`, `set_default`, `clear_default`, `set_kits`.
- **Library/kit filesystem:** `Library::add`, `make_folder`, `move_to`,
  `rename_folder`, `trash`. `KitStore::create`, `set_uses`,
  `set_instructions`, `rename`, `remove`, `build` (writes the built kit dirs),
  `write_prompt`.
- **Legacy layout:** `Legacy::retire` moves old dirs and `CLAUDE.md` files
  to the trash and removes old links. `Legacy::read` reads them.
- **Network / git:** `Fetcher::clone_temp`, `pull`, `discard`.
  `Releases::latest`, `download`, `install` (replaces the running binary),
  `discard`.
- **Subprocesses:** `CommandRunner::run` runs the wrapped status-line command
  (`/bin/sh -c` in `statusline_auto`). `Releases::sha256` runs `shasum` /
  `sha256sum`, `Releases::unpack` runs `tar`, and `Releases::probe` runs the
  downloaded binary. `Editor::edit` opens `$EDITOR`. `AwsProfiles::names` and
  `region` run the `aws` program, which reads the AWS CLI config.
- **Reads only:** `TextFiles::read` (settings JSON), `PinLocator::pinned`,
  `PathInput`.

## Verification

- Run: `cargo test`
- Primary-adapter boundary only: `cargo test --test boundaries`
- Layer imports: `hexa analyze .`
- The integration tests for these use cases are in the repository's `tests/`
  directory: `profiles.rs`, `kits.rs`, `assets.rs`, `plugins.rs`,
  `migrate.rs`, `upgrade.rs`, `tui.rs`, `cli.rs`.
