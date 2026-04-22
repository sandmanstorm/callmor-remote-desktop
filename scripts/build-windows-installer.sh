#!/usr/bin/env bash
# Cross-compile the Windows agent, then wrap it into two NSIS .exe installers:
#   1. tenant variant (target/windows/callmor-agent-setup-<ver>.exe)
#      - baked-in enrollment-token placeholder, replaced at download time by the
#        authenticated /downloads/agent/windows endpoint.
#   2. adhoc  variant (target/windows-public/callmor-agent-public-<ver>.exe)
#      - no token at all; on first run the agent self-registers and displays
#        an access code + PIN for anyone to connect with, no account required.
#
# Prerequisites (on Debian):
#   rustup target add x86_64-pc-windows-gnu
#   sudo apt install -y gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64 nsis
set -euo pipefail
source "$HOME/.cargo/env" 2>/dev/null || true

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION=$(grep '^version' "$REPO_ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)"/\1/')
BIN="$REPO_ROOT/target/x86_64-pc-windows-gnu/release/callmor-agent.exe"

echo "Building callmor-agent.exe for Windows..."
cd "$REPO_ROOT"
cargo build -p callmor-agent-win --target x86_64-pc-windows-gnu --release 2>&1 | tail -3
x86_64-w64-mingw32-strip "$BIN"

build_installer() {
    local mode="$1"        # tenant | adhoc
    local out_dir="$2"
    local out_name="$3"
    mkdir -p "$out_dir"
    local installer="$out_dir/$out_name"
    echo "  Building NSIS installer (mode=$mode) -> $installer"
    makensis -V2 \
        -DVERSION="$VERSION" \
        -DBIN="$BIN" \
        -DOUTPUT="$installer" \
        -DMODE="$mode" \
        "$REPO_ROOT/packaging/windows/installer.nsi" > /dev/null
    [ -f "$installer" ] || { echo "    FAILED"; return 1; }
    ls -lh "$installer" | awk '{print "    "$5, $9}'
}

build_installer "tenant" "$REPO_ROOT/target/windows"        "callmor-agent-setup-${VERSION}.exe"
build_installer "adhoc"  "$REPO_ROOT/target/windows-public" "callmor-agent-public-${VERSION}.exe"

echo ""
echo "Done:"
echo "  Tenant installer (requires login to download): target/windows/callmor-agent-setup-${VERSION}.exe"
echo "  Public installer (no-login, code+pin flow):    target/windows-public/callmor-agent-public-${VERSION}.exe"
