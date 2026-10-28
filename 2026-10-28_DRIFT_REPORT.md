# Drift report: EGM contract READMEs (2026-10-28)

Status: resolved. All 27 findings are closed: 22 README corrections and the 4
code bugs behind the 5 `[code]` findings. Resolution record:
`goldfish_review_report.md`.

Scope: the eight contract files written by `egm-bootstrap` on 2026-10-28,
checked against the code by three fresh reviewers. Each finding below was
then checked by hand against the code it cites.

`[doc]` means the README overclaims or leaves something out; the README is
corrected. `[code]` means the code falls short of a reasonable contract; it
was fixed on 2026-10-28, with a test written first where one can be run.

## src/domain/README.md

- [doc] "`kit_name_for_profile` always returns a valid name and falls back to a
  `p-<name>` prefix." The fallback builds `KitName(format!("p-{k}"))` without
  `KitName::parse` (src/domain/mod.rs:618). It is valid only when the input is
  already a valid profile name. The one caller passes `ProfileName::as_str()`
  (src/usecases/mod.rs:457), so the precondition holds; the README must state it. **Status:** fixed; README corrected 2026-10-28.

## src/ports/README.md

- [doc] "imports from `crate::domain` only, and only for value types". The
  module also re-exports pure domain functions: `front_matter_description`,
  `manifest_field`, `resolve_kit`, `library_tree`, `pick`, `parse_profile_env`
  (src/ports/mod.rs:13-18). **Status:** fixed; README corrected 2026-10-28.
- [doc] "A primary adapter sees only this trait." The TUI imports and calls
  `library_tree`, `pick` and `resolve_kit` and several value types from
  `crate::ports` (src/adapters/primary/tui.rs:8). **Status:** fixed; README corrected 2026-10-28.
- [doc] `Library` is described as "Skills and agents kept in folders". It also
  holds plugins (`Kind::Plugin`; `main_file` returns `plugin.json`,
  src/ports/mod.rs:59; src/adapters/secondary/fs_library.rs:57, :133, :158). **Status:** fixed; README corrected 2026-10-28.
- [doc] Purpose rows omit methods: `Releases::read` (src/ports/mod.rs:134),
  `Library::main_file` (:60), `ProfileStore::exists`, `clear_default`,
  `env_file`, `write_env` (:25, :31, :35, :37). **Status:** fixed; README corrected 2026-10-28.

## src/usecases/README.md

- [doc] "The functions here do no I/O themselves." `library_add_source` calls
  `Path::exists` directly (src/usecases/mod.rs:541). **Status:** fixed; README corrected 2026-10-28.
- [doc] `resolve_name` returns `CcsError::NoneResolved` when no source gives a
  name (src/usecases/mod.rs:24); the README names only `NoProfile`. **Status:** fixed; README corrected 2026-10-28.
- [code] "the unpacked binary reports the expected version". The check is a
  substring test, `said.contains(&version)` (src/usecases/mod.rs:230): a binary
  that prints `0.5.10` passes a check for `0.5.1`. The checksum check runs
  first, so the risk is low, but the guard is weaker than it reads. **Status:** fixed in code 2026-10-28: the last word of the output must equal the version. Test: `a_new_binary_whose_version_only_starts_the_same_is_not_installed` (red before the fix).
- [doc] The Subprocesses list omits `Releases::sha256` (`shasum`/`sha256sum`)
  and `Releases::unpack` (`tar`) (src/usecases/mod.rs:224, :228). `AwsProfiles`
  is listed as read-only config, but it runs the `aws` program
  (src/adapters/secondary/aws_cli.rs:17). **Status:** fixed; README corrected 2026-10-28.

## src/README.md

- [doc] "`lib.rs` and `main.rs` are the only files that print." `main.rs` prints
  nothing (src/main.rs:1-15). The TUI writes terminal control to stdout
  (src/adapters/primary/tui.rs:2180-2186). **Status:** fixed; README corrected 2026-10-28.
- [doc] The environment list is incomplete. `lib.rs` passes the whole
  environment to the status line (`std::env::vars()`, src/lib.rs:264, :274)
  and reads the working directory (src/lib.rs:105, :115). **Status:** fixed; README corrected 2026-10-28.
- [doc] "calls one use case per command". `kit list` calls the port directly
  (src/lib.rs:399); `version` and `help` call no use case
  (src/lib.rs:245-248, :300-302). **Status:** fixed; README corrected 2026-10-28.

## README.md (blueprint sections)

- [doc] "A profile's `config/` holds only login state and history". ccs writes
  `config/settings.json` (src/adapters/secondary/fs_store.rs:148-150) and
  `--from` copies it (fs_store.rs:57). **Status:** fixed; README corrected 2026-10-28.
- [doc] The external programs list omits `$SHELL`, which the TUI's profile
  shell runs by exec (src/lib.rs:151-153). **Status:** fixed; README corrected 2026-10-28.
- [code] "the new binary reports the expected version": the same substring
  check as in src/usecases/README.md. **Status:** fixed in code 2026-10-28 (same fix and test as the usecases item).

## src/adapters/README.md

- [doc] The environment list omits `HOME`, `CCS_REPO`, `SHELL` and the `vi`
  fallback (src/lib.rs:70, :99, :151-153, :250). **Status:** fixed; README corrected 2026-10-28.
- [doc] "`ExecLauncher` (`exec`s `claude`)". It also execs `$SHELL` for
  `Exit::Shell` (src/lib.rs:149-153). **Status:** fixed; README corrected 2026-10-28.

## src/adapters/primary/README.md

- [doc] "On Home … `q` quits." `Esc` also quits on Home
  (src/adapters/primary/tui.rs:2023; test `q_and_esc_quit_from_home`). **Status:** fixed; README corrected 2026-10-28.
- [code] "Before an edit, the editor gets the real terminal." An edit queued by
  a mouse double-click does not run until the next key press: `drive` calls
  `state.edit(b)` only in the key branch (tui.rs:2201-2203), not the mouse
  branch (tui.rs:2206-2217). The next key is applied to the state first. **Status:** fixed in code 2026-10-28: `drive` runs a queued edit after a mouse event too. No test: the loop reads the real terminal; checked by reading the code.
- [doc] "create profiles and kits, rename and remove them". `Browser` can rename
  and remove kits only, not profiles (src/ports/mod.rs:178-235). **Status:** fixed; README corrected 2026-10-28.
- [doc] Side effects omit `Browser::attach` (tui.rs:967) and
  `Browser::set_kits` (src/ports/mod.rs:217). **Status:** fixed; README corrected 2026-10-28.

## src/adapters/secondary/README.md

- [doc] "passes in every path and program name". `GhReleases` hard-codes `tar`
  and `shasum`/`sha256sum` (gh_releases.rs:68-69, :79). Adapters read `PATH`,
  `temp_dir()` and `current_exe()` themselves. **Status:** fixed; README corrected 2026-10-28.
- [code] "If any step fails, it removes the temporary file." A failed
  `fs::copy` returns through `?` and leaves `.ccs-upgrade-<pid>` behind
  (gh_releases.rs:102); only `set_permissions` and `rename` failures clean up. **Status:** fixed in code 2026-10-28: a failed copy removes the temporary file. No test: it needs a copy that fails part way; checked by reading the code.
- [code] "Profile creation is atomic." It is cleanup on failure, not atomic:
  `fs::remove_dir_all(&dir)` on `<name>/` (fs_store.rs:106-109). `exists()` is
  false for a folder with no `profile.env` or with the old `native` marker
  (fs_store.rs:92), so a failed create over such a folder deletes what was
  already in it, which can include login state in `config/`. **Status:** fixed in code 2026-10-28: a failed create removes only what it made. Tests: `a_failed_create_keeps_what_the_folder_held_before` (red before the fix), `a_failed_create_of_a_new_folder_leaves_nothing`.
- [doc] "`FsLegacy::retire` refuses any path under `claude_home`." The check is
  `Path::starts_with` (fs_legacy.rs:155): lexical, so a path that reaches
  `~/.claude` through `..` or a link is not caught. Callers pass paths from
  the legacy layout scan, so this is a limit to state, not a live bug. **Status:** fixed; README corrected 2026-10-28.
- [doc] "`available()` looks the program up on `PATH`." A program name with `/`
  is checked as a path (exec_launcher.rs:21-23). **Status:** fixed; README corrected 2026-10-28.
- [doc] `ProcessEditor`: the `vi` fallback (src/lib.rs:70) and the `Usage`
  error for an empty command (process_editor.rs:23) are not mentioned. **Status:** fixed; README corrected 2026-10-28.
