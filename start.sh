#!/usr/bin/env bash
# Build ccs and run it. Idempotent: cargo rebuilds only what changed.
#
#   ./start.sh list             build, then run `ccs list`
#   ./start.sh --install        build, then copy ccs to ~/.local/bin (or $CCS_INSTALL_DIR)
set -euo pipefail

cd "$(dirname "$0")"
command -v cargo >/dev/null || { echo "start.sh: cargo not found; install Rust from https://rustup.rs" >&2; exit 1; }
cargo build --release --quiet

if [[ "${1:-}" == "--install" ]]; then
  dest="${CCS_INSTALL_DIR:-$HOME/.local/bin}"
  mkdir -p "$dest"
  install -m 0755 target/release/ccs "$dest/ccs"
  echo "Installed $dest/ccs"
  case ":$PATH:" in *":$dest:"*) ;; *) echo "Add $dest to your PATH." ;; esac
  exit 0
fi

exec target/release/ccs "$@"
