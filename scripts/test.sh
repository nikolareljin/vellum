#!/usr/bin/env bash
# SCRIPT: test.sh
# DESCRIPTION: Run the full test suite.
# USAGE: ./test
# PARAMETERS: None
# EXAMPLE: ./test
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
cargo test --verbose
