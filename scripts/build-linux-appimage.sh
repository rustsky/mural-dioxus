#!/bin/sh
# Builds dist/Mural-<version>-<arch>.AppImage (Ubuntu 22.04+ or similar, with WebKitGTK 4.1,
# GTK 3 and GStreamer installed; see .github/workflows/build.yml for the package list).
#
# This follows what `dx bundle --package-types appimage` (tauri-bundler) does, with one addition
# dx does not expose: the GStreamer plugins, which WebKitGTK needs for microphone capture and
# audio playback. Mural is GPL-3.0-or-later: distribute the source archive with the AppImage.
set -eu
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
ARCH=$(uname -m)
TOOLS=${APPIMAGE_TOOLS_DIR:-$PWD/target/appimage-tools}
WORK=target/appimage
APPDIR=$WORK/Mural.AppDir

./scripts/rust-licenses.sh
cargo build --release --locked

# Tools: the same builds tauri-bundler downloads.
mkdir -p "$TOOLS"
fetch() { [ -s "$TOOLS/$1" ] || { curl -fsSL -o "$TOOLS/$1" "$2"; chmod +x "$TOOLS/$1"; }; }
fetch "AppRun-$ARCH" "https://github.com/AppImage/AppImageKit/releases/download/continuous/AppRun-$ARCH"
fetch "linuxdeploy-$ARCH.AppImage" "https://github.com/tauri-apps/binary-releases/releases/download/linuxdeploy/linuxdeploy-$ARCH.AppImage"
fetch linuxdeploy-plugin-gtk.sh "https://raw.githubusercontent.com/tauri-apps/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh"
fetch linuxdeploy-plugin-gstreamer.sh "https://raw.githubusercontent.com/tauri-apps/linuxdeploy-plugin-gstreamer/master/linuxdeploy-plugin-gstreamer.sh"
# Clear the AppImage magic bytes so linuxdeploy also runs where FUSE is unavailable (as tauri does).
dd if=/dev/zero bs=1 count=3 seek=8 conv=notrunc of="$TOOLS/linuxdeploy-$ARCH.AppImage" 2>/dev/null

rm -rf "$WORK"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" \
    "$APPDIR/usr/share/icons/hicolor/512x512/apps" "$APPDIR/usr/share/doc/mural"
cp target/release/mural "$APPDIR/usr/bin/mural"
cp icons/tile-512x512.png "$APPDIR/usr/share/icons/hicolor/512x512/apps/mural.png"
cp LICENSE LICENSE-MIT THIRD_PARTY_NOTICES.md THIRD_PARTY_RUST.txt "$APPDIR/usr/share/doc/mural/"
cat > "$APPDIR/usr/share/applications/mural.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Mural
Comment=The language app you eventually delete.
Exec=mural
Icon=mural
Categories=Education;Languages;
Terminal=false
StartupWMClass=mural
EOF

# WebKitGTK starts these helpers by path, so linuxdeploy cannot discover them; the gtk plugin
# rewrites WebKit's /usr paths to relative ones, which AppRun resolves from inside the AppImage.
for file in WebKitNetworkProcess WebKitWebProcess injected-bundle/libwebkit2gtkinjectedbundle.so; do
    for dir in "/usr/lib/$ARCH-linux-gnu" /usr/lib64 /usr/lib /usr/libexec; do
        source="$dir/webkit2gtk-4.1/$file"
        if [ -e "$source" ]; then
            mkdir -p "$APPDIR$(dirname "$source")"
            cp "$source" "$APPDIR$source"
        fi
    done
done

cp "$TOOLS/AppRun-$ARCH" "$APPDIR/AppRun"
cp "$APPDIR/usr/share/icons/hicolor/512x512/apps/mural.png" "$APPDIR/mural.png"
ln -sf mural.png "$APPDIR/.DirIcon"
ln -sf usr/share/applications/mural.desktop "$APPDIR/mural.desktop"

mkdir -p dist
OUTPUT="$PWD/dist/Mural-$VERSION-$ARCH.AppImage"
rm -f "$OUTPUT"
# WebRTC echo cancellation (webrtcdsp) lives in the "bad" GStreamer plugin set.
PATH="$TOOLS:$PATH" OUTPUT="$OUTPUT" ARCH="$ARCH" DEPLOY_GTK_VERSION=3 NO_STRIP=true \
GSTREAMER_INCLUDE_BAD_PLUGINS=1 \
    "$TOOLS/linuxdeploy-$ARCH.AppImage" --appimage-extract-and-run \
    --appdir "$APPDIR" --plugin gtk --plugin gstreamer --output appimage

echo "Built $OUTPUT"
