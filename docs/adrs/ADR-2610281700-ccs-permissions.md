# ADR-2610281700: `ccs permissions`

Status: accepted (2026-11-07)

## Context

The user needed a profile (`tt-gemma`) to allow a set of tools without asking, and
had a `jq` one-liner that edits `~/.claude/settings.json`. A ccs profile does not
read that file: its `CLAUDE_CONFIG_DIR` is `<profile>/config/`, so the one-liner
changes nothing for a ccs launch. The same one-liner also sorts the list
(`unique`) and rewrites the file with no backup check.

## Decision

- `ccs permissions [<profile>] [show | add <rule>… | add --preset dev | rm <rule>… | mode <m>]`
  (alias `perms`) edits `permissions` in `<profile>/config/settings.json`.
- Existing rules keep their order; new ones are appended once. Every other key
  of the file is left as it was, in its order (`serde_json` with `preserve_order`).
- `dev` is the user's rule list and mode `acceptEdits`, held in `domain`
  (`permission_preset`). A preset sets its mode; plain `add` leaves the mode alone.
- Rules are checked before anything is written (`check_rule`): `Tool` or
  `Tool(specifier)`, one line. Modes: default, acceptEdits, plan, dontAsk, bypassPermissions.
- `ProfileStore` gains `permissions` / `set_permissions`: the adapter that owns the
  profile layout owns the JSON. A file that does not parse, or whose `permissions`
  is not an object, is an error and is never replaced. The old file is copied to
  `settings.json.bak-ccs` first; the new one is written beside it and renamed over.
- New dependency: `serde_json` (with `preserve_order`), used only in `fs_store.rs`.

## Not decided

- `deny` and `ask` lists, and per-project `.claude/settings.json`, are not
  edited here. `ccs permissions` shows and changes `allow` and `defaultMode` only.

## Gate

`tests/permissions.rs`: the preset is compared with the user's own list, copied
from their command, not from the implementation; `jq` reads the result back.
Real run: the same permission set as the user's `jq` on a copy of the same file,
the rest of the file identical, and a second `add --preset dev` adds nothing.
