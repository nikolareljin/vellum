#!/usr/bin/env bash
# SCRIPT: setup.sh
# DESCRIPTION: Install system dependencies and Rust toolchain components needed for skopos.
# USAGE: ./setup
# PARAMETERS:
#  -h           : Show help message and exit.
# EXAMPLE: ./setup
# ----------------------------------------------------
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ "$(basename "$SCRIPT_DIR")" = "scripts" ]; then
  ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
else
  ROOT_DIR="$SCRIPT_DIR"
fi
source "$ROOT_DIR/scripts/include.sh" "$@"

echo "Installing system dependencies..."
if command -v apt-get &>/dev/null; then
  sudo apt-get update && sudo apt-get install -y build-essential ffmpeg
elif command -v brew &>/dev/null; then
  brew install ffmpeg
else
  echo "Unsupported package manager. Install ffmpeg manually." >&2
fi

echo "Installing Rust toolchain components..."
rustup component add rustfmt clippy

echo "Initialising git submodules..."
cd "$ROOT_DIR"
git submodule update --init --recursive

echo "Setup complete. Run ./build to compile."
