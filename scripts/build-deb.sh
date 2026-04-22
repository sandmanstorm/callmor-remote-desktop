#!/usr/bin/env bash
set -euo pipefail
source "$HOME/.cargo/env" 2>/dev/null || true

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION=$(grep '^version' "$REPO_ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)"/\1/')
ARCH="amd64"
PKG_NAME="callmor-agent"

echo "Building $PKG_NAME $VERSION for $ARCH..."

echo "  Compiling release binary..."
cd "$REPO_ROOT"
cargo build --release -p callmor-agent 2>&1 | tail -3
strip target/release/callmor-agent

build_deb() {
    local mode="$1"   # tenant | adhoc
    local out_dir="$2"
    local out_name="$3"
    local template_src="$4"

    local pkg_dir="$REPO_ROOT/$out_dir/${PKG_NAME}_${VERSION}_${ARCH}"
    local output="$REPO_ROOT/$out_dir/$out_name"

    echo "  Building $mode .deb -> $output"
    rm -rf "$pkg_dir"
    mkdir -p "$pkg_dir/DEBIAN" "$pkg_dir/usr/bin" "$pkg_dir/lib/systemd/system" "$pkg_dir/usr/share/callmor-agent"
    cp target/release/callmor-agent "$pkg_dir/usr/bin/"
    cp packaging/deb/callmor-agent.service "$pkg_dir/lib/systemd/system/"
    cp "$template_src" "$pkg_dir/usr/share/callmor-agent/agent.conf.template"
    cp packaging/deb/postinst "$pkg_dir/DEBIAN/"
    cp packaging/deb/prerm "$pkg_dir/DEBIAN/"
    chmod 755 "$pkg_dir/DEBIAN/postinst" "$pkg_dir/DEBIAN/prerm"

    local installed_size=$(du -sk "$pkg_dir" | awk '{print $1}')
    cat > "$pkg_dir/DEBIAN/control" <<EOF
Package: $PKG_NAME
Version: $VERSION
Section: admin
Priority: optional
Architecture: $ARCH
Installed-Size: $installed_size
Depends: libgstreamer1.0-0, gstreamer1.0-plugins-base, gstreamer1.0-plugins-good, gstreamer1.0-plugins-bad, gstreamer1.0-plugins-ugly, gstreamer1.0-x, gstreamer1.0-nice, libxcb-xtest0
Maintainer: Callmor <support@callmor.ai>
Description: Callmor Remote Desktop Agent
 Cross-platform remote desktop agent that captures screen,
 encodes H.264, and streams via WebRTC. Supports mouse and
 keyboard input injection for full remote control.
Homepage: https://callmor.ai
EOF

    # Uncompressed so the API can byte-replace the placeholder (tenant mode).
    # For adhoc mode compression is technically fine, but staying consistent
    # keeps the two artifacts interchangeable in ops.
    dpkg-deb -Znone --root-owner-group --build "$pkg_dir" "$output" 2>&1 | tail -1
    ls -lh "$output" | awk '{print "    "$5, $9}'
}

build_deb "tenant" "target/deb"        "${PKG_NAME}_${VERSION}_${ARCH}.deb"        "packaging/deb/agent.conf.template"
build_deb "adhoc"  "target/deb-public" "${PKG_NAME}-public_${VERSION}_${ARCH}.deb" "packaging/deb/agent.conf.template.adhoc"

echo ""
echo "Done:"
echo "  Tenant .deb: target/deb/${PKG_NAME}_${VERSION}_${ARCH}.deb"
echo "  Public .deb: target/deb-public/${PKG_NAME}-public_${VERSION}_${ARCH}.deb"
