#!/usr/bin/env bash
set -euo pipefail
source "$HOME/.cargo/env" 2>/dev/null || true

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION=$(grep '^version' "$REPO_ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)"/\1/')
ARCH="amd64"
PKG_NAME="callmor-agent"
PKG_DIR="$REPO_ROOT/target/deb/${PKG_NAME}_${VERSION}_${ARCH}"
OUTPUT="$REPO_ROOT/target/deb/${PKG_NAME}_${VERSION}_${ARCH}.deb"

echo "Building $PKG_NAME $VERSION for $ARCH..."

# Build release binary
echo "  Compiling release binary..."
cd "$REPO_ROOT"
cargo build --release -p callmor-agent 2>&1 | tail -3
strip target/release/callmor-agent

# Create .deb directory structure
echo "  Creating package structure..."
rm -rf "$PKG_DIR"
mkdir -p "$PKG_DIR/DEBIAN"
mkdir -p "$PKG_DIR/usr/bin"
mkdir -p "$PKG_DIR/lib/systemd/system"
mkdir -p "$PKG_DIR/usr/share/callmor-agent"

# Copy files
cp target/release/callmor-agent "$PKG_DIR/usr/bin/"
cp packaging/deb/callmor-agent.service "$PKG_DIR/lib/systemd/system/"
cp packaging/deb/agent.conf.template "$PKG_DIR/usr/share/callmor-agent/"
cp packaging/deb/postinst "$PKG_DIR/DEBIAN/"
cp packaging/deb/prerm "$PKG_DIR/DEBIAN/"
chmod 755 "$PKG_DIR/DEBIAN/postinst" "$PKG_DIR/DEBIAN/prerm"

# Calculate installed size (in KB)
INSTALLED_SIZE=$(du -sk "$PKG_DIR" | awk '{print $1}')

# Write control file
cat > "$PKG_DIR/DEBIAN/control" << EOF
Package: $PKG_NAME
Version: $VERSION
Section: admin
Priority: optional
Architecture: $ARCH
Installed-Size: $INSTALLED_SIZE
Depends: libgstreamer1.0-0, gstreamer1.0-plugins-base, gstreamer1.0-plugins-good, gstreamer1.0-plugins-bad, gstreamer1.0-plugins-ugly, gstreamer1.0-x, gstreamer1.0-nice, libxcb-xtest0
Maintainer: Callmor <support@callmor.ai>
Description: Callmor Remote Desktop Agent
 Cross-platform remote desktop agent that captures screen,
 encodes H.264, and streams via WebRTC. Supports mouse and
 keyboard input injection for full remote control.
Homepage: https://callmor.ai
EOF

# Build .deb (uncompressed so the API can byte-replace the enrollment token
# placeholder inside control+data tarballs at download time).
echo "  Building .deb package (uncompressed)..."
dpkg-deb -Znone --root-owner-group --build "$PKG_DIR" "$OUTPUT" 2>&1

# Show result
SIZE=$(ls -lh "$OUTPUT" | awk '{print $5}')
echo ""
echo "Done: $OUTPUT ($SIZE)"
echo "Install with: sudo dpkg -i $OUTPUT"
