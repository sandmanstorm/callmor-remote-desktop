#!/usr/bin/env bash
# Cross-compile the Windows agent from Linux.
#
# Prerequisites:
#   rustup target add x86_64-pc-windows-gnu
#   sudo apt install -y gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64
set -euo pipefail
source "$HOME/.cargo/env" 2>/dev/null || true

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION=$(grep '^version' "$REPO_ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)"/\1/')
OUT_DIR="$REPO_ROOT/target/windows"
BIN="$REPO_ROOT/target/x86_64-pc-windows-gnu/release/callmor-agent.exe"

echo "Building callmor-agent.exe for Windows..."
cd "$REPO_ROOT"
cargo build -p callmor-agent-win --target x86_64-pc-windows-gnu --release 2>&1 | tail -3

echo "Stripping..."
x86_64-w64-mingw32-strip "$BIN"

echo "Packaging..."
mkdir -p "$OUT_DIR"
DIST_NAME="callmor-agent-${VERSION}-windows-x64"
DIST_DIR="$OUT_DIR/$DIST_NAME"
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

cp "$BIN" "$DIST_DIR/callmor-agent.exe"
cp "$REPO_ROOT/packaging/windows/README.txt" "$DIST_DIR/" 2>/dev/null || true
cp "$REPO_ROOT/packaging/windows/install.bat" "$DIST_DIR/" 2>/dev/null || true
cp "$REPO_ROOT/packaging/windows/uninstall.bat" "$DIST_DIR/" 2>/dev/null || true
cp "$REPO_ROOT/packaging/windows/agent.conf.template" "$DIST_DIR/" 2>/dev/null || true

# Create .zip
ZIP="$OUT_DIR/$DIST_NAME.zip"
rm -f "$ZIP"
(cd "$OUT_DIR" && zip -r "$DIST_NAME.zip" "$DIST_NAME") > /dev/null

SIZE=$(ls -lh "$ZIP" | awk '{print $5}')
echo "Done: $ZIP ($SIZE)"
