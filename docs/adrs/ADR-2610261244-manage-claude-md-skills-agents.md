# ADR-2610261244: Manage CLAUDE.md, skills and agents per profile and shared

**Status:** Superseded
**Superseded-By:** ADR-2610261643
**Date:** 2026-10-26
**Drivers:** The user asked for `ccs` to manage CLAUDE.md, skills and agents: look them up, add them and delete them. Today that means hand-copying folders between `~/.claude-profiles/*/config/` and remembering which ones are symlinks.

## Context

ADR-2610261212 gives each profile its own config directory and a `shared/`
directory whose items are symlinked into every profile. Three kinds of item
matter here:

| Kind | Path in a config dir | Main file |
|---|---|---|
| skill | `skills/<name>/` | `skills/<name>/SKILL.md` |
| agent | `agents/<name>.md` (subfolders allowed: `agents/hex/<name>.md`) | the file |
| CLAUDE.md | `CLAUDE.md` | the file |

Skills and agents carry YAML front matter with a `description:`, which can be
plain, quoted, or a block (`|`, `>`).

Two facts of the current code block this feature:

1. `shared/agents` is linked into a profile as **one** symlink. Adding an agent
   to one profile would write into `shared/` and reach every profile.
2. `ccs link` re-links every shared item. A profile that removed a shared skill
   would get it back at the next `ccs link`.

## Decision

Add one command family per kind:

```
ccs skill     list | show <name> | add <path> [--name <n>] | add <name> --from <p|shared> | rm <name> | find <text>
ccs agent     (same)
ccs claude-md list | show | add <file> | rm
```

Each takes a target: `-p <profile>`, `--shared`, or neither (the resolved
profile, same order as launching). `find` and `claude-md list` look in every
profile and `shared/`.

### Rules

1. **Per-item links for agents too.** `skills/` and `agents/` in a profile are
   real directories that hold per-item links into `shared/`. `ccs link`
   converts an old whole-directory `agents` link in place.
2. **Native is read-only.** The `native` profile is the stock `~/.claude`.
   `ccs` lists, shows, searches and copies *from* it, and refuses to add to it or
   remove from it. This keeps the promise of ADR-2610261212.
3. **Add copies, never moves.** A skill source is a directory containing
   `SKILL.md` (or the `SKILL.md` itself); an agent or CLAUDE.md source is a file.
   The name defaults to the folder name (skill) or file stem (agent).
4. **Add never replaces a real item.** If the name exists as a real file, add
   fails and says to remove it first. If it exists as a link to `shared/`, add
   replaces the link with the profile's own copy — an override.
5. **Delete is recoverable.** `rm` of a real item moves it to
   `$CCS_HOME/.trash/<time>-<target>-<kind>-<name>` and prints the path. Nothing is
   erased.
6. **Opting out of a shared item.** `rm` of a shared link inside a profile
   removes the link and records the item in the profile's `unshared` file.
   `ccs link` skips what that file lists. Adding the item to the profile again
   removes the record.
7. **Shared changes reach every profile.** After `add --shared` and
   `rm --shared`, `ccs` re-links all profiles. Linking also removes links into
   `shared/` whose target is gone.
8. **Names are path-safe.** Each `/`-separated segment starts with a letter or
   digit and holds only letters, digits, `.`, `_`, `-`. Skills take one segment.

## Consequences

- A skill used only on Bedrock lives in that profile; a skill used everywhere is
  `ccs skill add ./my-skill --shared`.
- `ccs skill add inspect-edge --from native -p bedrock-work` brings one skill
  from the stock `~/.claude` into a profile without touching `~/.claude`.
- `.trash/` grows until the user empties it. That is the cost of recoverable
  deletes, and the path is printed each time.
- A plugin's skills and agents are not managed here; plugins have their own
  install path.

## Implementation

- `domain/`: `Kind`, `Target`, `Origin`, `Asset`, `asset_name`, `rel_path`,
  `default_asset_name`, `front_matter_description`, `Command::Asset`.
- `ports/`: `AssetStore` (list, read, add, remove — on a config directory);
  `ProfileStore::asset_root`, `set_unshared`.
- `adapters/secondary/fs_assets.rs`: `FsAssetStore`.
- `fs_store.rs`: per-item agent links, `unshared` file, pruning of dead links.
- Gate: `cargo test && hexa analyze . --grade A`.

### Checked against the real machine (2026-10-26)

In a throwaway `$CCS_HOME` with `native` imported from the real `~/.claude`:
`ccs agent list -p native` listed the real agents, including nested ones;
`ccs skill find bgp` found three native skills; `ccs skill add inspect-edge
--from native -p work` copied the skill in; `ccs skill rm inspect-edge -p work`
moved it to `.trash`; `ccs skill rm inspect-edge -p native` was refused with
exit 1. The number of files under `~/.claude` changed during the run was 0.

Injected bugs, each caught by the named test and reverted:

| Injected bug | Test that failed |
|---|---|
| `rm` erases instead of moving to trash | `removing_an_own_item_moves_it_to_trash` |
| The native profile becomes writable | `native_can_be_listed_but_not_changed` |
| An opt-out is not recorded | `removing_a_shared_link_from_one_profile_opts_it_out_for_good` |
| Dead shared links are not pruned | `removing_a_shared_item_removes_its_links_everywhere` |

## References

- ADR-2610261212 — one config directory per profile
- Claude Code skills: https://docs.claude.com/en/docs/claude-code/skills
- Claude Code subagents: https://docs.claude.com/en/docs/claude-code/sub-agents

## Evidence

`cargo test 2>&1 | grep "test result" && hexa analyze . --grade A 2>&1 | grep "Architecture grade"` at a2f6ff9 with uncommitted changes on 2026-10-26 16:55 UTC:

```text
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.46s
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  ⬡ Architecture grade: A+ — score 100/100
```
