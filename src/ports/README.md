# ports

## Overview

`src/ports/mod.rs` holds the contracts between the layers of ccs. Each port is a
Rust trait that names what the application needs from the outside world, never
how it gets it. `ProfileStore`, for example, says nothing about symlinks or
directories, so a test can back it with a temp dir.

There are two kinds of port here:

- **Driven (secondary) ports**: 13 traits that `usecases/` calls and that
  `adapters/secondary/` implements (filesystem, git, `curl`, AWS CLI, processes).
- **Driving (primary) port**: `Browser`, which `usecases::App` implements and
  the TUI in `adapters/primary/` calls. A primary adapter reaches the
  application only through this trait. To draw its pages it also uses the
  re-exported value types and pure functions (`library_tree`, `pick`,
  `pick_in`, `rank`, `resolve_kit`).

The module also re-exports the domain value types that adapters need, under
port-local names, so an adapter imports nothing but `crate::ports`.

## Interface

### Driven ports (implemented in `src/adapters/secondary/`)

| Trait | Purpose | Implementer |
|---|---|---|
| `ProfileStore` | Profiles: names, exists, load, create (optionally from another, never copying its login), default profile (read, set, clear), folder, `profile.env` (path, write), `settings.json`, default kits | `FsProfileStore` (`fs_store.rs`) |
| `Library` | Skills and agents kept in folders, and plugins (no folders): list, classify a source, add, make/move/rename folders, read, main file, trash, plugin manifest name, plugin contents | `FsLibrary` (`fs_library.rs`) |
| `KitStore` | Kits (named picks from the library): create, rename, trash, `uses` (saved atomically, keeping comments), instructions, build into a plugin dir, write a launch's joined prompt, and `build_dir`/`prompt_path`, which only name those paths | `FsKitStore` (`fs_kits.rs`) |
| `Fetcher` | Clone a git URL into a temp folder, discard it, `git pull --ff-only` | `GitFetcher` (`git_fetcher.rs`) |
| `Legacy` | Layout left by earlier ccs versions and the stock `~/.claude`; read-only except `retire` | `FsLegacy` (`fs_legacy.rs`) |
| `PathInput` | Expand and tab-complete paths typed into the TUI | `FsPathInput` (`fs_paths.rs`) |
| `Editor` | Open a file in the user's editor and wait | `ProcessEditor` (`process_editor.rs`) |
| `Releases` | Latest release tag, download assets, read a downloaded file, SHA-256, unpack, probe, install the binary, discard | `CurlReleases` (`curl_releases.rs`) |
| `TextFiles` | Read a text file that may not exist | `FsText` (`fs_text.rs`) |
| `CommandRunner` | Run another program with stdin and capture stdout (wrapping a status line command) | `ProcessRunner` (`process_runner.rs`) |
| `AwsProfiles` | AWS CLI profile names and per-profile region | `AwsCli` (`aws_cli.rs`) |
| `PinLocator` | Profile pinned to the working directory | `FsPinLocator` (`fs_pins.rs`) |
| `Launcher` | Check Claude Code is available; replace this process with it | `ExecLauncher` (`exec_launcher.rs`) |

### Driving port (implemented in `src/usecases/mod.rs`)

- `Browser`: everything the TUI can ask of the application. Covers profiles
  (`profiles`, `overview`, `set_default`, `new_profile`, `edit_profile`,
  `set_kits`), kits (`kits`, `attach`, `new_kit`, `remove_kit`, `rename_kit`,
  `kit_uses`, `set_use`, `kit_instructions`, `edit_kit`), the library
  (`library`, `read`, `edit`, `add`, `mkdir`, `move_entry`, `rename_folder`,
  `remove`, `update_plugins`), path completion (`complete_path`), and the
  one-off commands (`needs_migrate`, `migrate`, `import_claude`,
  `aws_profiles`, `import_aws`), and for the column screen (ADR-2610281305)
  `kit_lines`, `load_plan` and `plugin_contents`. Implemented by `App` and called from
  `src/adapters/primary/tui.rs`.

### Re-exports from `crate::domain` (for adapters)

| Port name | Domain name |
|---|---|
| `Error` | `CcsError` |
| `Name` | `ProfileName` |
| `LoadedProfile` | `Profile` |
| `ProfileRow` | `ListEntry` |
| `ProfileOverview` | `Overview` |
| `Kit` | `KitName` |
| `Plan` | `LaunchPlan` |
| `ItemKind` | `Kind` |
| `LibraryItem` | `LibItem` |
| `Removed` | `LibraryRemoved` |
| `Entry` | `Use` |
| `OldItem`, `OldLayout`, `OldProfile` | `LegacyItem`, `LegacyLayout`, `LegacyProfile` |
| `AwsProfileRow`, `AwsImport` | `AwsRow`, `ImportOutcome` |
| `parse_profile_env` | `parse_env` |
| Same name | `front_matter_description`, `front_matter_name`, `pick_in`, `rank`, `Found`, `LineGives`, `LoadPlan`, `PlannedKit`, `PluginContents`, `PluginPart`, `manifest_field`, `resolve_kit`, `library_tree`, `pick`, `Pick`, `Finding`, `TreeRow` |

## Invariants

- `ports/` imports from `crate::domain` only (hexagonal rule 2): value types,
  plus the pure domain functions it re-exports for adapters (see the table
  above). It holds no implementations, no I/O and no adapter
  imports.
- Adapters import only `crate::ports`. When an adapter needs a domain type,
  add a re-export here rather than letting the adapter import `domain/`
  (rule 4). Renaming or removing a re-export breaks adapters, so trace
  consumers first (`hexa graph consumers src/ports`).
- Ports describe what is needed, not how. Do not put filesystem layouts,
  command lines or library types in trait signatures beyond `std::path` paths
  and domain types.
- The composition root (`src/lib.rs`) is the only place that wires an adapter
  to a port.
- Contract details callers rely on, as stated in the trait docs:
  - `ProfileStore::names`, `Library::items`, `Library::folders` and
    `PathInput::complete` return sorted results. `Library::items` sorts by
    folder, then name.
  - `ProfileStore::create` with `from` copies variables, kits and settings,
    never the login.
  - `ProfileStore::write_settings_if_absent` never overwrites an existing
    `settings.json` and returns whether it wrote.
  - `KitStore::rename` and `Library::rename_folder` refuse a name already in
    use.
  - `KitStore::remove`, `Library::trash` and `Legacy::retire` move to the
    trash whole rather than deleting. `Legacy::retire` removes a link instead.
  - `KitStore::build` rebuilds the kit plugin to hold exactly the given items.
  - `KitStore::build` and `write_prompt` write exactly to `build_dir` and
    `prompt_path`, which touch nothing, so a launch can be planned without
    building.
  - `KitStore::set_uses` replaces the file whole or not at all, and keeps its
    comments and blank lines; a line still picked keeps its place.
  - `Legacy` is read-only except `retire`. It never writes the stock
    `~/.claude`.
  - `Releases::install` replaces the running binary whole or not at all.
    `Releases::discard` runs after every download attempt, so nothing is left
    behind.
  - `Launcher::launch` replaces the current process and returns only on
    failure, so its return type is `CcsError`, not `Result`.
  - `PathInput::expand` treats a leading `~` as the home dir and resolves a
    relative path against the directory ccs started in. `complete` keeps a
    typed `~`, ends folders in `/`, and lists dotfiles only when the last part
    starts with `.`.
  - The `Browser::edit*` methods expect the caller to hand over the terminal
    first.

## Side Effects

- None in this module. It only declares traits and re-exports.
- The traits describe side effects that implementers perform: filesystem
  writes under the ccs profile, kit and library folders and the trash
  (`ProfileStore`, `Library`, `KitStore`, `Legacy::retire`); git clones and
  pulls (`Fetcher`); network downloads and replacing the installed binary
  (`Releases`); spawning processes (`Editor`, `CommandRunner`,
  `Releases::probe`); reading AWS CLI config (`AwsProfiles`); and
  `exec` into Claude Code (`Launcher::launch`).

## Related decisions

- `docs/adrs/ADR-2610261643-logins-kits-library-sync.md`: profiles, kits,
  library, legacy layout, TUI rows and keys.
- `docs/adrs/ADR-2610261255-ratatui-tui.md`: the `Browser` driving port.
- `docs/adrs/ADR-2610261933-tui-shared-row-add-path-share-folder.md`:
  `PathInput`.
- `docs/adrs/ADR-2610271000-release-and-upgrade.md`: `Releases`.
- `docs/adrs/ADR-2610261238-import-aws-profiles.md`: `AwsProfiles`.

## Verification

- Run: `cargo test`
- Boundary check: `hexa analyze .`
