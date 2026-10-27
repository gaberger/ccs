#!/usr/bin/env bash
# Prepare a ccs release: check the repository, then set the version in
# Cargo.toml AND Cargo.lock together. It never commits, tags or pushes.
#
#   prepare.sh patch|minor|major|X.Y.Z [--dry-run]
#
# Exit 0: ready (or, with --dry-run, would be ready). Exit 1: a refusal, with why.
set -euo pipefail

want="${1:-}"; dry=""
[[ "${2:-}" == "--dry-run" || "${1:-}" == "--dry-run" ]] && dry=1
[[ "$want" == "--dry-run" ]] && want="${2:-}"
die() { echo "refused: $*" >&2; exit 1; }
[[ -n "$want" ]] || die "usage: prepare.sh patch|minor|major|X.Y.Z [--dry-run]"

root=$(git rev-parse --show-toplevel 2>/dev/null) || die "not in a git repository"
cd "$root"
[[ -f Cargo.toml ]] && grep -q '^name = "ccs"' Cargo.toml || die "$root is not the ccs repository"

# 1. Where we are: main, clean, the same commit as GitHub.
branch=$(git rev-parse --abbrev-ref HEAD)
[[ "$branch" == main ]] || die "on branch $branch; releases are cut from main"
[[ -z "$(git status --porcelain)" ]] || die "uncommitted changes; commit or stash them first:
$(git status --short)"
git fetch --quiet --tags origin main || die "cannot reach origin"
[[ "$(git rev-parse HEAD)" == "$(git rev-parse origin/main)" ]] || die "main and origin/main differ; pull or push first"

# 2. The versions: Cargo.toml's, the last tag's, and the new one.
current=$(sed -n 's/^version = "\([0-9]*\.[0-9]*\.[0-9]*\)"$/\1/p' Cargo.toml | head -1)
[[ -n "$current" ]] || die "no version = \"X.Y.Z\" line in Cargo.toml"
last=$(git tag --list 'v[0-9]*' --sort=-v:refname | head -1); last=${last#v}
IFS=. read -r ma mi pa <<< "${last:-$current}"
case "$want" in
  patch) new="$ma.$mi.$((pa + 1))" ;;
  minor) new="$ma.$((mi + 1)).0" ;;
  major) new="$((ma + 1)).0.0" ;;
  [0-9]*.[0-9]*.[0-9]*) new="${want#v}" ;;
  *) die "'$want' is not patch, minor, major or X.Y.Z" ;;
esac
[[ "$new" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "'$new' is not X.Y.Z"
newer() { [[ "$1" != "$2" && "$(printf '%s\n%s\n' "$1" "$2" | sort -V | tail -1)" == "$1" ]]; }
if [[ -n "$last" ]]; then
  newer "$new" "$last" || die "$new is not newer than the last release, v$last"
fi
# An existing tag is never newer than the last one, so the check above covers it.

since=${last:+v$last}; since=${since:-the first commit}
echo "last release : $since"
echo "Cargo.toml   : $current"
echo "new version  : $new"
echo "changes since $since:"
git log --oneline --no-decorate ${last:+"v$last..HEAD"} | sed 's/^/  /'
[[ -n "$(git log --oneline ${last:+"v$last..HEAD"})" ]] || die "nothing has changed since $since"

if [[ -n "$dry" ]]; then
  echo "dry run: nothing changed"
  exit 0
fi

# 3. Set both files. Cargo.lock repeats the version, and CI builds with --locked,
#    so a lock that still says the old version fails the release build.
# awk, not sed -i: the two seds (GNU, BSD on macOS) take different flags.
if [[ "$current" != "$new" ]]; then
  awk -v o="version = \"$current\"" -v n="version = \"$new\"" '!d && $0 == o {$0 = n; d = 1} 1' Cargo.toml > Cargo.toml.new
  mv Cargo.toml.new Cargo.toml
fi
cargo update --quiet --offline -p ccs 2>/dev/null || cargo check --quiet
grep -q "^version = \"$new\"" Cargo.toml || die "Cargo.toml was not updated"
awk '/^name = "ccs"$/ {getline; print; exit}' Cargo.lock | grep -q "\"$new\"" || die "Cargo.lock does not say $new"
cargo metadata --locked --format-version 1 --no-deps >/dev/null || die "Cargo.lock is not in step with Cargo.toml"

echo "ready: Cargo.toml and Cargo.lock say $new. Next: the gate, then commit, tag v$new, push."
