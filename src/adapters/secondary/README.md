# Secondary Adapters

## Overview

Secondary (driven) adapters implement the outbound port traits in
`src/ports/mod.rs` against the real world: the filesystem under `$CCS_HOME`,
the stock `~/.claude`, and the `aws`, `git`, `gh`, `tar`, `shasum` /
`sha256sum`, `claude` and editor programs. Each file holds one adapter struct.
Only the composition root (`src/lib.rs`) constructs these adapters, and it
passes in the paths and the main program names (`claude`, `aws`, `git`, `gh`,
the editor), so tests can point them at temporary folders and stub binaries
(`CCS_*_BIN` variables). `GhReleases` runs `tar` and `shasum` / `sha256sum`
by fixed name. Adapters also read `PATH`, `temp_dir()` and `current_exe()`
themselves.

## Interface

| File | Type | Implements | Purpose |
|---|---|---|---|
| `aws_cli.rs` | `AwsCli::new(program)` | `AwsProfiles` | `names()` runs `aws configure list-profiles`. `region(profile)` runs `aws configure get region --profile <p>`. |
| `exec_launcher.rs` | `ExecLauncher::new(program)` | `Launcher` | `available()` checks a program name that contains `/` as a path, and looks any other name up on `PATH`. `launch(&Plan)` replaces the current process with the program (Unix `exec`), using `Plan.args`, `unset`, `set` and `dir`. It returns an `Error` only if the exec fails. |
| `fs_kits.rs` | `FsKitStore::new(kits, build, trash)` | `KitStore` | Kits stored as `kits/<kit>/` folders, each with a `uses` file, an optional `CLAUDE.md` and an optional `.mcp.json`. Methods: `names`, `exists`, `create`, `remove`, `rename`, `uses`/`set_uses`, `instructions`/`set_instructions`/`instructions_file`. `build(kit, items)` assembles a Claude Code plugin under `.build/<kit>/`. `write_prompt(kits, text)` writes `.build/<a+b>.md`. |
| `fs_legacy.rs` | `FsLegacy::new(home, claude_home, trash)` | `Legacy` | Reads earlier ccs layouts and the stock `~/.claude`. `layout()` returns an `OldLayout` (the `shared/` folder, old profiles, and the `native` dir). `stock()` lists the items and `CLAUDE.md` in `~/.claude`. `read(path)` reads a file. `retire(path)` moves a path to the trash, or removes it if it is a link. |
| `fs_library.rs` | `FsLibrary::new(root, trash)` | `Library` | The item library: `skills/<folders…>/<skill>/SKILL.md`, `agents/<folders…>/<agent>.md` and `plugins/<plugin>/.claude-plugin/plugin.json`. Methods: `items`, `folders`, `add`, `classify`, `plugin_name`, `main_file`, `read`, `make_folder`, `move_to`, `rename_folder`, `trash`. |
| `fs_paths.rs` | `FsPathInput::new(home, cwd)` | `PathInput` | Paths typed into the TUI. `expand` handles `~` and resolves relative paths against `cwd`. `complete` lists completions and adds a trailing `/` to folders. |
| `fs_pins.rs` | `FsPinLocator::new(start)` | `PinLocator` | `pinned()` returns the first non-empty `.claude-profile` found in `start` or one of its parents. |
| `fs_store.rs` | `FsProfileStore::new(home)` | `ProfileStore` | Profiles stored as `<name>/profile.env`, `<name>/kits` and `<name>/config/` (the profile's `CLAUDE_CONFIG_DIR`), plus `$CCS_HOME/.default`. Methods: `folder`, `env_file`, `names`, `exists`, `load`, `create(name, from)`, `default_name`/`set_default`/`clear_default`, `kits`/`set_kits`, `write_env`, `write_settings_if_absent`. |
| `fs_text.rs` | `FsText` | `TextFiles` | `read(path)` returns `None` when a settings file is missing or unreadable. Used by the status line. |
| `gh_releases.rs` | `GhReleases::new(gh, repo)` | `Releases` | Backs `ccs upgrade`. Methods: `latest`, `download(tag, assets)` (into a temporary folder), `read`, `sha256`, `unpack` (`tar xzf`, takes out `ccs`), `probe` (runs `<bin> version`), `install` (replaces the running binary), `discard`. |
| `git_fetcher.rs` | `GitFetcher::new(program)` | `Fetcher` | `clone_temp(url)` makes a shallow clone into a fresh temporary folder. `pull(dir)` runs a fast-forward-only pull and reports the change. `discard(dir)` deletes a folder. |
| `process_editor.rs` | `ProcessEditor::new(command)` | `Editor` | `edit(path)` runs the editor command (`$VISUAL` / `$EDITOR`, default `vi`, chosen in `src/lib.rs`) on the file's canonical path and waits for it to exit. An empty command is `Error::Usage`. |
| `process_runner.rs` | `ProcessRunner` | `CommandRunner` | `run(argv, stdin)` runs a wrapped status-line command, sends `stdin` to it, and returns its stdout. |
| `mod.rs` | — | — | Declares the 13 modules above. |

## Invariants

- **Ports only.** Each adapter imports from `crate::ports` and `std` only. No
  adapter imports another adapter. Only `src/lib.rs` imports these modules
  (hexagonal rules 5–7, checked by `hexa analyze .`).
- **Nothing is erased; it is trashed.** `FsLibrary::trash`, `FsKitStore::remove`
  and `FsLegacy::retire` move items into `$CCS_HOME/.trash` under a name with a
  timestamp (`<secs>-library-…`, `<secs>-kit-…`, `<secs>-migrated-…`). The
  library and legacy trash names get `-1`, `-2`, … suffixes until the name is
  free. The exceptions: build output, temporary clones and download folders are
  deleted outright, and `retire` removes links instead of moving them.
- **`~/.claude` is never written.** `FsLegacy::retire` refuses any path that
  starts with `claude_home`. The check is lexical (`Path::starts_with`): it
  does not resolve `..` or links, so callers pass paths from the layout scan.
- **Credentials are never read.** `AwsCli` asks only for profile names and
  regions. The `profile.env` template says that no secrets belong there.
- **No shell evaluation.** `ProcessEditor` splits the editor command on
  whitespace and runs it directly. `profile.env` is parsed
  (`ports::parse_profile_env`) and never run by a shell.
- **A kit build is rebuilt from scratch.** `FsKitStore::build` deletes
  `.build/<kit>` first. In the build, skills are symlinks to their library
  folder and agents are copied, because Claude Code does not read an agent
  through a link.
- **A profile is a dir with `profile.env` and no `native` marker.** A dir that
  has the old `native` marker is legacy, not a profile.
- **A failed profile create removes only what it made.** It is not atomic.
  If `FsProfileStore::create` fails and the `<name>/` dir is new, it removes
  the dir. If the dir was there before, it removes only the files this call
  created (`profile.env`, `kits`, `config/settings.json`) and a new, empty
  `config/`. Anything that was there before, such as a login, stays. A clone (`from`) copies `profile.env`, `kits` and
  `config/settings.json` only, never login state or history.
- **`settings.json` is never replaced.** `write_settings_if_absent` opens the
  file with `create_new` and returns `false` if the file exists.
- **No partial binary is installed.** `GhReleases::install` copies the new
  binary to `.ccs-upgrade-<pid>` beside the running one, sets its mode to
  `0755`, and renames it over the running binary. If any step fails,
  the copy included, it removes the temporary file. A failed `download` removes its folder.
- **Name collisions are errors.** `add`, `move_to` and `rename_folder` return
  `Error::AssetExists`. `FsKitStore::rename` returns `Error::Usage`.
- **Missing or unreadable files read as absent.** This applies to
  `FsText::read`, kit instructions, the `kits` and `uses` files, and the pin
  file. A malformed `uses` line is an `Error::Io` that names the file.
- **The child process cannot deadlock on its pipes.** `ProcessRunner` writes
  stdin from a separate thread.
- **Git never asks for input.** `GitFetcher` sets `GIT_TERMINAL_PROMPT=0` and
  closes stdin.
- **Unix only.** `exec_launcher.rs`, `fs_kits.rs` and `gh_releases.rs` use
  `std::os::unix`.

## Side Effects

- **Filesystem under `$CCS_HOME`:** profile dirs, `.default`, `kits/`,
  `library/`, `.build/`, `.trash/`, and old-layout paths during migration.
- **Filesystem elsewhere:** reads `~/.claude` (never writes it). Reads the
  `.claude-profile` pin files in the working directory and its parents. Lists
  directories for path completion. Creates temporary dirs in the system temp
  folder (`ccs-clone-*`, `ccs-upgrade-*`). Replaces the running `ccs`
  executable on upgrade.
- **Processes:** runs `aws`, `git` (network: clone and pull), `gh` (network:
  release view and download from the private repo, default `gaberger/ccs`),
  `tar`, `shasum` / `sha256sum`, the downloaded `ccs version`, the user's
  editor (in the foreground, attached to the terminal) and the status-line
  command.
- **Process replacement:** `ExecLauncher::launch` replaces the `ccs` process
  with `claude` (or a shell). Signals, the TTY and the exit code then belong to
  that program.

## Verification

- Run: `cargo test`
- Adapter-focused integration tests live in `tests/` (`profiles.rs`, `kits.rs`,
  `assets.rs`, `plugins.rs`, `migrate.rs`, `upgrade.rs`, `cli.rs`).
- Boundary rules: `hexa analyze .`
