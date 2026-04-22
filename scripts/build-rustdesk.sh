#!/usr/bin/env bash
# Mirror the latest upstream RustDesk binaries under our domain and build the
# Callmor-branded Windows installer that pre-configures callmor.ai as the
# rendezvous server + our public key.
#
# Run this once per upstream RustDesk release. The API serves whatever's in
# the target/rustdesk-{mirror,branded}/ dirs — no rebuild needed afterwards.
#
# Usage: bash scripts/build-rustdesk.sh [VERSION]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="${1:-}"

if [ -z "$VERSION" ]; then
    echo "Fetching latest RustDesk release from GitHub..."
    VERSION=$(curl -sS https://api.github.com/repos/rustdesk/rustdesk/releases/latest \
        | grep -oE '"tag_name": "[^"]+"' | head -1 | cut -d'"' -f4)
    [ -n "$VERSION" ] || { echo "Failed to find latest version."; exit 1; }
    echo "Latest version: $VERSION"
fi

MIRROR="$REPO_ROOT/target/rustdesk-mirror"
BRANDED="$REPO_ROOT/target/rustdesk-branded"
mkdir -p "$MIRROR/windows" "$MIRROR/macos" "$MIRROR/linux" "$BRANDED"

base="https://github.com/rustdesk/rustdesk/releases/download/$VERSION"

echo "Mirroring RustDesk $VERSION binaries..."
curl -fL --progress-bar -o "$MIRROR/windows/rustdesk-$VERSION-x86_64.exe"   "$base/rustdesk-$VERSION-x86_64.exe"
curl -fL --progress-bar -o "$MIRROR/macos/rustdesk-$VERSION-x86_64.dmg"    "$base/rustdesk-$VERSION-x86_64.dmg"
curl -fL --progress-bar -o "$MIRROR/macos/rustdesk-$VERSION-aarch64.dmg"   "$base/rustdesk-$VERSION-aarch64.dmg"
curl -fL --progress-bar -o "$MIRROR/linux/rustdesk-$VERSION-x86_64.deb"    "$base/rustdesk-$VERSION-x86_64.deb"

echo "Building Callmor-branded Windows installer..."
makensis -V2 \
    -DVERSION="$VERSION" \
    -DBIN="$MIRROR/windows/rustdesk-$VERSION-x86_64.exe" \
    -DOUTPUT="$BRANDED/callmor-rd-$VERSION.exe" \
    -DSERVER="callmor.ai" \
    -DKEY="9LE62rY2BFqC+lw28MhiJEewt4KsQHUCWEUWBZIuxtk=" \
    "$REPO_ROOT/packaging/windows/rustdesk-installer.nsi" >/dev/null

echo ""
echo "Done. Artifacts:"
ls -lh "$MIRROR"/windows/ "$MIRROR"/macos/ "$MIRROR"/linux/ "$BRANDED"/
echo ""
echo "Served at:"
echo "  https://api.callmor.ai/downloads/rustdesk/windows         (branded, pre-configured)"
echo "  https://api.callmor.ai/downloads/rustdesk/windows/branded (alias)"
echo "  https://api.callmor.ai/downloads/rustdesk/macos           (official upstream mirror)"
echo "  https://api.callmor.ai/downloads/rustdesk/linux           (official upstream mirror)"
