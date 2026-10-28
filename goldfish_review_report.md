Status: resolved

Resolution (2026-10-28): the user chose to fix everything. All 27 items are
closed. The 22 doc items are applied, including the two edits that had been
missed (Issue 13: usecases Side Effects; Issue 17: README "What This Does").
The trait comment in Issue 9 is corrected. The 4 code bugs behind Issues 1-5
are fixed in code: Issues 1-3 with tests written first and seen to fail; Issues
4-5 checked by reading the code, because no test can drive them. The proposed
edits below that say "open" were applied as "fixed" instead; the drift report
holds the final status of each item.

# Goldfish Annotated Review Report

**Target:** `2026-10-28_DRIFT_REPORT.md` (full file, lines 1-113)

27 findings: 22 `[doc]`, 5 `[code]`. Each was checked against the code it
cites. None is contested: every one is accurate. The 22 doc fixes are already
applied to the READMEs; the 5 code findings (4 distinct bugs) are open in
`docs/workplans/drafts/draft-2610280734-contract-drift-code-fixes-from.json`.

The proposed edit for every item is the same kind: append a status mark to
the finding in the drift report, so a reader can tell open from closed.

- Doc items: append ` **Status:** fixed; README corrected 2026-10-28.`
- Code items: append ` **Status:** open; README states the current behaviour;
  fix tracked in draft-2610280734.`

## Prioritized Findings & AI Suggestions

### [Issue 1] [`drift: adapters`] - Severity: High
*   **Affected Scope:** [Lines 101-105](2026-10-28_DRIFT_REPORT.md#L101-L105) (src/adapters/secondary/README.md)
*   **Discrepancy Description:** "Profile creation is atomic." It is cleanup on failure: `fs::remove_dir_all(&dir)` on `<name>/` (fs_store.rs:106-109).
*   **Rationale:** `exists()` is false for a folder with no `profile.env` or with the `native` marker (fs_store.rs:92), so a failed create over such a folder deletes what was there, including login state in `config/`.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Verified at fs_store.rs:92 and :104-110. The only finding that can lose user data. The README now says "not atomic" and names the bug; the code fix is item (1) of the workplan.
*   ***Proposed Edit:***
    *   Location: line 105, end of the finding
    *   Change: append ` **Status:** open; README states the current behaviour; fix tracked in draft-2610280734 item (1).`

### [Issue 2] [`drift: usecases, src, root`] - Severity: Medium
*   **Affected Scope:** [Lines 42-45](2026-10-28_DRIFT_REPORT.md#L42-L45) (src/usecases/README.md)
*   **Discrepancy Description:** "the unpacked binary reports the expected version". The check is `said.contains(&version)` (src/usecases/mod.rs:230).
*   **Rationale:** A binary that prints `0.5.10` passes a check for `0.5.1`. The checksum check runs first, so the risk is low.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Verified at mod.rs:230. The README now says "contains, a substring check".
*   ***Proposed Edit:***
    *   Location: line 45
    *   Change: append ` **Status:** open; fix tracked in draft-2610280734 item (2).`

### [Issue 3] [`drift: usecases, src, root`] - Severity: Medium
*   **Affected Scope:** [Lines 70-71](2026-10-28_DRIFT_REPORT.md#L70-L71) (README.md blueprint)
*   **Discrepancy Description:** The same substring version check, as stated in the root blueprint.
*   **Rationale:** Duplicate of Issue 2 in a second document.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Same code line. One fix closes both.
*   ***Proposed Edit:***
    *   Location: line 71
    *   Change: append ` **Status:** open; same fix as the usecases item (draft-2610280734 item (2)).`

### [Issue 4] [`drift: adapters`] - Severity: Medium
*   **Affected Scope:** [Lines 98-100](2026-10-28_DRIFT_REPORT.md#L98-L100) (src/adapters/secondary/README.md)
*   **Discrepancy Description:** A failed `fs::copy` in `GhReleases::install` leaves `.ccs-upgrade-<pid>` behind (gh_releases.rs:102).
*   **Rationale:** Only `set_permissions` and `rename` failures clean up.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Verified: `fs::copy(...)?` returns before the cleanup block.
*   ***Proposed Edit:***
    *   Location: line 100
    *   Change: append ` **Status:** open; fix tracked in draft-2610280734 item (3).`

### [Issue 5] [`drift: adapters`] - Severity: Medium
*   **Affected Scope:** [Lines 84-87](2026-10-28_DRIFT_REPORT.md#L84-L87) (src/adapters/primary/README.md)
*   **Discrepancy Description:** An edit queued by a double-click runs only after the next key press.
*   **Rationale:** `drive` calls `state.edit(b)` in the key branch only (tui.rs:2201-2203), not the mouse branch (tui.rs:2206-2217).
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Verified; the profile page sets `self.editing` from an Enter dispatched by a click (tui.rs:986).
*   ***Proposed Edit:***
    *   Location: line 87
    *   Change: append ` **Status:** open; fix tracked in draft-2610280734 item (4).`

### [Issue 6] [`drift: domain, ports`] - Severity: Low
*   **Affected Scope:** [Lines 14-18](2026-10-28_DRIFT_REPORT.md#L14-L18) (src/domain/README.md)
*   **Discrepancy Description:** `kit_name_for_profile` is valid only for a valid profile name.
*   **Rationale:** The fallback skips `KitName::parse` (domain/mod.rs:618).
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** The one caller passes `ProfileName::as_str()` (usecases/mod.rs:457), so it is a precondition, not a bug. README now states it.
*   ***Proposed Edit:***
    *   Location: line 18
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 7] [`drift: domain, ports`] - Severity: Low
*   **Affected Scope:** [Lines 22-25](2026-10-28_DRIFT_REPORT.md#L22-L25) (src/ports/README.md)
*   **Discrepancy Description:** "only for value types": ports also re-export pure domain functions.
*   **Rationale:** src/ports/mod.rs:13-18.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate; README now names the functions.
*   ***Proposed Edit:***
    *   Location: line 25
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 8] [`drift: domain, ports`] - Severity: Low
*   **Affected Scope:** [Lines 26-28](2026-10-28_DRIFT_REPORT.md#L26-L28) (src/ports/README.md)
*   **Discrepancy Description:** "A primary adapter sees only this trait." The TUI also calls `library_tree`, `pick`, `resolve_kit`.
*   **Rationale:** src/adapters/primary/tui.rs:8.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate. These are pure functions through `ports`, so no layer rule breaks; only the sentence was wrong.
*   ***Proposed Edit:***
    *   Location: line 28
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 9] [`drift: domain, ports`] - Severity: Low
*   **Affected Scope:** [Lines 29-31](2026-10-28_DRIFT_REPORT.md#L29-L31) (src/ports/README.md)
*   **Discrepancy Description:** `Library` holds plugins, not only skills and agents.
*   **Rationale:** `Kind::Plugin`; `main_file` returns `plugin.json` (ports/mod.rs:59).
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate. The trait's own doc comment (ports/mod.rs:45) has the same gap; that is code, outside this report.
*   ***Proposed Edit:***
    *   Location: line 31
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28. The trait comment at src/ports/mod.rs:45 still says "Every skill and agent".`

### [Issue 10] [`drift: domain, ports`] - Severity: Low
*   **Affected Scope:** [Lines 32-34](2026-10-28_DRIFT_REPORT.md#L32-L34) (src/ports/README.md)
*   **Discrepancy Description:** Purpose rows omit trait methods.
*   **Rationale:** `Releases::read`, `Library::main_file`, four `ProfileStore` methods.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate; rows now list them.
*   ***Proposed Edit:***
    *   Location: line 34
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 11] [`drift: usecases, src, root`] - Severity: Low
*   **Affected Scope:** [Lines 38-39](2026-10-28_DRIFT_REPORT.md#L38-L39) (src/usecases/README.md)
*   **Discrepancy Description:** "no I/O themselves", but `library_add_source` calls `Path::exists`.
*   **Rationale:** src/usecases/mod.rs:541.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate; README names the exception.
*   ***Proposed Edit:***
    *   Location: line 39
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 12] [`drift: usecases, src, root`] - Severity: Low
*   **Affected Scope:** [Lines 40-41](2026-10-28_DRIFT_REPORT.md#L40-L41) (src/usecases/README.md)
*   **Discrepancy Description:** `resolve_name` can return `NoneResolved`.
*   **Rationale:** src/usecases/mod.rs:24.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate; README names both errors.
*   ***Proposed Edit:***
    *   Location: line 41
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 13] [`drift: usecases, src, root`] - Severity: Low
*   **Affected Scope:** [Lines 46-49](2026-10-28_DRIFT_REPORT.md#L46-L49) (src/usecases/README.md)
*   **Discrepancy Description:** Subprocesses omit `sha256` and `unpack`; `AwsProfiles` runs `aws`.
*   **Rationale:** src/usecases/mod.rs:224, :228; aws_cli.rs:17.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate. Note: the fix script corrected only the opening sentence of Side Effects; the Subprocesses and "Reads only" bullets still need this edit.
*   ***Proposed Edit:***
    *   Location: line 49; and src/usecases/README.md, Side Effects
    *   Change: in the README, add "`Releases::sha256` runs `shasum`/`sha256sum` and `Releases::unpack` runs `tar`" to Subprocesses, and move `AwsProfiles` there ("runs `aws`"). In the report, append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 14] [`drift: usecases, src, root`] - Severity: Low
*   **Affected Scope:** [Lines 53-55](2026-10-28_DRIFT_REPORT.md#L53-L55) (src/README.md)
*   **Discrepancy Description:** `main.rs` prints nothing; the TUI writes terminal control.
*   **Rationale:** src/main.rs:1-15; tui.rs:2180-2186.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate; my own overclaim. Corrected.
*   ***Proposed Edit:***
    *   Location: line 55
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 15] [`drift: usecases, src, root`] - Severity: Low
*   **Affected Scope:** [Lines 56-58](2026-10-28_DRIFT_REPORT.md#L56-L58) (src/README.md)
*   **Discrepancy Description:** Environment list omits `std::env::vars()` and the working directory.
*   **Rationale:** src/lib.rs:105, :115, :264, :274.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate; corrected.
*   ***Proposed Edit:***
    *   Location: line 58
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 16] [`drift: usecases, src, root`] - Severity: Low
*   **Affected Scope:** [Lines 59-61](2026-10-28_DRIFT_REPORT.md#L59-L61) (src/README.md)
*   **Discrepancy Description:** Not every command calls a use case.
*   **Rationale:** src/lib.rs:399, :245-248, :300-302.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate; corrected.
*   ***Proposed Edit:***
    *   Location: line 61
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 17] [`drift: usecases, src, root`] - Severity: Low
*   **Affected Scope:** [Lines 65-67](2026-10-28_DRIFT_REPORT.md#L65-L67) (README.md blueprint)
*   **Discrepancy Description:** `config/` also holds the `settings.json` ccs seeds.
*   **Rationale:** fs_store.rs:148-150, :57.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate. The same "only login state and history" phrase also appears in README.md's "What This Does" section, which this run did not touch.
*   ***Proposed Edit:***
    *   Location: line 67; and README.md "What This Does"
    *   Change: in README.md "What This Does", change "which holds only login state and history" to "which holds login state, history and the `settings.json` ccs seeds for the status line". In the report, append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 18] [`drift: usecases, src, root`] - Severity: Low
*   **Affected Scope:** [Lines 68-69](2026-10-28_DRIFT_REPORT.md#L68-L69) (README.md blueprint)
*   **Discrepancy Description:** External programs omit `$SHELL`.
*   **Rationale:** src/lib.rs:151-153.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate; corrected.
*   ***Proposed Edit:***
    *   Location: line 69
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 19] [`drift: adapters`] - Severity: Low
*   **Affected Scope:** [Lines 75-76](2026-10-28_DRIFT_REPORT.md#L75-L76) (src/adapters/README.md)
*   **Discrepancy Description:** Environment list omits `HOME`, `CCS_REPO`, `SHELL`, `vi`.
*   **Rationale:** src/lib.rs:70, :99, :151-153, :250.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate; corrected.
*   ***Proposed Edit:***
    *   Location: line 76
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 20] [`drift: adapters`] - Severity: Low
*   **Affected Scope:** [Lines 77-78](2026-10-28_DRIFT_REPORT.md#L77-L78) (src/adapters/README.md)
*   **Discrepancy Description:** `ExecLauncher` also execs `$SHELL`.
*   **Rationale:** src/lib.rs:149-153.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate; corrected.
*   ***Proposed Edit:***
    *   Location: line 78
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 21] [`drift: adapters`] - Severity: Low
*   **Affected Scope:** [Lines 82-83](2026-10-28_DRIFT_REPORT.md#L82-L83) (src/adapters/primary/README.md)
*   **Discrepancy Description:** `Esc` also quits on Home.
*   **Rationale:** tui.rs:2023; test `q_and_esc_quit_from_home` (tests/tui.rs:408).
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Verified; corrected.
*   ***Proposed Edit:***
    *   Location: line 83
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 22] [`drift: adapters`] - Severity: Low
*   **Affected Scope:** [Lines 88-89](2026-10-28_DRIFT_REPORT.md#L88-L89) (src/adapters/primary/README.md)
*   **Discrepancy Description:** The TUI cannot rename or remove profiles.
*   **Rationale:** `Browser` has no such methods (src/ports/mod.rs:178-235).
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate; corrected.
*   ***Proposed Edit:***
    *   Location: line 89
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 23] [`drift: adapters`] - Severity: Low
*   **Affected Scope:** [Lines 90-91](2026-10-28_DRIFT_REPORT.md#L90-L91) (src/adapters/primary/README.md)
*   **Discrepancy Description:** Side effects omit `attach` and `set_kits`.
*   **Rationale:** tui.rs:967; ports/mod.rs:217.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate; corrected.
*   ***Proposed Edit:***
    *   Location: line 91
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 24] [`drift: adapters`] - Severity: Low
*   **Affected Scope:** [Lines 95-97](2026-10-28_DRIFT_REPORT.md#L95-L97) (src/adapters/secondary/README.md)
*   **Discrepancy Description:** Not every program name is passed in.
*   **Rationale:** gh_releases.rs:68-69, :79.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate; corrected.
*   ***Proposed Edit:***
    *   Location: line 97
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 25] [`drift: adapters`] - Severity: Low
*   **Affected Scope:** [Lines 106-109](2026-10-28_DRIFT_REPORT.md#L106-L109) (src/adapters/secondary/README.md)
*   **Discrepancy Description:** The `~/.claude` refusal is lexical.
*   **Rationale:** fs_legacy.rs:155.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate as a limit, not a live bug. A path such as `~/.claude/../x` is refused, not let through, because `starts_with` compares components; only a path that enters `~/.claude` from outside, through `..` or a link, escapes. Callers pass paths from the layout scan.
*   ***Proposed Edit:***
    *   Location: line 109
    *   Change: append ` **Status:** fixed; README states the limit 2026-10-28.`

### [Issue 26] [`drift: adapters`] - Severity: Low
*   **Affected Scope:** [Lines 110-111](2026-10-28_DRIFT_REPORT.md#L110-L111) (src/adapters/secondary/README.md)
*   **Discrepancy Description:** `available()` treats a name with `/` as a path.
*   **Rationale:** exec_launcher.rs:21-23.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate; corrected.
*   ***Proposed Edit:***
    *   Location: line 111
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28.`

### [Issue 27] [`drift: adapters`] - Severity: Low
*   **Affected Scope:** [Lines 112-113](2026-10-28_DRIFT_REPORT.md#L112-L113) (src/adapters/secondary/README.md)
*   **Discrepancy Description:** `ProcessEditor` omits the `vi` fallback and the empty-command error.
*   **Rationale:** src/lib.rs:70; process_editor.rs:23.
*   ***AI Suggestion:*** PROPOSE FIX
*   ***Justification:*** Accurate; corrected.
*   ***Proposed Edit:***
    *   Location: line 113
    *   Change: append ` **Status:** fixed; README corrected 2026-10-28.`
