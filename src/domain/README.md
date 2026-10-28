# domain

## Overview

`src/domain/mod.rs` holds the rules of `ccs`, the Claude Code profile switcher,
as pure data and pure functions. It parses the `ccs` command line, validates
profile, kit and library names, parses `profile.env`, resolves which profile a
launch uses, plans the environment a launch sets, works out which library items
a kit picks, and formats the status line, doctor findings, and release names.

It is the innermost layer of the hexagon (`domain <- ports <- usecases`, see
`src/lib.rs`). It does no disk, process, or network I/O, so every rule here can
be tested without wiring anything up. `src/ports/mod.rs` re-exports the domain
types that adapters need, and `src/usecases/mod.rs` and `src/lib.rs` (the
composition root) call the functions directly.

The ADRs cited below live in `docs/adrs/` at the repository root.

## Interface

### Errors

- `CcsError`: the one error type of the crate. Variants: `BadName`,
  `EnvSyntax { line }`, `NoProfile`, `Exists`, `NoneResolved`, `Usage`, `Io`,
  `AssetExists`, `NoAsset`, `NoKit`, `Gone` (a retired command, with what
  replaced it). Its `Display` impl is the message the user sees.

### Profiles and launch planning

- `PROVIDER_VARS: &[&str]`: the provider variables cleared before a profile's
  own variables apply (ADR-2610261212, rule 2). Secrets are deliberately absent.
- `ProfileName::parse(&str) -> Result<ProfileName, CcsError>`, `as_str()`: a
  path-safe profile name.
- `Profile { name, env, config_dir }`: a profile as loaded.
- `parse_env(&str) -> Result<Vec<(String, String)>, CcsError>`: parses
  `profile.env`.
- `resolve(explicit, pinned, env, default) -> Option<String>`: picks the
  profile for a launch.
- `LaunchPlan { unset, set, args, dir }` and
  `plan_launch(&Profile, Vec<String>) -> LaunchPlan`.
- `render_exports(&LaunchPlan) -> String`: the plan as POSIX shell, for
  `eval "$(ccs env <profile>)"`.
- `ListEntry` (one row of `ccs list`) and `Overview` (what the TUI shows for one
  profile, ADR-2610261255).

### Command line

- `Command`: every top-level verb, including `Launch { profile, kits, args }`,
  `Statusline { wrap }`, `StatuslineAuto`, `Upgrade { version, check }`,
  `ImportAws { profiles, all, region }`, `Migrate`, `Library(LibraryCmd)` and
  `Kit(KitCmd)`.
- `LibraryCmd`: `List`, `Add`, `Mkdir`, `Move`, `Rename`, `Remove`, `Show`,
  `Edit`, `Update`.
- `KitCmd`: `List`, `Edit`, `New`, `Remove`, `Show`, `Use`, `Drop`, `Rename`,
  `Attach`, `Detach`.
- `parse_command(&[String], is_profile: impl Fn(&str) -> bool) -> Result<Command, CcsError>`:
  parses the arguments after `ccs`. The caller supplies `is_profile`, so the
  parser itself never looks at the disk.
- `HELP: &str`: the text that `ccs help` prints.

### The library and kits (ADR-2610261643)

- `Kind` (`Skill`, `Agent`, `Plugin`), with `label()` and `dir()` (`skills`,
  `agents`, `plugins`).
- `item_name(Kind, &str)`, `default_asset_name(Kind, &str)`,
  `front_matter_description(&str)`: item names and YAML front matter.
- `Use` (`Folder(Kind, String)` or `Item(Kind, String)`), with `parse`, `kind`,
  `line`: one line of a kit's `uses`, such as `skills/net/` or
  `skills/humanize`.
- `LibItem { kind, name, folder, description, source }`, with `path()` and
  `in_folder()`.
- `KitContents { items, missing }` and
  `resolve_kit(&[Use], &[LibItem]) -> KitContents`.
- `KitName::parse`, `as_str`, and `kit_name_for_profile(&str) -> KitName`.
- `TreeRow { path, depth, item }` (`leaf`, `entry`, `folder`),
  `library_tree(&[LibItem], &[String]) -> Vec<TreeRow>`, `Pick` (`Direct`,
  `Through(line)`, `No`), `pick(&[Use], Kind, &TreeRow) -> Pick`.
- `rewrite_folder(&[Use], Kind, from, to) -> Vec<Use>`: kit lines after a folder
  is renamed.
- `LibraryRemoved { trashed, kits }`.
- `kit_args(&[PathBuf], Option<&Path>) -> Vec<String>`: the `--plugin-dir` and
  `--append-system-prompt-file` arguments for `claude`.
- `with_kits(LaunchPlan, &[KitName], Vec<String>) -> LaunchPlan`.
- `manifest_field(json, key) -> Option<String>`: a top-level string field of a
  JSON object, such as a `plugin.json` `name`.
- `git_source(&str) -> Option<String>`: the git URL that a typed source stands
  for (`owner/repo` becomes a GitHub URL).

### Layouts left by earlier versions (ADR-2610261643 §5, §8)

- `LegacyItem`, `LegacyProfile`, `LegacyLayout` (with `is_empty()`): what
  `ccs migrate` finds and converts.

### Providers and the status line (ADR-2610261237)

- `provider_label(&[(String, String)]) -> String`: `bedrock <profile>@<region>`,
  `vertex`, `foundry`, `gateway` or `anthropic`.
- `status_segment(vars) -> Option<String>`: for example
  `⬡ work +net · bedrock sso@us-east-1`. Returns `None` when `CCS_PROFILE` is not
  set.
- `Wrapped` (`Nothing`, `Output`, `Failed`) and
  `compose_status(Option<String>, Wrapped) -> String`.
- `settings_statusline(&str)`, `next_statusline(&[Option<String>])`,
  `statusline_dir(&str)`: pick which status line runs after the segment, and in
  which folder.
- `launch_settings(ccs_bin)` and `default_settings(ccs_bin)`: the `--settings`
  JSON that every launch passes, and a fresh profile's `settings.json`.

### Releases and `ccs upgrade` (ADR-2610271000)

- `VERSION: &str`: `CARGO_PKG_VERSION`, fixed at compile time.
- `newer_version(a, b) -> bool`, `release_target(os, arch) -> Option<&str>`,
  `release_asset(version, target) -> String`,
  `checksum_for(sums, file) -> Option<String>`.

### AWS import (ADR-2610261238) and doctor

- `ccs_name_for_aws(&str) -> Result<ProfileName, CcsError>`,
  `aws_profile_env(aws, region) -> String`, `AwsRow { aws, imported_as }`,
  `ImportOutcome` (`Created`, `Skipped`; its `Display` impl gives the line
  printed by both the CLI and the TUI).
- `Finding` (`Pass`, `Warn`, `Fail`) and
  `check_env(&[(String, String)]) -> Vec<Finding>`: the doctor rules that need
  only a profile's variables.

### Subdirectories

- None. The module is the single file `mod.rs`.

## Invariants

- **Pure layer.** `domain` imports only `std` and itself (hexagonal rule 1). It
  never reads files, environment variables, or processes at runtime. Callers
  pass in anything that needs the outside world, such as the `is_profile`
  predicate given to `parse_command`.
- **Profile names** start with an ASCII letter or digit and contain only
  letters, digits, `.`, `_` and `-`. A name must not be one of the private
  `RESERVED` words (commands, retired commands, and ccs's own folders: `list`,
  `kit`, `library`, `sync`, `upgrade` and others). When you add a top-level
  command, add its word to `RESERVED` too, or an existing profile with that name
  becomes unreachable.
- **Kit names** start with a lowercase letter and contain only lowercase
  letters, digits and `-`. They cannot be `kit`, `kits`, `library` or `lib`.
  `kit_name_for_profile` returns a valid name when its input is a valid
  profile name, and falls back to a `p-<name>` prefix. It does not check its
  input, so pass it `ProfileName::as_str()` only.
- **Item and folder names** are path-safe: each segment uses the same character
  set as a profile name. Plugins have no folders (`plugins/<name>` only).
- **`profile.env` is data.** Lines are split on the first `=` and never
  evaluated. Blank lines and `#` comments are skipped, and a leading `export ` is
  dropped. One pair of matching outer quotes is removed. A key that is not
  `[A-Za-z_][A-Za-z0-9_]*` fails with `EnvSyntax` and its 1-based line number.
- **Resolution order** is argument, pin file, `$CCS_PROFILE`, then the default
  (ADR-2610261212, rule 1). Values are trimmed, and a blank value falls through
  to the next source.
- **Launch plan.** `plan_launch` always unsets every `PROVIDER_VARS` entry, then
  sets the profile's variables, then `CLAUDE_CONFIG_DIR` and `CCS_PROFILE`, in
  that order. `with_kits` puts kit arguments before the user's arguments, and
  sets `CCS_KITS` only when at least one kit is chosen.
- **Shell output is quoted.** `render_exports` single-quotes every value, so
  `eval` cannot run anything inside one.
- **Command parsing.** A first word that names a profile launches it. Leading
  `+kit` words choose the kits for that launch, and every other word goes to
  `claude` untouched. Retired commands (`skill`, `skills`, `agent`, `agents`,
  `claude-md`, `claudemd`, `link`) return `CcsError::Gone`, which names what
  replaced them. `sync` returns `Usage`, because it is decided but not built.
- **Kit resolution.** `resolve_kit` returns items in library order, each item
  once, and lists every `uses` line that matches nothing in the library. A
  folder line picks every item of the same kind at or under that folder.
- **Versions.** `newer_version` is false when either side is not `X.Y.Z` or
  `vX.Y.Z`, so an unreadable version is never treated as newer.
- **Status line.** `next_statusline` skips any command that is `ccs statusline`
  itself, unless it wraps another command with ` -- `, so a launch never wraps
  its own segment.

## Side Effects

- None. Every function is deterministic and returns new values. The module
  does no filesystem, network, process, or environment access. `VERSION` is read
  by the compiler (`env!`), not at runtime.

## Verification

- Run: `cargo test`
- Domain rules only: `cargo test --test domain` (`tests/domain.rs`, 40 tests).
  `hexa analyze .` checks that `domain/` imports nothing outside itself.
