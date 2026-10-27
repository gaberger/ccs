# ADR-2610261255: A ratatui TUI as the first primary adapter

**Status:** Accepted
**Date:** 2026-10-26
**Drivers:** The user asked for the switcher to use ratatui (https://ratatui.rs/). With profiles, skills, agents and CLAUDE.md all managed by `ccs`, the command line asks the user to remember names; a screen can show them.

## Context

Every `ccs` feature so far is a command line verb wired in the composition root
(`lib.rs`). There is no primary adapter, and no dependency outside `std`.

ratatui 0.30.2 draws to the terminal through a backend. It re-exports the
`crossterm` it was built against as `ratatui::crossterm`, and ships a
`TestBackend` that draws into a memory buffer.

hexa rule 4: a primary adapter imports `ports/` only. So the TUI cannot call
`usecases::` directly; it needs a port that drives the application.

## Decision

1. **`ccs ui` opens the TUI.** A bare `ccs` keeps launching Claude Code, so
   scripts and muscle memory do not change.
2. **A driving port, `Browser`,** names everything the screen can do: list
   profiles, a profile overview (provider, variables, doctor findings), list
   items of a kind, read one, remove one, set the default. `usecases::App`
   implements it over the existing ports. The TUI sees only `Browser`.
3. **The TUI never launches.** Enter returns `Exit::Launch(name)`. The
   composition root restores the terminal, then launches `claude` exactly as
   `ccs <name>` does. Claude Code never starts inside a half-restored terminal.
4. **Deleting asks first.** `x` opens a confirm line; only `y` removes. Removal
   goes through the same use case as `ccs … rm`, so it trashes, unlinks, and
   refuses `native` exactly as the command line does.
5. **State is separate from drawing.** `tui::State` takes `Key`s (the adapter's
   own key type) and is drawn by `tui::draw`. Tests drive `State` with keys and
   draw into `TestBackend`; no terminal is needed.
6. **One dependency, `ratatui`.** `crossterm` is used through `ratatui::crossterm`
   only, so the two can never disagree on a version.

### Screen

```
┌ Profiles ─────────┐┌ Overview │ Skills │ Agents │ CLAUDE.md ───────────────────┐
│▶ claude-bedrock   ││ ⬡ claude-bedrock · bedrock claude-bedrock@us-east-1     │
│  native (default) ││ CLAUDE_CODE_USE_BEDROCK=1                               │
│  work             ││ ✓ no broken symlinks                                    │
└───────────────────┘└─────────────────────────────────────────────────────────┘
 ↑↓ move · ←→ tab · / search · enter launch · v view · x delete · d default · q quit
```

## Consequences

- `ccs` gains its first dependency tree (ratatui and what it pulls in). Build
  time grows; the binary stays one file.
- The `Browser` port duplicates no logic: each method is a call into an existing
  use case.
- **Not in this version: adding items from the TUI.** It needs a path input
  or a picker over another profile's items. Recorded as follow-up work here
  rather than deferred: the command line (`ccs skill add …`) covers it today.
  The picker over another profile's items shipped in ADR-2610261720 (`c` copies);
  adding from a path is still command line only.

## Implementation

- `domain/`: `Overview`, `Command::Ui`.
- `ports/`: `Browser` (driving port).
- `usecases/`: `App` implements `Browser`; `overview`.
- `adapters/primary/tui.rs`: `Key`, `State`, `Exit`, `draw`, `run`.
- `lib.rs`: `ccs ui` wiring; launch after restore.
- Gate: `cargo test && hexa analyze . --grade A+`.

### A gap in `hexa analyze`, and how this project closes it

Injecting `use crate::usecases::App;` into `src/adapters/primary/tui.rs` left
the grade at A+ with 0 boundary violations: hexa 26.9.12 allows a primary
adapter to import use cases, although CLAUDE.md says primary adapters import
`ports/` only. The same probe found that hexa does catch primary → domain,
primary → secondary adapter, secondary → use cases and secondary → domain.

An `[[import_policy]]` cannot close it: policies see external imports only.
So `tests/boundaries.rs` reads the real source of `src/adapters/primary/` and
fails on `crate::usecases`, `crate::domain` or `crate::adapters::secondary`. It
was shown to fail on the injected import, naming the line.

The probe also showed one ordinary violation costs 10 points and still grades
`A`, so `--grade A` let one through. The gate is now `--grade A+`, shown to exit
1 on a single violation.

The import policy `tui-depends-on-ratatui-only` limits the TUI's outside
imports to `ratatui`; it was shown to fire on an injected `use regex::Regex;`.

### Checked against the real machine (2026-10-26)

`ccs ui` ran in a real pseudo-terminal (120×22) against a throwaway `$CCS_HOME`
holding `native` (the real `~/.claude`) and `claude-bedrock` (imported from the
real AWS profile). It drew the profile list, the overview
`⬡ claude-bedrock · bedrock claude-bedrock@us-east-1` with `AWS_REGION=us-east-1`,
said "No skills in claude-bedrock" on its Skills tab, showed the real
`inspect-edge` skill on native's Skills tab, quit on `q` with exit status 0, and
left the alternate screen.

Injected bugs, each caught by the named test and reverted:

| Injected bug | Test that failed |
|---|---|
| `x` deletes at once, without asking | `delete_asks_first_and_n_cancels` (and two more) |
| Enter on an item launches instead of viewing | `search_filters_the_focused_list` |
| TUI imports `crate::usecases` | `primary_adapters_import_ports_only` |

## References

- ratatui: https://ratatui.rs/
- ADR-2610261212, ADR-2610261244

## Evidence

`cargo test 2>&1 | grep "test result" && hexa analyze . --grade A 2>&1 | grep "Architecture grade"` at a2f6ff9 with uncommitted changes on 2026-10-26 17:03 UTC:

```text
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.49s
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  ⬡ Architecture grade: A+ — score 100/100
```
