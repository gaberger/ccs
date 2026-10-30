# ADR-2610261237: The status line shows the active profile, wrapping any existing status line

**Status:** Accepted
**Date:** 2026-10-26
**Drivers:** Problem 4 of ADR-2610261212 ("It is invisible") — nothing on screen says which profile a session runs under. The user already has a status line script (`~/.claude/statusline-bedrock.sh`) that must keep working.

## Context

Claude Code runs the `statusLine.command` from settings on each refresh. It
pipes a JSON payload about the session on stdin and shows the first lines the
command prints. The command inherits the environment of the `claude` process.

`ccs` already exports `CCS_PROFILE` and the profile's provider variables into
that process (ADR-2610261212, rule 5). So a status line command can know the
profile without reading any file.

The user's existing script reads the JSON from stdin. Anything that sits in
front of it must pass stdin through unchanged.

## Decision

Add `ccs statusline [-- <command> [args…]]`.

1. It prints a segment: `⬡ <profile> · <provider>`.
   - Provider is `bedrock <AWS_PROFILE>@<AWS_REGION>` when
     `CLAUDE_CODE_USE_BEDROCK` is truthy, then `vertex`, `foundry`, `gateway`
     (`ANTHROPIC_BASE_URL` set), else `anthropic`. Missing parts are left out.
   - Without `CCS_PROFILE` (claude not started by `ccs`) the segment is empty.
2. With `-- <command>`, it runs the command with the same stdin, and prints
   `<segment> │ <first line of its output>`, then the remaining lines as-is.
3. If the wrapped command cannot start, it prints the segment and
   `│ ⚠ statusline failed: <command>`. It never goes silent: a status line that
   drops a failure is worse than one that names it.
4. `ccs new <name>` (without `--from`) and `ccs import aws` write a
   `config/settings.json` with `statusLine` set to `<path of ccs> statusline`,
   only when no `settings.json` exists. A cloned profile keeps the status line
   it was cloned with.

## Consequences

- Every fresh profile shows its name with no setup.
- Existing scripts keep working by wrapping: `"command": "ccs statusline -- ~/.claude/statusline-bedrock.sh"`.
- The segment trusts the environment. An `env` block in the profile's
  `settings.json` that changes the provider is also visible to the status line,
  because Claude Code applies it to its own process before running children.
- One more process per refresh when wrapping. It is a small cost; the status
  line refresh is not a hot path.

## Implementation

- `domain/`: `status_segment`, `compose_status`, `default_settings`, `Command::Statusline`.
- `ports/`: `CommandRunner` (run argv with stdin, return stdout).
- `adapters/secondary/process_runner.rs`: `ProcessRunner`.
- `ProfileStore::write_settings_if_absent`.
- Gate: `cargo test && hexa analyze . --grade A`.

## 2. A launch sets its own status line (decided 2026-10-27)

The user: "the status line doesn't tell me what profile i am on or what kits".
Found: a project's `.claude/settings.json` ranks above the profile's
`settings.json`, and hexa writes one with `statusLine: hexa loop | head -1`.
In this repository, and in four others of the user's (APIDIFF, deskman,
forward-mcp, fwd), the profile's status line never ran. The `⬡ Loop (ccs)` the
user saw is hexa's, and `ccs` there is the project name.

Measured first: `claude --settings '{"statusLine":…}'` in this repository
showed the `--settings` line, not the project's. Command-line settings rank
above project settings.

- Every launch passes `--settings` with `ccs statusline --auto` as the status
  line. `--auto` prints the segment, then runs the first status line it finds
  in the project's `.claude/settings.local.json`, its `.claude/settings.json`,
  then the profile's `settings.json`, with the same stdin, and shows its first
  line after `│`. The project folder comes from the payload's
  `workspace.project_dir`.
- `ccs statusline -- X` in a settings file stands for X; a plain `ccs
  statusline`, or `--auto`, is skipped, so it never runs itself.
- A session not started by ccs (plain `claude`) is untouched: ccs is not in it.
- A user's own `--settings` after the profile name comes later on the command
  line than ccs's.

**Done** (tests: 3 in `tests/domain.rs`, 1 in `tests/cli.rs`, 1 in
`tests/kits.rs`; the fake `claude` of `tests/cli.rs` became a script, since
`env` rejects `--settings`). Real run (2026-10-27): `ccs claude-bedrock-gary`
in this repository showed `⬡ claude-bedrock-gary +base · bedrock
claude-bedrock-gary@us-east-1 │ ⬡ Loop (ccs): nothing recorded…`.

## Later changes (2026-10-27)

- **Kits in the segment.** The segment is now `⬡ <profile> +<kit>… ·
  <provider>`: one `+kit` per name in `CCS_KITS`, which a launch with kits
  exports. ADR-2610261643 §4 decided it ("The status line segment names the
  kits"); Decision 1 above predates kits. Code: `status_segment` in
  `src/domain/mod.rs`.
- **Where `--auto` finds the project.** §2 says the project folder comes from
  `workspace.project_dir`. The code falls back to `workspace.current_dir`, then
  to the payload's top-level `cwd` (`statusline_dir` in `src/domain/mod.rs`).
  No ADR records that fallback; this note does.

## References

- ADR-2610261212 — one config directory per profile
- Claude Code status line: https://docs.claude.com/en/docs/claude-code/statusline

## 3. The profile's line and the project's both show (decided 2026-10-30)

The user: "where are the ccs updates on the status line". Found: the profile's
`settings.json` was switched to a two-line powerline
(`~/.claude/statusline-host.sh`), and in this repository it never ran. §2 runs
only the *first* status line it finds, and hexa's project line comes before the
profile's, so every hexa project hid the profile's line.

- `--auto` now runs the profile's own status line, then the project's (local
  settings before shared), each with the same stdin. The segment goes before
  the first output line; the rest follow in order. An empty output is left out.
- A command that both the profile and the project name runs once.
- A failure in one is named in its place (§1 rule 3); the other still shows.
- What ccs prints itself stays plain text for any terminal and font: no escape
  codes, no private-use glyphs (Nerd Font, powerline). Styling belongs to the
  user's own line, which ccs runs and passes through unchanged. Guarded by
  `the_segment_is_plain_text_for_any_terminal`.
- Found on the way: a launch with no kits left `CCS_KITS` alone, so a profile
  launched from inside a kit session showed that session's kits. `with_kits`
  now unsets it when no kit is chosen.
- Gate: `cargo test --test cli statusline_auto && cargo test --test domain status && cargo clippy --all-targets -- -D warnings`.
