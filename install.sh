#!/usr/bin/env bash
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
BINDIR="$PREFIX/bin"
APPDIR="$PREFIX/share/applications"
ICONDIR="$PREFIX/share/icons/hicolor/scalable/apps"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

refresh_caches() {
    command -v update-desktop-database >/dev/null && update-desktop-database "$APPDIR" 2>/dev/null || true
    command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" 2>/dev/null || true
}

if [ "${1:-}" = "--uninstall" ]; then
    rm -f "$BINDIR/annotshot" "$APPDIR/net.gappc.Annotshot.desktop" "$ICONDIR/net.gappc.Annotshot.svg"
    refresh_caches
    echo "Annotshot uninstalled from $PREFIX"
    exit 0
fi

cd "$HERE"
if [ -x annotshot ] && [ ! -f Cargo.toml ]; then
    BIN=annotshot                       # prebuilt binary from a release tarball
else
    cargo build --release
    BIN=target/release/annotshot
fi
mkdir -p "$BINDIR" "$APPDIR" "$ICONDIR"
install -m 755 "$BIN" "$BINDIR/annotshot"
sed "s|@BINDIR@|$BINDIR|" data/net.gappc.Annotshot.desktop > "$APPDIR/net.gappc.Annotshot.desktop"
install -m 644 data/net.gappc.Annotshot.svg "$ICONDIR/net.gappc.Annotshot.svg"
refresh_caches
echo "Annotshot installed to $PREFIX"
