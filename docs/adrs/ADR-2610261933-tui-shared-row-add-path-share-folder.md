# ADR-2610261933: The TUI browses shared/, adds from a path, and shares an agent folder

**Status:** Superseded
**Superseded-By:** ADR-2610261643
**Date:** 2026-10-26
**Drivers:** The three follow-ups left by ADR-2610261720 and ADR-2610261902 (workplan draft `draft-2610261329-tui-follow-ups-after-adr`). The user asked to finish them.

## Context

- The TUI lists profiles only. `shared/` is visible only as `→` links inside a
  profile, so a shared item cannot be viewed, copied or deleted at its source.
- Adding an item from a file or folder on disk is command line only
  (`ccs skill add <path>`). The TUI has no text input.
- `share` refuses a nested agent (`hex/rev`) because shared agents are linked
  one top-level folder at a time (`agents/hex`).
- The `Browser` port names a place with `&ProfileName`, which cannot say
  "shared".

## Decision

### 1. `shared` is a row in the profile list

- The list ends with a `◆ shared` row (when there is at least one profile). Its
  Overview tab says what shared means and how many items it has; its item tabs
  list `shared/` itself.
- `Browser` methods that act on items take a `Target` (`Profile(name)` or
  `Shared`) instead of a `ProfileName`. `Target::Resolved` is a usage error at
  this port. The same use cases run behind it, so deleting from `shared` trashes
  the item and re-links every profile, exactly like `ccs … rm --shared`.
- On the shared row: Enter does not launch and `d` does not set a default (the
  footer says shared is not a profile). `c` copies into a profile, which
  becomes an override. `s` says the item is already shared. The delete confirm
  says that every profile loses the item.

### 2. `a` adds from a path

- On an item tab, `a` opens an input line: `Add skill to work from: ▏`. Tab
  completes the path. One match is filled in, with `/` after a folder. Several
  matches complete to their common prefix and are listed in the body. Enter
  adds, Esc cancels.
- A new secondary port, `PathInput`, does the filesystem part: `expand` (a
  leading `~`, and paths relative to the directory `ccs` was started in) and
  `complete`. Its adapter is `fs_paths.rs`. The TUI reaches it only through
  `Browser::complete_path`. The add itself is `usecases::add_asset` with the
  name taken from the path, as `ccs … add <path>` does.

### 3. Sharing a nested agent shares its folder

- `ccs agent share hex/` (a trailing `/` names a folder) shares every agent
  under `agents/hex/`. A nested agent name without the `/` is refused with the
  folder command to run instead.
- In the TUI, `s` on a nested agent asks about the whole folder and names its
  agents: `Share agents folder 'hex/' (hex/a, hex/rev): move it from work into shared? y/n`.
- The order is the same as for one item. Copy each agent into
  `shared/agents/hex/` and re-link every profile. Move the profile's whole
  `agents/hex` folder to the trash in one piece, so files that are not agents
  are kept there too. Then link `agents/hex` back and check that every agent in
  the folder is now a link.
- It refuses, changing nothing, when: the folder holds no agents; any agent in
  it is a link or broken; `shared/` already has any agent under that folder.
- `AssetStore` gains `trash(root, rel)`, which moves any path under a config
  dir to the trash. `remove` keeps its meaning.

## Consequences

- Every `Browser` item method changes signature; the fake in `tests/tui.rs`
  follows.
- The TUI gains text input. Typed text never reaches a shell: it is a path
  handed to `add_asset`.
- Profiles that hold their own `agents/hex` folder keep it (an override) and are
  named, as for single items.

## Implementation

- `ports/`: `Browser` takes `&Target`; `Browser::add`, `Browser::complete_path`;
  `PathInput`; `AssetStore::trash`.
- `usecases/`: `share_asset` handles `folder/`; `App` maps a `Target` to a root.
- `adapters/secondary/fs_paths.rs`: `FsPathInput`; `fs_assets.rs`: `trash`.
- `adapters/primary/tui.rs`: the shared row, `Mode::Input`, folder confirm.
- `lib.rs`: wire `FsPathInput`.
- Gate: `cargo test && hexa analyze . --grade A+`.

### Checked against the real machine (2026-10-26)

`ccs ui` ran in a real pseudo-terminal (120×22), started in a throwaway `$HOME`
holding `~/src/demo/SKILL.md`, with profiles `lab` and `work` and the agent
`hex/rev` in `work`. Keys: ↓ → tab `a` `~/src/de` Tab Enter, then → `s` `y`,
then tab ↓ q. Exit status 0. Afterwards:

- `ccs skill list -p work` showed `demo  Demo skill`. The path was completed
  and added.
- `work/config/agents/hex` was a link to `shared/agents/hex`, `ccs agent list -p lab`
  showed `→ hex/rev`, and `.trash/` held `…-work-agents_hex`.

Injected bugs, each caught by the named test and reverted:

| Injected bug | Test that failed |
|---|---|
| a folder clash with shared/ not checked | `sharing_a_folder_refuses_and_changes_nothing` |
| a folder with linked agents shared anyway | `sharing_a_folder_refuses_and_changes_nothing`, `share_refuses_and_changes_nothing` |
| only one file of the folder trashed | `sharing_an_agent_folder_moves_it_whole_and_links_it_back` |
| `Target::Resolved` treated as shared | `the_app_adds_from_a_typed_path_and_browses_shared` (and one more) |
| no shared row | `moving_stops_at_the_ends` (and two more) |
| shared offered as a copy target from shared | `copying_from_shared_offers_writable_profiles_only` |
| Enter on shared launches | `enter_and_d_on_shared_neither_launch_nor_set_a_default` |
| a single completion not filled in | `tab_completes_one_match_and_lists_several` |
| a nested agent shared without its folder | `s_on_a_nested_agent_asks_about_its_whole_folder` |
| the "every profile loses it" warning dropped | `deleting_from_shared_says_every_profile_loses_it` |
| dotfiles always completed | `path_input_completes_as_typed_with_folders_ending_in_a_slash` |
| `~` expanded to the start dir | `path_input_expands_home_and_relative_paths` (and two more) |
| folders completed without `/` | `path_input_completes_as_typed_with_folders_ending_in_a_slash` (and one more) |

Two of my own test mistakes were fixed along the way. Neither was a code bug:
- A fixture wrote into `work`'s agent folder after it had become a link, so the
  file landed in `shared/`. The fixture now writes first and asserts it did not.
- A test expected `BadName` for a bad agent name. `asset_name` reports `Usage`.

## References

- ADR-2610261244, ADR-2610261255, ADR-2610261720, ADR-2610261902

## Evidence

`cargo test 2>&1 | grep "test result" && hexa analyze . --grade A+ 2>&1 | grep "Architecture grade"` at a2f6ff9 with uncommitted changes on 2026-10-26 19:49 UTC:

```text
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.60s
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  ⬡ Architecture grade: A+ — score 100/100
```
