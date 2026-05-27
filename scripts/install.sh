#!/usr/bin/env bash
# SCRIPT: install.sh
# DESCRIPTION: Build and install the vellum binary to the local system.
# USAGE: ./install [--prefix PREFIX] [--user] [--uninstall]
# PARAMETERS:
#  --prefix DIR : Install to DIR/bin/vellum (default: ~/.local if --user,
#                 /usr/local otherwise; or $PREFIX if set in env).
#  --user       : Install to ~/.local/bin (no sudo required, default on Linux).
#  --uninstall  : Remove previously installed binary.
#  -h           : Show help message and exit.
# EXAMPLE: ./install
#          ./install --prefix /usr/local
#          ./install --user
#          ./install --uninstall
# ----------------------------------------------------
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ "$(basename "$SCRIPT_DIR")" = "scripts" ]; then
  ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
else
  ROOT_DIR="$SCRIPT_DIR"
fi
source "$ROOT_DIR/scripts/include.sh" "$@"

BINARY_NAME="vellum"
UNINSTALL=false
USER_INSTALL=false
PREFIX=""

# Parse install-specific args (include.sh already consumed -h/-v/-d)
for arg in "$@"; do
  case "$arg" in
    --uninstall) UNINSTALL=true ;;
    --user)      USER_INSTALL=true ;;
    --prefix)    ;;  # handled with shift below
  esac
done

# Re-scan for --prefix VALUE pair
i=1
for arg in "$@"; do
  next_i=$((i + 1))
  if [ "$arg" = "--prefix" ] && [ $next_i -le $# ]; then
    eval "PREFIX=\${$next_i}"
  fi
  i=$((i + 1))
done

# Determine install prefix
if [ -z "$PREFIX" ]; then
  if $USER_INSTALL; then
    PREFIX="$HOME/.local"
  elif [ -n "${PREFIX:-}" ]; then
    : # keep env-provided PREFIX
  else
    # Default: user-local on Linux/macOS (no sudo needed)
    PREFIX="$HOME/.local"
  fi
fi

BIN_DIR="$PREFIX/bin"
INSTALL_PATH="$BIN_DIR/$BINARY_NAME"

# ── Uninstall ────────────────────────────────────────────────────────────────
if $UNINSTALL; then
  if [ -f "$INSTALL_PATH" ]; then
    rm -f "$INSTALL_PATH"
    echo "Removed $INSTALL_PATH"
  else
    echo "Not installed at $INSTALL_PATH — nothing to remove."
  fi
  exit 0
fi

# ── Build ────────────────────────────────────────────────────────────────────
echo "Building $BINARY_NAME (release)..."
cd "$ROOT_DIR"
cargo build --release

BUILT_BINARY="$ROOT_DIR/target/release/$BINARY_NAME"
if [ ! -f "$BUILT_BINARY" ]; then
  echo "Build failed: $BUILT_BINARY not found." >&2
  exit 1
fi

# ── Install ──────────────────────────────────────────────────────────────────
mkdir -p "$BIN_DIR"
cp -f "$BUILT_BINARY" "$INSTALL_PATH"
chmod 755 "$INSTALL_PATH"

echo "Installed: $INSTALL_PATH"

# Remind user to add ~/.local/bin to PATH if needed
if [[ "$BIN_DIR" == "$HOME/.local/bin" ]]; then
  if ! echo "$PATH" | tr ':' '\n' | grep -qx "$HOME/.local/bin"; then
    echo ""
    echo "  NOTE: $HOME/.local/bin is not in your PATH."
    echo "  Add this to your shell config (~/.bashrc, ~/.zshrc, etc.):"
    echo ""
    echo "    export PATH=\"\$HOME/.local/bin:\$PATH\""
    echo ""
  fi
fi

echo "Run:  $BINARY_NAME --help"
