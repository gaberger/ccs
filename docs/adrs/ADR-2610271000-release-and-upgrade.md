# ADR-2610271000: Releases built by CI, and `ccs upgrade`

Status: accepted (2026-10-27)

## Context

The user: "can u add CI to build a release", then "than add a ccs upgrade".
Until now ccs was installed only from source, with `./start.sh --install`, so
each machine needed Rust and a clone. The repository, `gaberger/ccs`, is
private, so its releases need a GitHub login to download.

## Decision

### 1. CI and releases

- `.github/workflows/ci.yml` runs the gate on every push to `main` and every
  pull request: `cargo test` (failing on "0 passed", since a vacuous gate is a
  failed gate) and `cargo clippy -D warnings` on Linux and macOS, and
  `hexa analyze . --grade A+ --exit-code` with hexa 26.9.12 from its public
  release, checked against its published SHA-256.
- `.github/workflows/release.yml` runs on a tag `vX.Y.Z`: the CI gate first,
  then a refusal if the tag differs from `Cargo.toml`'s version, then builds
  for `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin` and
  `x86_64-apple-darwin`, and publishes a GitHub release holding
  `ccs-X.Y.Z-<target>.tar.gz` (the `ccs` binary alone) and `SHA256SUMS.txt`.
  The names match hexa's releases. Run by hand, it builds without publishing.
- Linux on ARM is left out: GitHub's ARM runners are not assumed free for a
  private repository.
- Only GitHub's own actions are used, plus the runners' `rustup` and `gh`.

### 2. `ccs upgrade`

- `ccs upgrade` installs the latest release over the running `ccs`, when it is
  newer. `ccs upgrade --check` only says what is available. `ccs upgrade X.Y.Z`
  installs that version, older or newer. `ccs version` (and `--version`)
  prints the running version.
- It downloads with the GitHub CLI, `gh release download`, because the
  repository is private and `gh` already holds the user's login. Without `gh`,
  it says so and names the command to install and log in.
- The archive is checked against `SHA256SUMS.txt` before anything is unpacked.
  A mismatch, a missing line, or a missing asset for this platform refuses, and
  the installed `ccs` stays as it was.
- The new binary is written next to the running one and renamed over it, so a
  failure part-way never leaves a half-written `ccs`. It is run once with
  `version` first; a binary that cannot run is not installed.
- A `Releases` port; the `gh_releases` adapter runs `gh`, `tar`, and
  `shasum -a 256` or `sha256sum`. The use case decides versions and names.
- `$CCS_GH_BIN` replaces `gh`, for the tests; `$CCS_REPO` replaces
  `gaberger/ccs`, for a fork.

## Consequences

- A new machine needs `gh auth login`, then one download, instead of Rust.
- CI costs Actions minutes on a private repository; macOS minutes cost ten
  times Linux ones. A release builds twice on macOS.
- `ccs upgrade` replaces the binary it runs from. Installed with `start.sh`,
  that is `~/.local/bin/ccs`; a binary run from `target/` replaces that one.

## Implementation

- Gate: `cargo test && hexa analyze . --grade A+ --exit-code`; tests with a fake
  `gh` serving a local archive: an upgrade, `--check`, up to date, an explicit
  version, a checksum mismatch, a missing platform asset, and no `gh`.
- Real run: tag `v0.1.0`, watch the release build, then `ccs upgrade 0.1.0` on
  a copy of the binary.

## References

- ADR-2610261212 — one config directory per profile (the first ADR)
- hexa releases: https://github.com/gaberger/hexa/releases
