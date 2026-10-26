# ADR 0001: One config directory per profile, selected at launch

- **Status:** Proposed
- **Date:** 2026-10-26
- **Deciders:** Gary Berger

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
3. **It is lossy.** Each swap risks overwriting a change I made in the active
   file. The backup files are the evidence.
4. **It is invisible.** Nothing tells me which profile is active.

### What Claude Code gives us to work with

- `CLAUDE_CONFIG_DIR` — moves the whole user config (settings, `.claude.json`,
  skills, agents, plugins, history) to another directory, for one process.
- Environment variables for the provider — `CLAUDE_CODE_USE_BEDROCK`,
  `AWS_PROFILE`, `AWS_REGION`, `ANTHROPIC_MODEL`, `ANTHROPIC_SMALL_FAST_MODEL`.
- `--settings <file>` — layers an extra settings file on top for one session.
- `--mcp-config <file>` and `--strict-mcp-config` — sets the MCP servers for
  one session and ignores all others.

## Decision drivers

- Switching must be one short command.
- Two profiles must be able to run at the same time, in two terminals.
- A profile must control **all** of: provider/auth, model, MCP servers, skills,
  plugins, hooks, status line.
- Things I want everywhere (my global `CLAUDE.md`, output styles, agents) must be
  written once, not copied into each profile.
- No new runtime dependency. A shell script is enough.
- Switching must never edit or delete my existing `~/.claude`.

## Options considered

### Option A — Swap files inside `~/.claude`

Automate what I do now: copy `profiles/<name>/settings.json` over
`~/.claude/settings.json`, and the same for MCP files.

- ✅ Simplest to build.
- ❌ Global: breaks concurrent sessions.
- ❌ Cannot separate two claude.ai logins.
- ❌ Destructive by design: every switch overwrites live files.

### Option B — Launch wrapper with per-session flags, shared `~/.claude`

Keep one `~/.claude`. A wrapper starts `claude` with the profile's env vars,
`--settings profiles/<name>/settings.json` and
`--mcp-config profiles/<name>/mcp.json --strict-mcp-config`.

- ✅ Per-process, so concurrent sessions work.
- ✅ Nothing in `~/.claude` changes.
- ❌ Skills, agents and plugins in `~/.claude` still load for every profile.
  Hiding them needs a long `skillOverrides` / `enabledPlugins` list per profile.
- ❌ Only one claude.ai login exists.
- ❌ Settings merge rather than replace, so a hook or permission in the base
  file leaks into every profile.

### Option C — One `CLAUDE_CONFIG_DIR` per profile, shared items symlinked (chosen)

Each profile is a complete Claude Code config directory. The wrapper sets
`CLAUDE_CONFIG_DIR` and the profile's env vars, then starts `claude`. Items I
want in every profile are symlinks to one shared directory.

- ✅ Per-process, so concurrent sessions work.
- ✅ Total control: each profile has its own settings, `.claude.json` (MCP
  servers), skills, plugins, hooks and login.
- ✅ Shared items are written once, through symlinks.
- ✅ `~/.claude` is not touched. It can stay as the "default" profile.
- ❌ History, `projects/` memory and session resume are separate per profile.
  (I count this as mostly a benefit: Bedrock work and personal work do not mix.)
- ❌ Plugins install per profile, so a plugin I want everywhere needs an
  install in each profile, or a symlinked `plugins/` directory.
- ❌ More directories on disk.

## Decision

**Choose Option C.** A profile is a directory. The switcher is a launcher that
sets `CLAUDE_CONFIG_DIR` and env vars for one process. It never swaps files.

### Layout

```
~/.claude-profiles/
├── shared/                    # written once, symlinked into every profile
│   ├── CLAUDE.md
│   ├── output-styles/
│   ├── agents/
│   └── skills/                # skills wanted everywhere (each linked one by one)
├── personal/
│   ├── profile.env            # env vars for this profile (no secrets)
│   └── config/                # becomes CLAUDE_CONFIG_DIR
│       ├── settings.json
│       ├── .claude.json       # MCP servers, login state
│       ├── skills/            # profile skills + symlinks into shared/skills
│       ├── CLAUDE.md -> ../../shared/CLAUDE.md
│       └── output-styles -> ../../shared/output-styles
└── bedrock-work/
    ├── profile.env
    └── config/
```

Example `profile.env` for a Bedrock profile:

```sh
CLAUDE_CODE_USE_BEDROCK=1
AWS_PROFILE=work-sso
AWS_REGION=us-east-1
ANTHROPIC_MODEL=us.anthropic.claude-opus-5-5-v1:0
ANTHROPIC_SMALL_FAST_MODEL=us.anthropic.claude-haiku-5-5-v1:0
```

`profile.env` holds names and IDs only. Secrets stay in the AWS credential
chain (SSO / `~/.aws`) or the Keychain. The tool never writes a secret to disk.

### Command surface (`ccs`)

| Command | Effect |
|---|---|
| `ccs` | Start `claude` with the resolved profile (see below). |
| `ccs <name> [claude args…]` | Start `claude` with profile `<name>`. |
| `ccs list` | List profiles; mark the default. |
| `ccs new <name> [--from <name>]` | Make a profile, optionally cloned from another. Wire shared symlinks. |
| `ccs edit <name>` | Open the profile in `$EDITOR`. |
| `ccs default <name>` | Set the default profile. |
| `ccs env <name>` | Print `export` lines, for `eval "$(ccs env bedrock-work)"`. |
| `ccs doctor <name>` | Check symlinks, env vars, AWS session, and MCP servers. |
| `ccs import` | Make a profile named `default` that points at the existing `~/.claude`. |

### Profile resolution order

1. An explicit name on the command line.
2. A `.claude-profile` file in the current directory or a parent (pins a repo
   to a profile).
3. `$CCS_PROFILE`.
4. The default set with `ccs default`.

### Visibility

The launcher exports `CCS_PROFILE=<name>`. Each profile's status line shows
it. This replaces `statusline-bedrock.sh` with one shared script that reads
the variable.

## Consequences

**Good**

- Switching is `ccs bedrock-work`. No file is copied or overwritten.
- A personal session and a Bedrock session can run side by side.
- The backup-file pile goes away; each profile is a plain directory I can put in
  git (minus `.claude.json`, which holds login state).
- Adding a new AWS account is `ccs new aws-prod --from bedrock-work` plus one
  edit to `profile.env`.

**Bad, and how we live with it**

- **Split history.** `/resume` shows only sessions from the active profile.
  Accepted.
- **Plugins per profile.** `ccs new --from` copies the plugin config. If this
  becomes painful, symlink `plugins/` into `shared/` in a later ADR.
- **Symlinks can rot.** `ccs doctor` checks them.

## Risks to check in a spike before "Accepted"

1. **Keychain separation.** Confirm that two `CLAUDE_CONFIG_DIR` values keep two
   separate claude.ai logins on macOS, and do not share one Keychain item. If
   they share, personal-vs-work claude.ai profiles need `/login` on each switch,
   and this ADR needs an amendment. Bedrock profiles are not affected.
2. **Symlinked skills and `CLAUDE.md`.** Confirm Claude Code follows symlinks for
   `skills/<name>/`, `CLAUDE.md`, `output-styles/` and `agents/`.
3. **Managed and project settings.** Enterprise managed settings and a repo's
   `.claude/settings.json` still apply on top of any profile. This is expected,
   but `ccs doctor` should say so when a project overrides the provider.

## Out of scope

- A GUI or TUI picker. (`ccs list | fzf` is enough for now.)
- Syncing profiles between machines.
- Storing or rotating secrets.
