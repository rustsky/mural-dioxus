#!/usr/bin/env bash
# Builds the Android app (an APK) with the Dioxus CLI.
#
#   ./scripts/build-android.sh            # debug APK, arm64
#   ./scripts/build-android.sh --release  # optimized APK, about 23 MB (debug-signed unless [android] signing is set up)
#
# Needs Java 17, the Android SDK with an NDK, the Rust target (rustup target add
# aarch64-linux-android) and dx matching the dioxus version in Cargo.lock. ANDROID_HOME, and the
# NDK inside it, are found automatically on a Mac with Android Studio's default SDK location.
set -euo pipefail
cd "$(dirname "$0")/.."

profile=debug
args=()
if [ "${1:-}" = "--release" ]; then profile=release; args+=(--release); fi

export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$(ls -d "$ANDROID_HOME"/ndk/* 2>/dev/null | sort -V | tail -1)}"
if [ -z "${JAVA_HOME:-}" ] && [ -d /opt/homebrew/opt/openjdk@17 ]; then
  export JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home
fi
[ -d "$ANDROID_NDK_HOME" ] || { echo "No Android NDK found; set ANDROID_NDK_HOME." >&2; exit 1; }

# sherpa-onnx-sys finds its output folder by looking for a directory named after Cargo's profile
# ("debug" or "release") above its OUT_DIR, but dx builds Android under android-dev and
# android-release. A target directory ending in that name satisfies it; see build.rs for how the
# sherpa-onnx libraries then get into the APK.
export CARGO_TARGET_DIR="$PWD/target/android/$profile"

# CI's build cache restores Cargo's record of sherpa-onnx-sys but not the libraries its build script
# unpacked into sherpa-onnx-prebuilt, so Cargo skips the script and linking fails. Without them,
# forget the record and the script runs again.
if [ -z "$(find -L "$CARGO_TARGET_DIR/sherpa-onnx-prebuilt" -type f -path "*jniLibs/arm64-v8a/libsherpa-onnx-c-api.so" 2>/dev/null | head -1)" ]; then
  # The cache can also bring back the folders empty, and the script takes an existing folder as
  # already unpacked, so it has to go too.
  rm -rf "${CARGO_TARGET_DIR:?}/sherpa-onnx-prebuilt"
  find "$CARGO_TARGET_DIR" -path "*/.fingerprint/sherpa-onnx-sys-*" -prune -exec rm -rf {} + 2>/dev/null || true
fi

dx build --platform android --target aarch64-linux-android ${args[@]+"${args[@]}"}

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
apk=$(find "$CARGO_TARGET_DIR/dx/mural/$profile/android" -name "*.apk" -path "*outputs/apk*" | head -1)
[ -n "$apk" ] || { echo "dx didn't produce an APK." >&2; exit 1; }
mkdir -p dist
cp "$apk" "dist/Mural-$version-arm64-$profile.apk"
echo "dist/Mural-$version-arm64-$profile.apk"
