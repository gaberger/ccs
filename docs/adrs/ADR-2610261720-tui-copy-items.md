# ADR-2610261720: Copy skills, agents and CLAUDE.md between profiles from the TUI

**Status:** Superseded
**Superseded-By:** ADR-2610261643
**Date:** 2026-10-26
**Drivers:** The user asked to get more out of the TUI, starting with copying skills and agents. ADR-2610261255 left adding items from the TUI as follow-up work, because it needed a picker over targets.

## Context

`ccs skill add <name> --from <profile|shared>` already copies an item between
config dirs (`usecases::add_from`). It enforces ADR-2610261244: native is
read-only, add copies and never moves, add refuses to replace a real item but
replaces a link with an override, and changes to `shared/` reach every profile.

The TUI shows one profile's items on the Skills, Agents and CLAUDE.md tabs. To
copy an item you have to remember its name, quit, and type the command.

## Decision

1. **`c` on an item opens a "Copy to" picker.** It lists `shared` first, then
   every profile except the source and except native (rule 2: native is
   read-only). These are the only places a copy can go.
2. **Space marks targets; Enter copies.** Enter copies to every marked target.
   If none is marked, it copies to the highlighted one. Esc cancels and copies
   nothing.
3. **One copy per target, through the same use case as the command line.** A new
   method on the driving port, `Browser::copy(from, kind, item, to)`, calls
   `usecases::add_from`. So the TUI gets the same refusals and the same
   re-linking as `ccs … add --from`, with no logic of its own.
4. **Every outcome is reported.** A failure at one target does not stop the
   others. The footer lists the targets that worked and the reason for each
   one that did not, e.g. `copied skill 'x' to shared, work · lab: … already exists there`.
5. **Broken links cannot be copied.** The picker does not open for a broken
   link; the footer says why.

### Screen

```
┌ Profiles ─────────┐┌ Copy skill 'inspect-bgp' from native ─────────────────┐
│  claude-bedrock   ││ [x] shared                                            │
│▶ native (default) ││ [ ] claude-bedrock                                    │
│  work             ││ [x] work                                              │
└───────────────────┘└───────────────────────────────────────────────────────┘
 ↑↓ move · space mark · enter copy · esc cancel
```

## Consequences

- With native as the source, this is how a stock `~/.claude` skill reaches a
  Bedrock profile without touching `~/.claude`.
- Copying to `shared` while the item still lives in the source profile leaves
  the profile's own copy in place, so it overrides the shared one. Promoting an
  item to shared (copy it there, then replace the own copy with the link) is a
  separate action. It is not part of this ADR. It shipped as ADR-2610261902 (`s`).
- `Browser` gains one method. The fake in `tests/tui.rs` implements it.

## Implementation

- `ports/`: `Browser::copy`; `Target` re-exported for the adapter.
- `usecases/`: `App::copy` → `add_from`.
- `adapters/primary/tui.rs`: `Mode::Copy`, key `c`, picker drawing.
- Gate: `cargo test && hexa analyze . --grade A+`.

### Checked against the real machine (2026-10-26)

`ccs ui` ran in a real pseudo-terminal (110×20) against a throwaway `$CCS_HOME`
with profiles `lab` and `work`, and skill `demo` in `work`. Keys: ↓ → tab c
space ↓ space enter q. The picker title `Copy skill 'demo' from work` was drawn;
`shared/skills/demo` and `lab/config/skills/demo` both existed afterwards; exit
status 0. `lab` holds its own copy, not a link: the copy to shared linked it
in, then the copy to `lab` replaced that link with an override (ADR-2610261244,
rule 4).

Injected bugs, each caught by the named test and reverted:

| Injected bug | Test that failed |
|---|---|
| native offered as a target | `c_offers_shared_and_every_writable_profile_but_the_source` |
| the first failure stops the remaining copies | `a_failed_target_is_reported_and_does_not_stop_the_others` |
| marks ignored; only the highlighted target copied | `space_marks_several_targets_and_toggles` (and one more) |
| a broken link opens the picker | `a_broken_link_cannot_be_copied` |

Also fixed: `tests/cli.rs` let the shell's own provider variables through, so
`statusline_survives_a_missing_wrapped_script` failed when run from a Bedrock
shell. The helper now removes every `PROVIDER_VARS` entry.

## References

- ADR-2610261244: managing CLAUDE.md, skills and agents
- ADR-2610261255: the ratatui TUI

## Evidence

`cargo test 2>&1 | grep "test result" && hexa analyze . --grade A+ 2>&1 | grep "Architecture grade"` at a2f6ff9 with uncommitted changes on 2026-10-26 17:29 UTC:

```text
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.51s
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  ⬡ Architecture grade: A+ — score 100/100
```
