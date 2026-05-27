#!/usr/bin/env bash
# SCRIPT: update.sh
# DESCRIPTION: Update git submodules (script-helpers) to latest production.
# USAGE: ./update
# PARAMETERS: None
# EXAMPLE: ./update
# ----------------------------------------------------
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ "$(basename "$SCRIPT_DIR")" = "scripts" ]; then
  ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
else
  ROOT_DIR="$SCRIPT_DIR"
fi

cd "$ROOT_DIR"
git submodule update --remote --merge
echo "Submodules updated."
