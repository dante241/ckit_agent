#!/usr/bin/env bash
# Build ckit (release) and install it as `ckit` on the user's PATH.
#
# The crate produces a binary named `ckit` (Cargo `[[bin]] name`), and the
# embedded CMD/NS default to `ckit`, so the invoked command + config namespace
# (`~/.config/ckit/`) match the file name — we just copy it onto PATH.
set -euo pipefail

cd "$(dirname "$0")"

BIN_DIR="${BIN_DIR:-$HOME/.local/bin}"
DEST="$BIN_DIR/ckit"

echo "==> cargo build --release --bin ckit"
cargo build --release --bin ckit

mkdir -p "$BIN_DIR"
install -m755 target/release/ckit "$DEST"
echo "==> installed → $DEST"

# Refresh the shell's command-path cache so the new binary is picked up now.
hash -r 2>/dev/null || true

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) echo "!  $BIN_DIR is not on PATH — add it: export PATH=\"$BIN_DIR:\$PATH\"" ;;
esac

echo "==> $(ckit --version 2>/dev/null || echo 'ckit installed')"
