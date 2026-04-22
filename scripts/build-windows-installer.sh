#!/usr/bin/env bash
# Cross-compile the Windows agent, then wrap it into a single NSIS .exe installer.
# Produces one file: target/windows/callmor-agent-setup-<version>.exe
#
# Prerequisites (on Debian):
#   rustup target add x86_64-pc-windows-gnu
#   sudo apt install -y gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64 nsis
set -euo pipefail
source "$HOME/.cargo/env" 2>/dev/null || true

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION=$(grep '^version' "$REPO_ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)"/\1/')
BIN="$REPO_ROOT/target/x86_64-pc-windows-gnu/release/callmor-agent.exe"
OUT_DIR="$REPO_ROOT/target/windows"

echo "Building callmor-agent.exe for Windows..."
cd "$REPO_ROOT"
cargo build -p callmor-agent-win --target x86_64-pc-windows-gnu --release 2>&1 | tail -3
x86_64-w64-mingw32-strip "$BIN"

echo "Building NSIS installer..."
mkdir -p "$OUT_DIR"
INSTALLER="$OUT_DIR/callmor-agent-setup-${VERSION}.exe"
makensis -V2 -DVERSION="$VERSION" -DBIN="$BIN" -DOUTPUT="$INSTALLER" "$REPO_ROOT/packaging/windows/installer.nsi" > /dev/null
if [ -f "$INSTALLER" ]; then
    SIZE=$(ls -lh "$INSTALLER" | awk '{print $5}')
    echo ""
    echo "Done: $INSTALLER ($SIZE)"
    echo "Single .exe with placeholder enrollment token — API injects the"
    echo "tenant's real token at download time. Double-click installs and starts."
else
    echo "NSIS build failed — installer not produced"
    exit 1
fi
