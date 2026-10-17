#!/usr/bin/env bash
set -euo pipefail

export ANDROID_HOME="${ANDROID_HOME:-/home/polymath/Android/Sdk}"
unset ANDROID_SDK_ROOT || true
export NDK_HOME="${NDK_HOME:-$ANDROID_HOME/ndk/28.2.13676358}"
export ANDROID_NDK_HOME="$NDK_HOME"
export PATH="$HOME/.cargo/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin:$PATH"

echo "=== Building PdfCraft Android APK ==="
echo "ANDROID_HOME: $ANDROID_HOME"
echo "NDK_HOME:     $NDK_HOME"

# Ensure target is installed
rustup target add aarch64-linux-android

# Ensure cargo-apk is installed
if ! command -v cargo-apk >/dev/null 2>&1; then
    echo "Installing cargo-apk..."
    cargo install cargo-apk
fi

# Build APK
mkdir -p dist
cd apps/pdfcraft-android
cargo apk build --release

# Find generated apk and copy to dist/
if [ -f "../../target/release/apk/pdfcraft_android.apk" ]; then
    cp "../../target/release/apk/pdfcraft_android.apk" ../../dist/pdfcraft-mobile.apk
elif [ -f "../../target/release/apk/pdfcraft-android.apk" ]; then
    cp "../../target/release/apk/pdfcraft-android.apk" ../../dist/pdfcraft-mobile.apk
else
    find ../../target -name "*.apk" -exec cp {} ../../dist/pdfcraft-mobile.apk \;
fi

echo "=== Success! APK generated at dist/pdfcraft-mobile.apk ==="
ls -lh ../../dist/pdfcraft-mobile.apk
