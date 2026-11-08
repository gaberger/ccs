# src

## Overview

The whole ccs program: the Claude Code profile switcher. The code is hexagonal
(ADR-2610261212). The rules sit in the middle, and the parts that touch the
world sit at the edge:

```text
  domain  ←  ports  ←  usecases
               ↑
          adapters (primary: the TUI; secondary: filesystem, git, curl, aws, exec)
               ↑
       lib.rs — the composition root, which wires them once
```

`main.rs` is the entry point. `lib.rs` reads the environment, builds every
adapter, parses the command line with the domain, calls the use case for the
command, and prints the result. A few small commands (`version`, `help`,
`kit list`) it answers itself.

## Interface

- `ccs::run(args: Vec<String>) -> i32`: runs one command and returns the exit
  code. An error prints `ccs: <error>` to stderr and returns 1.
- Factory functions that return a port and hide the adapter:
  `profile_store(home)`, `legacy(home, claude_home)`, `library(home)`,
  `kit_store(home)`, `path_input(home, cwd)`, `fetcher()`, `editor()`,
  `pin_locator(start)`. The tests use them.
- `pub mod domain`, `pub mod ports`, `pub mod usecases`, `pub mod adapters`.
- `main.rs`: `fn main` restores the default `SIGPIPE` action, then exits with
  `ccs::run(std::env::args().skip(1))`.
- Subdirectories:
  - [domain](./domain/README.md): pure rules, with no I/O.
  - [ports](./ports/README.md): the traits between use cases and adapters.
  - [usecases](./usecases/README.md): one function per command, plus `App`, the TUI's port.
  - [adapters](./adapters/README.md): the TUI and the 13 driven adapters.

## Invariants

- `lib.rs` is the only file that names a concrete adapter. `hexa analyze .`
  checks this.
- `lib.rs` is the only file that prints messages. The TUI draws on the
  terminal but prints no messages. Every other layer returns values or
  `CcsError`.
- Configuration comes from the environment only in `lib.rs`: `HOME` (required),
  `CCS_HOME`, `CCS_PROFILE`, `CCS_CLAUDE_BIN`, `CCS_AWS_BIN`, `CCS_GIT_BIN`,
  `CCS_CURL_BIN`, `CCS_REPO`, `VISUAL`, `EDITOR`, `SHELL`. Adapters read the
  environment only for their own mechanics (`PATH`, `temp_dir`, `current_exe`).
  `lib.rs` also reads the working directory, and passes the whole environment
  to `ccs statusline`, which reads `CLAUDE_CONFIG_DIR` and the status-segment
  variables from it.
- `$CCS_HOME` defaults to `~/.claude-profiles`. ccs never writes to `~/.claude`.
- The TUI runs only when stdin and stdout are both terminals. It returns an
  `Exit`; `lib.rs` launches after the terminal is restored.
- A closed pipe ends ccs quietly, because `main` restores `SIGPIPE` before any
  thread starts.

## Side Effects

- Prints to stdout and stderr (`lib.rs`). `main.rs` only sets the `SIGPIPE`
  action and exits with the code `run` returns.
- Replaces the process with `claude`, or with `$SHELL` for a profile's shell.
- Everything else happens through the adapters: files under `$CCS_HOME`, reads
  of `~/.claude`, `git`, `curl`, `aws`, the editor, and self-replacement on
  `ccs upgrade`.

## Verification

- Run: `cargo test`
- Boundaries: `hexa analyze . --grade A+ --exit-code`
