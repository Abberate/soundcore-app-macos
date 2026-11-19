#!/usr/bin/env bash
# Builds "dist/Soundcore App.app" (ad-hoc signed).
#   scripts/bundle.sh             native architecture
#   scripts/bundle.sh --universal Apple Silicon + Intel (needs: rustup target add x86_64-apple-darwin aarch64-apple-darwin)
set -euo pipefail
cd "$(dirname "$0")/.."

APP="dist/Soundcore App.app"
BINS=(soundcore-app soundcorectl)
# Same minimum macOS for both architectures (matches LSMinimumSystemVersion).
export MACOSX_DEPLOYMENT_TARGET=11.0

if [[ "${1:-}" == "--universal" ]]; then
  for t in aarch64-apple-darwin x86_64-apple-darwin; do
    cargo build --release -p soundcore-app --bins --target "$t"
  done
  OUT=target/universal
  mkdir -p "$OUT"
  for b in "${BINS[@]}"; do
    lipo -create -output "$OUT/$b" \
      "target/aarch64-apple-darwin/release/$b" "target/x86_64-apple-darwin/release/$b"
  done
else
  cargo build --release -p soundcore-app --bins
  OUT=target/release
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
for b in "${BINS[@]}"; do cp "$OUT/$b" "$APP/Contents/MacOS/"; done
cp packaging/Info.plist "$APP/Contents/Info.plist"
printf 'APPL????' > "$APP/Contents/PkgInfo"

ICONSET="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z "$s" "$s" packaging/icon_1024.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  sips -z $((s * 2)) $((s * 2)) packaging/icon_1024.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"

codesign --force --deep --sign - "$APP"
echo "✓ $APP"
