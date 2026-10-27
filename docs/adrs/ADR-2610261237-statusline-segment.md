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

## References

- ADR-2610261212 — one config directory per profile
- Claude Code status line: https://docs.claude.com/en/docs/claude-code/statusline
