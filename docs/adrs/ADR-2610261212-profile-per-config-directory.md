# ADR-2610261212: One config directory per profile, selected at launch

**Status:** Proposed
**Date:** 2026-10-26
**Drivers:** Switching Claude Code between a personal claude.ai account and AWS Bedrock profiles is done by copying `settings.json` over itself by hand; each mode needs its own MCP servers, skills and plugins.

## Context

I run Claude Code in more than one mode:

- **Personal** — signed in with my claude.ai account.
- **AWS Bedrock** — one or more AWS profiles and regions, with Bedrock model IDs.

Each mode wants a different set of MCP servers, skills, plugins, hooks, status
line and model settings. Some of these also hold credentials.

Today I switch by hand. `~/.claude` holds `settings.json.bedrock`,
`statusline-bedrock.sh` and five dated `settings.json.bak*` files. Switching means
copying one file over another. This has these problems:

1. **It is global.** A switch changes every Claude Code session on the machine,
   including sessions that are already open in other terminals.
2. **It is partial.** `settings.json` is one of several places that configure a
   session. MCP servers also live in `~/.claude.json` and `~/.claude/mcp.json`.
   Skills live in `~/.claude/skills/`. Login credentials live in the macOS
   Keychain. A file swap does not cover them.
3. **It is lossy.** Each swap risks overwriting a change made in the active
   file. The backup files are the evidence.
4. **It is invisible.** Nothing tells me which profile is active.

What Claude Code gives us to work with:

- `CLAUDE_CONFIG_DIR` — moves the whole user config (settings, `.claude.json`,
  skills, agents, plugins, history) to another directory, for one process.
- Provider environment variables — `CLAUDE_CODE_USE_BEDROCK`, `AWS_PROFILE`,
  `AWS_REGION`, `ANTHROPIC_MODEL`, `ANTHROPIC_SMALL_FAST_MODEL`.
- `--settings <file>` — layers an extra settings file on top for one session.
- `--mcp-config <file>` and `--strict-mcp-config` — sets the MCP servers for one
  session and ignores all others.

### Options considered

**A — Swap files inside `~/.claude`.** Automates today's habit. Rejected: it is
global, so it breaks concurrent sessions; it cannot separate two claude.ai
logins; and every switch overwrites live files.

**B — Launch wrapper with per-session flags over a shared `~/.claude`.** Uses
`--settings` and `--mcp-config --strict-mcp-config`. Per-process and
non-destructive, but rejected: every skill, agent and plugin in `~/.claude` still
loads in every profile, settings merge rather than replace (a base hook leaks
everywhere), and there is still one claude.ai login.

**C — One `CLAUDE_CONFIG_DIR` per profile, shared items symlinked.** Chosen.

## Decision

A profile is a directory. The switcher, `ccs`, is a launcher. It sets
`CLAUDE_CONFIG_DIR` and the profile's environment variables for one process,
then replaces itself with `claude`. It never copies, swaps or deletes a file in
`~/.claude`.

`ccs` is a single Rust binary built on the hexa hexagonal skeleton, so the
architecture grade (`hexa analyze . --grade A`) gates it alongside `cargo test`.

### Layout

```
~/.claude-profiles/            # $CCS_HOME overrides
├── .default                   # name of the default profile
├── shared/                    # written once, symlinked into every profile
│   ├── CLAUDE.md
│   ├── output-styles/
│   ├── agents/
│   └── skills/                # each skill linked one by one
├── native/                    # made by `ccs import`
│   ├── native                 # marker: use the stock ~/.claude, set no CLAUDE_CONFIG_DIR
│   └── profile.env
└── bedrock-work/
    ├── profile.env            # KEY=VALUE, no secrets; parsed, never sourced
    └── config/                # becomes CLAUDE_CONFIG_DIR
        ├── settings.json
        ├── .claude.json       # MCP servers and login state, owned by Claude Code
        ├── skills/
        └── CLAUDE.md -> ~/.claude-profiles/shared/CLAUDE.md
```

### Commands

| Command | Effect |
|---|---|
| `ccs [claude args…]` | Start `claude` with the resolved profile. |
| `ccs <profile> [claude args…]` | Start `claude` with `<profile>`. |
| `ccs list` | List profiles; mark the resolved one and the default. |
| `ccs new <name> [--from <p>]` | Make a profile, optionally cloned; link shared items. |
| `ccs import` | Add the existing `~/.claude` as the profile `native`. |
| `ccs default [<name>]` | Show or set the default profile. |
| `ccs env [<name>]` | Print shell `export` lines, for `eval "$(ccs env work)"`. |
| `ccs link` | Re-link `shared/` into every profile. |
| `ccs doctor [<name>]` | Check a profile. |
| `ccs which` | Print the resolved profile. |

### Rules

1. **Resolution order:** explicit argument, then a `.claude-profile` file in the
   working directory or a parent, then `$CCS_PROFILE`, then the default.
2. **No leaks from the parent shell.** Before a profile's own variables apply,
   `ccs` unsets the provider-selection variables listed in Context, plus
   `CLAUDE_CONFIG_DIR`. It does not unset secrets.
3. **`profile.env` is data.** It is parsed as `KEY=VALUE` lines. It is never
   executed by a shell.
4. **Linking never destroys.** A real file in a profile is never replaced by a
   shared symlink. `ccs` reports it and moves on.
5. **Visibility.** The launched process gets `CCS_PROFILE=<name>`, so a status
   line can show the active profile.

## Consequences

**Good**

- Switching is `ccs bedrock-work`. No file is copied or overwritten.
- A personal session and a Bedrock session run side by side.
- Each profile is a plain directory that can live in git, minus `.claude.json`.
- A new AWS account is `ccs new aws-prod --from bedrock-work` plus one edit.

**Bad, and how we live with it**

- **Split history.** `/resume` shows only sessions of the active profile. Accepted:
  Bedrock work and personal work stay apart.
- **Plugins per profile.** `ccs new --from` copies the settings that enable them.
  If this becomes painful, a later ADR can link `plugins/` into `shared/`.
- **Symlinks can rot.** `ccs doctor` reports broken links.
- **Settings in `settings.json` still win.** An `env` block in a profile's
  `settings.json` overrides `profile.env`, because Claude Code applies it after
  start. `ccs doctor` cannot see that; the profile's author must keep one source.

### Open risk: Keychain separation

Not yet verified. On macOS, Claude Code keeps the claude.ai login in the
Keychain. If two `CLAUDE_CONFIG_DIR` values share one Keychain item, two
claude.ai profiles would need `/login` at each switch. Bedrock profiles are not
affected, because they authenticate through the AWS credential chain.

A direct read of the Keychain was attempted on 2026-10-26 and was refused by
the device's security policy. The check must be behavioral instead: log in
inside profile A, log out inside profile B, and confirm profile A is still
logged in. The status stays **Proposed** until that result is recorded here.

## Implementation

Hexagonal layers under `src/`:

- `domain/` — `ProfileName` (validation), `profile.env` parsing, profile
  resolution, the launch plan (which variables to unset and set), shell
  quoting for `ccs env`, argument parsing into a `Command`, doctor findings.
- `ports/` — `ProfileStore` (profiles on disk), `PinLocator` (finds
  `.claude-profile`), `Launcher` (replaces the process with `claude`).
- `usecases/` — one function per command.
- `adapters/secondary/` — `FsProfileStore`, `FsPinLocator`, `ExecLauncher`.
- `lib.rs` — composition root; `main.rs` calls it.

Gate: `cargo test && hexa analyze . --grade A`.

`CCS_CLAUDE_BIN` overrides the program launched, which lets the end-to-end test
launch `env` and read back exactly what `claude` would receive.

### Checked against the real Claude Code (2.1.310, 2026-10-26)

In a throwaway `$CCS_HOME`, profiles `alpha` and `beta` were made, and
`ccs alpha mcp add -s user demo-only-alpha -- echo hi` was run through the real
`claude`. The server was written to `alpha/config/.claude.json` only. It was
absent from `beta` and from the user's `~/.claude.json`. This confirms that
`CLAUDE_CONFIG_DIR` isolates user-scope MCP servers. It does not settle the
Keychain risk above.

Each of these injected bugs was caught by the named test, then reverted:

| Injected bug | Test that failed |
|---|---|
| Launcher stops clearing inherited provider vars | `a_bedrock_shell_does_not_leak_into_a_personal_profile` |
| Linking replaces a real file | `linking_never_replaces_a_real_file` |
| Clone copies `.claude.json` login state | `cloning_copies_env_settings_and_own_skills_but_relinks_shared` |

## References

- Claude Code settings and environment variables: https://docs.claude.com/en/docs/claude-code/settings
- Claude Code on Amazon Bedrock: https://docs.claude.com/en/docs/claude-code/amazon-bedrock
- hexa: https://github.com/gaberger/hexa

## Evidence

`cargo test 2>&1 | grep "test result" && hexa analyze . --grade A 2>&1 | grep "Architecture grade"` at a2f6ff9 with uncommitted changes on 2026-10-26 16:21 UTC:

```text
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
  ⬡ Architecture grade: A+ — score 100/100
```
