#!/usr/bin/env bash
# SCRIPT: lint.sh
# DESCRIPTION: Run Rust formatting and clippy lint checks.
# USAGE: ./lint
# PARAMETERS:
#  -h           : Show help message and exit.
#  -f           : Fix formatting and apply clippy auto-fixes, then re-check.
# EXAMPLE: ./lint
# ----------------------------------------------------
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ "$(basename "$SCRIPT_DIR")" = "scripts" ]; then
  ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
else
  ROOT_DIR="$SCRIPT_DIR"
fi
SCRIPT_HELPERS_DIR="${SCRIPT_HELPERS_DIR:-$ROOT_DIR/scripts/script-helpers}"
if [ -f "$SCRIPT_HELPERS_DIR/helpers.sh" ]; then
  source "$SCRIPT_HELPERS_DIR/helpers.sh"
  shlib_import help logging
  parse_common_args "$@"
fi

fix_mode=0
while getopts "hf?" opt; do
  case $opt in
    h) echo "Usage: ./lint [-f]"; exit 0 ;;
    f) fix_mode=1 ;;
    \?) echo "Invalid option: -$OPTARG" >&2; exit 1 ;;
  esac
done

export CARGO_INCREMENTAL=0
export CARGO_TERM_COLOR=always

if [ "$fix_mode" -eq 1 ]; then
  cd "$ROOT_DIR" && cargo fmt
  cd "$ROOT_DIR" && cargo clippy --fix --allow-dirty --allow-staged -- -D warnings || cargo clippy -- -D warnings
  cd "$ROOT_DIR" && cargo fmt -- --check
  cd "$ROOT_DIR" && cargo clippy -- -D warnings
else
  cd "$ROOT_DIR" && cargo fmt -- --check
  cd "$ROOT_DIR" && cargo clippy -- -D warnings
fi
