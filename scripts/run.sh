#!/usr/bin/env bash
# SCRIPT: run.sh
# DESCRIPTION: Build and run vellum against a local Markdown file.
# USAGE: ./run [<file.md>]
# PARAMETERS:
#  -h           : Show help message and exit.
# EXAMPLE: ./run README.md
# ----------------------------------------------------
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ "$(basename "$SCRIPT_DIR")" = "scripts" ]; then
  ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
else
  ROOT_DIR="$SCRIPT_DIR"
fi
source "$ROOT_DIR/scripts/include.sh" "$@"

TARGET="${1:-$ROOT_DIR/README.md}"

cd "$ROOT_DIR"
cargo build --release
./target/release/vellum "$TARGET"
