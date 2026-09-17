#!/bin/sh
# Builds dist/Mural.app, dist/Mural-<version>-<arch>.dmg and dist/Mural-<version>-source.tar.gz.
# Mural is GPL-3.0-or-later: always distribute the source archive alongside the app.
# SIGN_IDENTITY="Developer ID Application: …" signs with hardened runtime; the default is ad-hoc.
set -eu
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
IDENTITY=${SIGN_IDENTITY:--}

./scripts/rust-licenses.sh
dx bundle --release --package-types macos
APP=target/dx/mural/bundle/macos/bundle/macos/Mural.app
ARCH=$(lipo -archs "$APP/Contents/MacOS/mural" | tr ' ' '-')

rm -rf dist && mkdir -p dist
cp -R "$APP" dist/Mural.app
xattr -cr dist/Mural.app

if [ "$IDENTITY" = "-" ]; then
    codesign --force --sign - --identifier chat.mural.desktop --entitlements macos/Mural.entitlements dist/Mural.app
else
    codesign --force --options runtime --timestamp --sign "$IDENTITY" --identifier chat.mural.desktop \
        --entitlements macos/Mural.entitlements dist/Mural.app
fi
codesign --verify --strict --verbose=2 dist/Mural.app

STAGE=$(mktemp -d)
cp -R dist/Mural.app "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -quiet -volname Mural -srcfolder "$STAGE" -ov -format UDZO "dist/Mural-$VERSION-$ARCH.dmg"
rm -rf "$STAGE"

# Corresponding source: every tracked or new, non-ignored file in the working tree.
git ls-files --cached --others --exclude-standard \
    | while IFS= read -r file; do case "$file" in *.DS_Store) continue;; esac; [ -e "$file" ] && printf '%s\n' "$file"; done \
    | tar -T - -czf "dist/Mural-$VERSION-source.tar.gz" -s ",^,mural-desktop-$VERSION/,"
echo "Built dist/Mural.app, dist/Mural-$VERSION-$ARCH.dmg and dist/Mural-$VERSION-source.tar.gz"
