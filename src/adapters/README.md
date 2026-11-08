# Adapters

## Overview

This is the outer ring of the hexagon: the code that connects ccs to the
terminal, the filesystem and external programs. `mod.rs` declares the two
groups. Primary adapters drive the application inward through driving ports.
Secondary adapters are driven by the application and implement its outbound
ports. The composition root (`src/lib.rs`) is the only place that names a
concrete adapter. It builds each adapter from `HOME`, `$CCS_HOME`, the
working directory, `CCS_REPO`, `SHELL` and the `CCS_*_BIN` / `VISUAL` /
`EDITOR` environment variables (the editor defaults to `vi`), and
it hands the adapters to the use cases as port trait objects.

## Interface

- `mod.rs`: `pub mod primary;` and `pub mod secondary;`.
- [primary](./primary/README.md): the ratatui TUI (`tui::run`, `tui::State`,
  `tui::draw`, `tui::Key`, `tui::Exit`). It drives the application through
  `ports::Browser` and returns an `Exit` for the composition root to act on.
- [secondary](./secondary/README.md): 13 driven adapters, one per outbound
  port:
  - Filesystem: `FsProfileStore`, `FsKitStore`, `FsLibrary`, `FsLegacy`,
    `FsPathInput`, `FsPinLocator`, `FsText`.
  - External programs: `AwsCli` (`aws`), `GitFetcher` (`git`), `CurlReleases`
    (`curl`, `tar`, `shasum`), `ProcessEditor` (`$VISUAL` / `$EDITOR`),
    `ProcessRunner` (the status-line command), `ExecLauncher` (`exec`s
    `claude`, or `$SHELL` for a profile's shell).

## Invariants

- Adapters import from `crate::ports` (plus `std` and external crates) only.
  They never import `crate::domain` or `crate::usecases` directly. If an
  adapter needs a domain type, the port re-exports it.
- Adapters never import other adapters. Primary and secondary adapters reach
  each other only through ports and the composition root.
- Only `src/lib.rs` imports anything under `adapters::`. The integration tests
  `tests/tui.rs` and `tests/parity.rs` also import `ccs::adapters::primary::tui`,
  to drive the screen without a terminal.
- `tests/boundaries.rs` enforces the primary rule against the source text.
  `hexa analyze .` checks the rest.

## Side Effects

- Primary: terminal control (alternate screen, mouse capture, key and mouse
  events).
- Secondary: reads and writes under `$CCS_HOME`, including the trash. Reads
  `~/.claude` (never writes it). Creates temporary clone and download folders.
  Runs `aws`, `git`, `curl`, `tar`, the hash tools, the editor and the
  status-line command. Replaces the `ccs` binary on upgrade. Replaces the
  process with `claude` on launch, or with `$SHELL` for a profile's shell.

## Verification

- Run: `cargo test`
- Boundary rules: `hexa analyze .`
