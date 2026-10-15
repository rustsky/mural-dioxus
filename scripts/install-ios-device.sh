#!/usr/bin/env bash
# Builds Mural for iPhone, signs it with your own Apple development certificate and provisioning
# profile, and installs it on the iPhone connected to this Mac.
#
#   ./scripts/install-ios-device.sh [path/to/profile.mobileprovision]
#
# Without an argument it uses the newest provisioning profile Xcode has downloaded. Getting one with
# a free Apple ID: in Xcode, add the Apple ID under Settings → Accounts, make any iOS App project,
# pick your team and a bundle identifier of your own (e.g. com.yourname.mural), and run it once on
# the iPhone. The app installed here takes that bundle identifier.
set -euo pipefail
cd "$(dirname "$0")/.."

profile=${1:-}
if [ -z "$profile" ]; then
  profile=$(ls -t "$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles/"*.mobileprovision \
    "$HOME/Library/MobileDevice/Provisioning Profiles/"*.mobileprovision 2>/dev/null | head -1 || true)
fi
[ -n "$profile" ] && [ -f "$profile" ] || { echo "No provisioning profile found; see the note at the top of this script." >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
security cms -D -i "$profile" > "$work/profile.plist"
read_profile() { /usr/libexec/PlistBuddy -c "Print :$1" "$work/profile.plist"; }
team=$(read_profile "TeamIdentifier:0")
app_id=$(read_profile "Entitlements:application-identifier")
bundle_id=${app_id#"$team."}
case "$bundle_id" in *"*"*) bundle_id=$(sed -n 's/^identifier = "\(.*\)"/\1/p' Dioxus.toml | head -1) ;; esac
/usr/libexec/PlistBuddy -x -c "Print :Entitlements" "$work/profile.plist" > "$work/entitlements.plist"

identity=$(security find-identity -v -p codesigning | awk -F'"' '/Apple Development/ {print $2; exit}')
[ -n "$identity" ] || { echo "No Apple Development certificate; add your Apple ID in Xcode → Settings → Accounts." >&2; exit 1; }

device=$(xcrun devicectl list devices 2>/dev/null | awk '/iPhone|iPad/ && !/simulated/ && /connected|available/ {for (i = 1; i <= NF; i++) if ($i ~ /^[0-9A-F]{8}-/) {print $i; exit}}')
[ -n "$device" ] || { echo "No iPhone connected (or it isn't trusted / in Developer Mode)." >&2; exit 1; }

./scripts/build-ios.sh >/dev/null
built="$PWD/target/ios/release/dx/mural/release/ios/Mural.app"
app="$work/Mural.app"
cp -R "$built" "$app"

/usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $bundle_id" "$app/Info.plist"
cp "$profile" "$app/embedded.mobileprovision"
codesign --force --sign "$identity" "$app/Frameworks/SherpaOnnxC.framework"
codesign --force --sign "$identity" --entitlements "$work/entitlements.plist" "$app"

echo "Installing $bundle_id, signed by $identity"
xcrun devicectl device install app --device "$device" "$app"
xcrun devicectl device process launch --device "$device" "$bundle_id" >/dev/null 2>&1 || \
  echo "Installed. If it won't open, trust your Apple ID under Settings → General → VPN & Device Management."
