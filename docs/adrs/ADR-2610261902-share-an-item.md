# ADR-2610261902: Share an item: move a profile's own copy into shared/ and link it back

**Status:** Superseded
**Superseded-By:** ADR-2610261643
**Date:** 2026-10-26
**Drivers:** After ADR-2610261720, copying an item to `shared` from the TUI leaves the source profile's own copy in place, so the source profile overrides the shared item it just made. The user asked for the follow-up: one action that promotes an item to shared.

## Context

ADR-2610261244 links each shared item into every profile, one link per item,
unless the profile already holds a real item with that name (an override) or
has opted out (`unshared`). Shared links are made per top-level entry under
`skills/` and `agents/`. So a nested agent `hex/rev` is shared through one link
`agents/hex`, not through a link per file.

Promoting by hand is three steps: `ccs skill add <n> --from <p> --shared`,
`ccs skill rm <n> -p <p>`, `ccs link`.

## Decision

1. **`ccs skill|agent|claude-md share <name> [-p <profile>]`** and the TUI key
   `s` (after a y/n confirm) run one use case, `share_asset`.
2. **What it does, in order:** copy the profile's own item into `shared/` (this
   re-links every profile), move the profile's copy to the trash, then link the
   shared item back into the profile. The profile ends up holding a link. Its
   old copy is recoverable from the trash, like every `rm`.
3. **It refuses, and changes nothing, when:**
   - the profile is `native` (read-only, ADR-2610261244 rule 2);
   - `--shared` is the target (shared is the destination, not a source);
   - the item is already a link, or a broken link;
   - `shared/` already has an item with that name (remove it there first);
   - the item is a nested agent (`a/b`): shared agents are linked by top-level
     folder, so one file in a profile's real folder cannot become a link.
4. **It checks its own result.** If the profile's item is not a link to
   `shared/` at the end, the use case fails and says where the old copy is in
   the trash. It never reports success it did not observe.
5. **Other profiles' overrides are named.** A profile that holds its own copy
   with the same name keeps it (rule 4 of ADR-2610261244). The result lists
   those profiles so the user knows the shared copy does not reach them.

## Consequences

- The TUI's `c` copies and `s` shares. Copy-to-shared stays as it is: it is the
  right action when the profile should keep its own version.
- `Browser` gains `share`; `domain` gains `AssetAction::Share` and `Shared`.
- Sharing a nested agent is still done by hand. Sharing a whole agent folder
  would need a kind of item the asset model does not have yet; not part of this
  ADR. Shipped in ADR-2610261933 as `share <folder>/`.

## Implementation

- `domain/`: `AssetAction::Share`, `Shared { trashed, overridden }`, parse `share`.
- `ports/`: `Browser::share`; re-export `Shared`.
- `usecases/`: `share_asset`; `App::share`.
- `lib.rs`: `ccs … share` output.
- `adapters/primary/tui.rs`: key `s`, the confirm generalised to delete or share.
- Gate: `cargo test && hexa analyze . --grade A+`.

### Checked against the real machine (2026-10-26)

`ccs ui` ran in a real pseudo-terminal (110×20) against a throwaway `$CCS_HOME`
with profiles `lab` and `work`, and skill `demo` in `work`. Keys: ↓ → tab s y q.
Exit status 0. Afterwards `work/config/skills/demo` and `lab/config/skills/demo`
were both links to `shared/skills/demo`, `.trash/` held `…-work-skill-demo`,
and `ccs skill list -p work` showed `→ demo  Demo skill`.

Injected bugs, each caught by the named test and reverted:

| Injected bug | Test that failed |
|---|---|
| nested agents allowed | `share_refuses_and_changes_nothing` |
| the end result not checked | `share_fails_loudly_when_the_link_did_not_appear` |
| the profile not re-linked | `sharing_moves_the_own_copy_into_shared_and_links_it_back`, `claude_md_can_be_shared` |
| overrides not named | `sharing_moves_the_own_copy_into_shared_and_links_it_back` |
| TUI `y` deletes instead of sharing | `s_then_y_shares_and_names_the_overrides` |
| TUI asks to share a link | `s_on_a_linked_item_says_it_is_already_shared` |

One probe survived: removing an early "shared/ already has it" check changed
nothing. `add_asset` already refuses a real item before its first change, so the
check was redundant. It was deleted; the refusal and the "nothing moved"
assertion still hold.

## References

- ADR-2610261244: managing CLAUDE.md, skills and agents
- ADR-2610261720: copying from the TUI

## Evidence

`cargo test 2>&1 | grep "test result" && hexa analyze . --grade A+ 2>&1 | grep "Architecture grade"` at a2f6ff9 with uncommitted changes on 2026-10-26 19:26 UTC:

```text
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.44s
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  ⬡ Architecture grade: A+ — score 100/100
```
