#!/usr/bin/env bash
# Builds the iOS app with the Dioxus CLI, unsigned.
#
#   ./scripts/build-ios.sh          # dist/Mural-<version>-ios-unsigned.ipa, for iPhones
#   ./scripts/build-ios.sh --sim    # a .app for Xcode's iOS Simulator
#
# The .ipa is deliberately unsigned: iOS only installs an app signed for the person and phone, so
# each user signs it with their own Apple ID when installing it (Sideloadly or AltStore; see the
# README). Needs Xcode with the iOS platform, the Rust targets (rustup target add aarch64-apple-ios
# aarch64-apple-ios-sim) and dx matching the dioxus version in Cargo.lock.
set -euo pipefail
cd "$(dirname "$0")/.."

target=aarch64-apple-ios
slice=ios-arm64
sim=false
if [ "${1:-}" = "--sim" ]; then target=aarch64-apple-ios-sim; slice=ios-arm64_x86_64-simulator; sim=true; fi

# sherpa-onnx-sys finds its output folder by looking for a directory named after Cargo's profile
# above its OUT_DIR, but dx builds iOS under ios-release; a target directory ending in "release"
# satisfies it (the same workaround as scripts/build-android.sh).
export CARGO_TARGET_DIR="$PWD/target/ios/release"

# Without --codesign or a device, dx leaves the app unsigned.
dx build --platform ios --target "$target" --release

app=$(find "$CARGO_TARGET_DIR/dx/mural/release/ios" -name "*.app" -type d -prune | head -1)
[ -n "$app" ] || { echo "dx didn't produce an app." >&2; exit 1; }

# sherpa-onnx is a framework named @rpath/SherpaOnnxC.framework/SherpaOnnxC; build.rs makes the app
# look in its Frameworks folder, and it has to be there.
framework=$(find "$CARGO_TARGET_DIR/sherpa-onnx-prebuilt" -type d -path "*SherpaOnnxC.xcframework/$slice/SherpaOnnxC.framework" | head -1)
[ -n "$framework" ] || { echo "No SherpaOnnxC.framework for $slice." >&2; exit 1; }
mkdir -p "$app/Frameworks"
rm -rf "$app/Frameworks/SherpaOnnxC.framework"
cp -R "$framework" "$app/Frameworks/"

# dx doesn't give iOS apps an icon, so compile one: a single 1024-point image that iOS scales, and
# the CFBundleIcons entries that point the home screen at it.
platform=iphoneos
if $sim; then platform=iphonesimulator; fi
catalog=$(mktemp -d)
mkdir -p "$catalog/Assets.xcassets/AppIcon.appiconset"
cp icons/ios-1024.png "$catalog/Assets.xcassets/AppIcon.appiconset/"
cat > "$catalog/Assets.xcassets/AppIcon.appiconset/Contents.json" <<'JSON'
{"images": [{"idiom": "universal", "platform": "ios", "size": "1024x1024", "filename": "ios-1024.png"}], "info": {"author": "xcode", "version": 1}}
JSON
xcrun actool "$catalog/Assets.xcassets" --compile "$app" --platform "$platform" \
  --minimum-deployment-target 16.0 --app-icon AppIcon --output-partial-info-plist "$catalog/icon.plist" \
  --output-format human-readable-text --notices --warnings >/dev/null
/usr/libexec/PlistBuddy -c "Merge $catalog/icon.plist" "$app/Info.plist" >/dev/null
rm -rf "$catalog"

if $sim; then
  # The Simulator runs unsigned code only with an ad-hoc signature.
  codesign --force --deep --sign - "$app"
  echo "$app"
  echo "Install with: xcrun simctl install booted \"$app\""
  exit 0
fi

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
ipa="$PWD/dist/Mural-$version-ios-unsigned.ipa"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/Payload" dist
cp -R "$app" "$work/Payload/"
rm -f "$ipa"
(cd "$work" && zip -qry "$ipa" Payload)
echo "dist/Mural-$version-ios-unsigned.ipa"
