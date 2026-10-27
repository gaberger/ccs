---
name: ccs-release
description: Cut a release of ccs (this repository, gaberger/ccs) so the new version reaches other machines through `ccs upgrade`. Sets the version in Cargo.toml and Cargo.lock together, runs the gate, commits, tags vX.Y.Z, pushes, watches the Release workflow, and checks the published assets. Use this whenever the user asks to release, publish, ship, cut or tag ccs, bump its version, make a change available to `ccs upgrade`, or asks why `ccs upgrade` does not see a change that is already on main — even if they don't say "release".
---

# Releasing ccs

A ccs release is a git tag `vX.Y.Z` on `main`. Pushing the tag starts
`.github/workflows/release.yml`, which runs the gate, builds three platforms,
and publishes `ccs-X.Y.Z-<target>.tar.gz` plus `SHA256SUMS.txt`. `ccs upgrade`
on every machine reads those (ADR-2610271000).

## Why the version needs care

The version lives in two files, and the tag must agree with both:

- `Cargo.toml` — the `version = "X.Y.Z"` line. `ccs version` prints it, and
  `ccs upgrade` compares against it.
- `Cargo.lock` — repeats the version under `name = "ccs"`. CI builds with
  `--locked`, so a lock file left at the old version fails the release build.
- The tag — the release workflow refuses a tag that differs from `Cargo.toml`.

A change merged to `main` without a new tag never reaches `ccs upgrade`: it
offers only published releases. That is the usual answer when someone asks why
an upgrade "does not see" a change.

Edit the version only through `scripts/prepare.sh`. It refuses the states that
break a release, and it sets both files at once.

## Steps

### 1. See what changed, and choose the version

```bash
.claude/skills/ccs-release/scripts/prepare.sh minor --dry-run
```

It checks that you are on `main`, the tree is clean, `main` equals
`origin/main`, and the new version is newer than the last tag. Then it lists
the commits since the last release. Read that list to choose:

- **patch** (0.2.0 → 0.2.1): fixes, docs, tests only; nothing a user must learn.
- **minor** (0.2.0 → 0.3.0): a new command, key, option or behaviour, or, while
  the version is 0.x, anything that changes how existing use works.
- **major**: only when the user asks for it.
- A version the user names (`0.4.0`) wins over this rule.

Tell the user the version and why, in one line, before going on. If the script
refuses, report its reason and fix the cause (commit, pull, push); do not work
around it.

### 2. Set the version

```bash
.claude/skills/ccs-release/scripts/prepare.sh <patch|minor|major|X.Y.Z>
```

It changes `Cargo.toml` and `Cargo.lock` and nothing else, then confirms that
`cargo metadata --locked` agrees. `git diff --stat` must show exactly those two
files.

### 3. Run the gate

```bash
cargo test 2>&1 | tee /tmp/ccs-test.log
awk '/^test result:/ {p+=$4; f+=$6} END {print "passed:", p, "failed:", f; exit (p == 0 || f > 0)}' /tmp/ccs-test.log
cargo clippy --all-targets -- -D warnings
hexa analyze . --grade A+ --exit-code
```

"0 passed" is a failure, not a pass. Do not tag on a red gate: CI runs the same
gate on the tag and refuses to publish.

### 4. Commit and push main

```bash
git add Cargo.toml Cargo.lock
git commit -m "Release vX.Y.Z" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
git push origin main
gh run watch "$(gh run list --branch main --workflow ci.yml --limit 1 --json databaseId --jq '.[0].databaseId')" --exit-status
```

Wait for CI on that commit to pass before tagging, so the tag points at a
commit already proven on Linux and macOS.

### 5. Tag and push the tag

```bash
git tag -a vX.Y.Z -m "ccs X.Y.Z"
git push origin vX.Y.Z
```

Pushing the tag publishes the release. The user asking for a release is the
go-ahead; do not push a tag for any other reason.

### 6. Watch the release and check it

```bash
gh run watch "$(gh run list --workflow release.yml --limit 1 --json databaseId --jq '.[0].databaseId')" --exit-status
gh release view vX.Y.Z --json assets --jq '.assets[].name'
ccs upgrade --check
```

The release must hold four files: the three `ccs-X.Y.Z-*.tar.gz` (for
`aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`) and
`SHA256SUMS.txt`. `ccs upgrade --check` on this machine must now offer X.Y.Z;
`ccs upgrade` installs it.

## When something fails

- **The release workflow fails at "The tag matches Cargo.toml"**: the version
  was not set first. A tag already pushed may have been fetched by others, so do
  not move or delete it without asking the user. Prefer the next patch version,
  done properly.
- **A build or the gate fails on the tag**: fix the cause on `main` and release
  the next patch version. Do not re-point an existing tag.
- **`gh` is not logged in**: ask the user to run `! gh auth login`.

## Report

End with: the version and why it was chosen, the commit and tag, the workflow
result, the four assets, and what `ccs upgrade --check` said.
