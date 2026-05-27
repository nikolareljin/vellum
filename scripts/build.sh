#!/usr/bin/env bash
# SCRIPT: build.sh
# DESCRIPTION: Build release binary for the current platform.
# USAGE: ./build
# PARAMETERS: None
# EXAMPLE: ./build
# ----------------------------------------------------
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ "$(basename "$SCRIPT_DIR")" = "scripts" ]; then
  ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
else
  ROOT_DIR="$SCRIPT_DIR"
fi
source "$ROOT_DIR/scripts/include.sh" "$@"

cd "$ROOT_DIR"
cargo build --release
echo "Binary: target/release/skopos"
