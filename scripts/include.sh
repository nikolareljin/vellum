#!/usr/bin/env bash
# SCRIPT: include.sh
# DESCRIPTION: Common loader for repo scripts (helpers + standard args).
# USAGE: source ./scripts/include.sh "$@"
# ----------------------------------------------------
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
SCRIPT_HELPERS_DIR="${SCRIPT_HELPERS_DIR:-$ROOT_DIR/scripts/script-helpers}"

# Prefer rustup-managed toolchain over system cargo (avoids lockfile-version
# and edition mismatches when apt-installed Rust is older than stable rustup).
if command -v rustup &>/dev/null; then
  _rustup_home="$(rustup show home 2>/dev/null || true)"
  _active_toolchain="$(rustup show active-toolchain 2>/dev/null | awk '{print $1}' || true)"
  if [[ -n "$_rustup_home" && -n "$_active_toolchain" ]]; then
    _toolchain_bin="$_rustup_home/toolchains/$_active_toolchain/bin"
    [[ -d "$_toolchain_bin" ]] && export PATH="$_toolchain_bin:$PATH"
    unset _toolchain_bin
  fi
  unset _rustup_home _active_toolchain
fi

if [ ! -f "$SCRIPT_HELPERS_DIR/helpers.sh" ]; then
  echo "script-helpers is missing. Run ./update to initialize submodules." >&2
  exit 1
fi

source "$SCRIPT_HELPERS_DIR/helpers.sh"
shlib_import help logging
parse_common_args "$@"
