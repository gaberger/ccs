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

## Evidence (2026-10-27)

- CI: green on `ubuntu-latest` and `macos-14`, with the grade, from its first
  run. Action versions moved to checkout@v7, cache@v6, upload-artifact@v7,
  download-artifact@v8 after the first run warned that Node 20 is deprecated.
- Tests: `tests/upgrade.rs`, 14. Injected bugs, each caught: the checksum
  ignored; the new binary not run first; `--check` installing; versions
  compared as text; a checksum line matched by suffix; an older "latest"
  installed (missed at first: no test had one; added); the download folder
  left behind (missed at first: it is in the system temp folder; the tests now
  point `TMPDIR` into their own folder). Not caught, and left so: copying
  straight over the running binary instead of renaming a copy over it. The two
  differ only when a copy fails half-way, which no test here can cause.
- Release: tag `v0.1.0` ran the gate, built the three targets, and published
  `ccs-0.1.0-{aarch64-apple-darwin,x86_64-apple-darwin,x86_64-unknown-linux-gnu}.tar.gz`
  and `SHA256SUMS.txt`.
- Real run, on a copy of the installed binary: `upgrade --check` and
  `upgrade` said 0.1.0 is up to date; `upgrade 0.1.0` installed the CI build
  (arm64, a different hash from the local build) and left no file beside it;
  the README's `gh release download … | tar xz` printed `ccs 0.1.0`.
- Found on the way: a ccs built before this change does not know `upgrade` or
  `version`, so it passes them to `claude` (`claude upgrade` updates Claude
  Code). Install once from source or from a release to get `ccs upgrade`.

## Correction (2026-10-27, drift report)

- The Evidence above counts "the download folder left behind" as caught. Only
  the success path was tested: a bad checksum, a missing `SHA256SUMS.txt` line,
  a binary that did not run, or one that printed another version each returned
  early and left the folder in the temp folder. Now `Releases::discard` runs
  after every attempt, and a failed `gh release download` removes what it
  made. Each failure test checks that the temp folder is empty, and a new test
  covers a binary that prints another version. Injected bugs, each caught: the
  use case not discarding; the download not cleaning up after `gh` fails.
- The probe is stricter than §2 says: the new binary's `version` output must
  hold the version being installed, not only run.
- A release changes `Cargo.lock` with `Cargo.toml`, because CI and the release
  build with `--locked`. It starts from a clean `main` equal to `origin/main`,
  and the tag waits for CI on that commit. `.claude/skills/ccs-release/`
  carries the procedure, and its `scripts/prepare.sh` refuses the other states.

## References

- ADR-2610261212 — one config directory per profile (the first ADR)
- hexa releases: https://github.com/gaberger/hexa/releases

## Note, 2026-11-07: the repository is public

The text above was written while `gaberger/ccs` was private; it is kept as
history. The repository is now public, so releases download without a login.
`ccs upgrade` still uses `gh`, which reads public releases, so nothing in the
code changed. Dropping the `gh` dependency (plain HTTPS plus `SHA256SUMS.txt`)
would be a new decision and needs its own ADR. CI minutes are free for public
repositories, so the cost note above no longer applies.
