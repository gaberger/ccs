# Drift report: ccs, 2026-10-27

The EGM drift check (`egm_drift_report`, google/elephant-goldfish-model) over `docs/architecture.md`, `README.md` and the 10 ADRs in `docs/adrs/`, against `main` at `f27bbc2`. Each section lists only the differences.

The four differences that are bugs in the program, not only in a document:

1. `ccs --help` does not list plugins, `library add <owner/repo>` or `library update`.
2. `ccs sync` is not reserved, so it runs `claude sync`.
3. A failed `ccs upgrade` leaves its download folder in the temp folder.
4. `HELP` says that a bare `ccs` opens the picker only "when no profile is chosen"; it opens Home on any terminal.

## docs/architecture.md

- L112-113 says that `src/lib.rs` is the only file that reads the process environment. That is false: `exec_launcher.rs:24-25` reads `PATH`, and `gh_releases.rs:39,97` and `git_fetcher.rs:42` use `temp_dir()` and `current_exe()`.
- L115-116, step 1, lists `HOME`, `CCS_HOME` and `CCS_*_BIN`. `dispatch` also reads `CCS_PROFILE` (`lib.rs:106`), `CCS_REPO` (`lib.rs:248`), `VISUAL`/`EDITOR` (`lib.rs:68`) and `SHELL` (`lib.rs:149`).
- L117, step 2, says that `lib.rs` "builds each adapter once". `GhReleases` is built in the `Upgrade` branch (`lib.rs:248`), `ProcessRunner`/`FsText` on each status line call (`lib.rs:263,273`), and a second `ExecLauncher` for the TUI shell (`lib.rs:151`).
- The public factory functions in the composition root (`profile_store`, `library`, `kit_store`, `fetcher`, … at `src/lib.rs:37-74`) are not in the doc, although tests call them (`tests/assets.rs:127`).
- L97-99 says that each `App` method delegates to a use case. `kits`, `kit_uses`, `kit_instructions` and `complete_path` call ports directly (`src/usecases/mod.rs:821-823, 836-838, 846-848, 885-887`).
- L189-191 names only `Exit::Launch(name)`. `Exit` also has `Quit`, `LaunchWith` and `Shell` (`tui.rs:141-149`; `lib.rs:135-153`). The terminal is restored by `close_terminal` in `tui.rs`, not by `lib.rs`.
- L185-186 says that `run` is the only part that touches a terminal. The work is in the private `drive`, `open_terminal` and `close_terminal` (`tui.rs:2173-2222`), and `drive` opens the terminal again after an editor hand-off.
- L43-44 says that `tests/boundaries.rs` fails on any import outside `ports`. It is a deny list of three paths (`tests/boundaries.rs:10`), and it scans all of `src/adapters/primary`.
- L147-150, step 5, gives `with_kits` the `--settings` and `--plugin-dir` work. `with_kits` only adds `CCS_KITS` (`src/domain/mod.rs:752-759`); `prepare_launch` builds the arguments (`src/usecases/mod.rs:85`).
- Step 4 does not mention the warning about a kit/plugin name clash (`usecases/mod.rs:77-80`), or that an unknown kit name stops the launch (`usecases/mod.rs:61`).
- The "On disk" tree (L160-173) leaves out `$CCS_HOME/.default`, `.build/<kit+kit…>.md` and `kits/<kit>/uses`.
- L224 says that CI runs on every push. It runs on pushes to `main`, pull requests and `workflow_call` (`ci.yml:6-9`).
- The L220-221 gate leaves out `--exit-code` and the "0 passed" guard that CI uses (`ci.yml:38-42, 57`).

## README.md

- **`ccs --help` does not know about plugins.** The README lists `library list [skills|agents|plugins]`, `add <path|owner/repo|git URL>` and `update`. The built-in `HELP` (`src/domain/mod.rs:1103-1135`) still prints `library list [skills|agents] | add <path>`, has no `update`, and leaves `plugins/<name>` out of its entry note. Verified with `cargo run -- --help`. `HELP` also says that a bare `ccs` opens the picker "when no profile is chosen"; it opens Home on any terminal (`lib.rs:141-144`).
- L203-214, Status line: every launch now passes `--settings` with `ccs statusline --auto` (`src/usecases/mod.rs:85`), so every profile shows the segment, and `--auto` already chains to the project's or profile's own status line (`usecases/mod.rs:239-259`). Wrapping a script by hand is no longer needed. `--auto` is not mentioned anywhere, and the Usage list (L79-101) does not have it.
- L259, How It Works, lists `--plugin-dir` per kit only. The launch also adds `--settings`, `--append-system-prompt-file` and a `--plugin-dir` for each picked plugin (`usecases/mod.rs:81, 85`).
- L217-218, Migrating, says that the folder and kit get the profile's name. `kit_name_for_profile` lowercases the name, maps `_` and `.` to `-`, and can add a `p-` prefix (`src/domain/mod.rs:609-612`).
- L275-286, the Environment table, leaves out `SHELL` (`lib.rs:149`) and `HOME`, which is required (`lib.rs:97`).
- L248, File Structure, leaves out `tests/assets.rs` and `tests/boundaries.rs`. L249 says "CI (the gate on every push)"; it runs on `main` and pull requests only.
- L165-178, the key table, leaves out `n` and `i` on Home (`tui.rs:932-934`), `j`/`k`/`g`/`G` (`tui.rs:48-53`) and Ctrl-C.
- L30, "Rust 1.88+": `Cargo.toml` has no `rust-version`, so nothing enforces it.

## ADR-2610261212-profile-per-config-directory.md

- Superseded by ADR-2610261643: the `native` profile and its marker, `shared/` and its links, `ccs link` (now `Gone`), `ccs import` as "add ~/.claude as native", Rule 4 ("linking never destroys"), the relink in `ccs new --from`, and a bare `ccs` falling through to the default. The evidence tests `linking_never_replaces_a_real_file` and `cloning_copies_env_settings_and_own_skills_but_relinks_shared` are gone. The replacement test is `cloning_copies_variables_kits_and_settings_never_login_or_items` (`tests/profiles.rs:84`).
- **The Status is still Proposed.** The ADR says that it "stays Proposed until that result [the Keychain check] is recorded here". No result is recorded, but the code ships the decision, and later accepted ADRs depend on it.
- Rule 2 lists the variables that ccs unsets. The code unsets more: `CLAUDE_CODE_USE_VERTEX`, `CLAUDE_CODE_USE_FOUNDRY`, `ANTHROPIC_BASE_URL` and `ANTHROPIC_DEFAULT_{OPUS,SONNET,HAIKU}_MODEL` (`src/domain/mod.rs:13-26`).
- The launch plan is no longer "unset, set, then exec `claude`". Every launch also adds `--settings` and the kit arguments `--plugin-dir` and `--append-system-prompt-file` (`src/usecases/mod.rs:86`). The Decision section does not mention them.

## ADR-2610261237-statusline-segment.md

- The segment now also shows kits as `+kit` from `CCS_KITS` (`src/domain/mod.rs:897-899`). `--auto` falls back from `workspace.project_dir` to `workspace.current_dir`, then to `cwd` (`src/domain/mod.rs:960-972`). ADR-2610261643 §4 decides both, but this ADR does not cite it.

## ADR-2610261238-import-aws-profiles.md

No drift detected.

## ADR-2610261244-manage-claude-md-skills-agents.md

- Superseded by ADR-2610261643: the whole ADR, and its Status says so. `ccs skill|agent|claude-md` return `Gone`. The `AssetStore` port, `fs_assets.rs`, `Target`, `Origin` and `Command::Asset` no longer exist. The four evidence tests are gone. Only `Kind`, `default_asset_name` and `front_matter_description` remain, and the library uses them (`src/domain/mod.rs:243, 284, 297`).

## ADR-2610261255-ratatui-tui.md

- **The Status line is out of date.** It still says `**Status:** Accepted`, with no Superseded-By note. ADR-2610261643 §10 and §18 replaced its entry point, screen and key map. Only Decisions 3, 5 and 6, the `Browser` port and the boundary test still hold as written. The code points to the replacement (`src/adapters/primary/tui.rs:1`, `src/ports/mod.rs:174-175`).
- Superseded by ADR-2610261643 §10/§18: Decision 1, "A bare `ccs` keeps launching Claude Code". A bare `ccs` on a terminal now opens Home (`src/lib.rs:132, 160-161, 292-294`).
- Superseded by ADR-2610261643 §18: the screen (Profiles beside the Overview │ Skills │ Agents │ CLAUDE.md tabs) and its footer. It is now Home plus pages (`tui.rs:371-418, 1536`).
- Superseded by ADR-2610261643: the method list of Decision 2. `Browser` now works on library entries (`src/ports/mod.rs:176-226`).
- Superseded by ADR-2610261643 phase 4: Decision 4, delete "refuses `native`". There is no `native` profile now.
- `Exit` has variants that the ADR does not name: `Quit`, `LaunchWith { profile, kits }` and `Shell(String)` (`tui.rs:141-150`, `lib.rs:137-152`).
- The Consequences section says "Adding from a path is still command line only". This is false: `a` opens `Mode::Input` with `Browser::complete_path` and `Browser::add` (`tui.rs:570-603, 1089`, `ports/mod.rs:198, 207`).
- The injected-bug tests that the ADR names, `delete_asks_first_and_n_cancels` and `search_filters_the_focused_list`, are not in `tests/`.
- The gate and the Evidence disagree inside the ADR: the gate is `--grade A+`, but the Evidence ran `--grade A`.

## ADR-2610261720-tui-copy-items.md

- Superseded by ADR-2610261643 phase 4, which deleted `copy`, `shared/` and `native`: all of Decisions 1–5. `Browser` has no `copy` (`src/ports/mod.rs:176-226`), and `ccs skill …` returns `Gone` (`src/domain/mod.rs:335-338`).
- The `**Supersedes:**` header of ADR-2610261643 (lines 10-11) does not list this ADR, although this ADR says `Superseded-By: ADR-2610261643`. Only the phase 4 text (line 514) mentions it.
- The `tests/cli.rs` fix under "Also fixed" is still live (`tests/cli.rs:47-48, 214`), although the ADR is marked Superseded.

## ADR-2610261902-share-an-item.md

- Superseded by ADR-2610261643: all of Decisions 1–5. `share_asset`, `AssetAction::Share`, `Shared` and `Browser::share` are not in `src/`. None of the six tests that the ADR names is in `tests/`.

## ADR-2610261933-tui-shared-row-add-path-share-folder.md

- Superseded by ADR-2610261643: §1 (the `◆ shared` row, `Target`) and §3 (sharing a folder). The tests that the ADR names for those sections are not in `tests/`.
- **The Status line says that the whole ADR is superseded, but §2 is still live.** §2 (`a` adds from a path, the `PathInput` port) is in the code, which cites this ADR (`src/ports/mod.rs:113-120`, `src/adapters/secondary/fs_paths.rs:1-18`, `src/lib.rs:57-58`, `tests/assets.rs:88, 99`).
- §2 no longer describes the add path correctly. "`usecases::add_asset` with the name taken from the path" and the prompt `Add skill to work from:` are gone. `Browser::add(path, folder)` calls `library_add_source` into the library (`src/usecases/mod.rs:861-862`). The test `tab_completes_one_match_and_lists_several` is not in `tests/tui.rs`.
- `AssetStore::trash(root, rel)` does not exist. The only `trash` is `Library::trash(kind, path)` (`src/ports/mod.rs:67`).

## ADR-2610261643-logins-kits-library-sync.md

- **§7 sync is not built, and the ADR does not say so.** §7 is in the present tense ("`ccs sync init <remote>`; `ccs sync` commits…"), the Status is Accepted, and Implementation step 5 (l.557) only lacks the "**Done**" that the other steps have. There is no `Sync` port, no git sync adapter and no `.gitignore` writer. `sync` is not a reserved word (`src/domain/mod.rs:30-31`), so `ccs sync` goes through to `claude sync` (`src/domain/mod.rs:367`). Verified.
- §6 names `ccs skill|agent edit <name>`. That command is retired (`src/domain/mod.rs:327-330`); the edit path is `ccs library edit <entry>`.
- §8 says that every retired command names its replacement. `ccs link` names none: "kits are built at launch, so there is nothing to link" (`src/domain/mod.rs:335`).
- §5 puts a profile's own items in `library/<profile>/`. Migrate uses the kit name made from the profile name, which can differ (`src/usecases/mod.rs:453-454`).
- §4 lists `--plugin-dir` and `--append-system-prompt-file`. Every launch also passes `--settings` (`src/usecases/mod.rs:84`), and the Consequences section (l.511-513) still calls `--settings` "not decided".
- §14: the `t` key and the hint "`Folder: <dir>  t opens a shell here`" are gone. A profile page row, "Open a shell in its folder", replaced them (`tui.rs:985, 1678, 1685`). §18 does not say that `t` was removed.
- §12: the keys `e` (edit `profile.env`), `c` (copy) and `d` (default) on a profile are gone. They are rows on the profile page now (`tui.rs:926, 938-988`). §18 does not say so.
- §17: the frame titles (`ccs · ⬡ <profile>`, `profile <name>`, `kit <name>`) and the footer `⏎ launch …` were replaced by the breadcrumb and `3 · ⏎ start …` (`tui.rs:1462, 1585-1607`). §18 and §19 do not say that they replace §17.
- Phase 1 Evidence cites `edit_*` tests in `tests/cli.rs`, `tests/tui.rs` and `tests/assets.rs`. Only `tests/cli.rs` has them (`tests/cli.rs:324, 336`).
- The ADR counts 20 tests in `tests/kits.rs` (17 + 3). The file has 21; the extra one is `a_launch_sets_the_ccs_status_line_above_any_projects` (`tests/kits.rs:433`).
- §15 adds `library add <owner/repo|git URL>`, `library update` and `plugins/`. `ccs --help` has none of them (`src/domain/mod.rs:1126`), although the parser's own usage string has them (`src/domain/mod.rs:796`).

## ADR-2610271000-release-and-upgrade.md

- **A failed upgrade leaves its download folder behind.** Only `install` removes it (`src/adapters/secondary/gh_releases.rs:107-109`). A checksum mismatch, a missing `SHA256SUMS.txt` line, or a failed probe returns early (`src/usecases/mod.rs:220-231`). The Evidence counts "the download folder left behind" as caught, but its test covers the success path only. Verified.
- The ADR does not describe the release procedure in `.claude/skills/ccs-release/`. It never says that `Cargo.lock` must change with `Cargo.toml` because CI builds with `--locked`, that a release starts from a clean `main` equal to `origin/main`, or that CI on `main` must pass before the tag.
- §2 says only that a binary that cannot run is not installed. The code also refuses a binary whose `version` output does not hold the expected version (`src/usecases/mod.rs:229-231`).
