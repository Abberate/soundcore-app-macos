#!/usr/bin/env bash
# Builds the installer: dist/Soundcore-App-<version>.dmg
# Universal app (Apple Silicon + Intel), Finder window with a drag-to-Applications
# background and step-by-step instructions, bilingual read-me included.
# Needs: rustup target add x86_64-apple-darwin aarch64-apple-darwin (and a logged-in GUI
# session: the background picture is rendered by the app itself).
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
NAME="Soundcore App"
APP="dist/$NAME.app"
VOLNAME="$NAME $VERSION"
README="Lisez-moi - Read me.txt"
OUT="dist/Soundcore-App-${VERSION}.dmg"

scripts/bundle.sh --universal

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
STAGE="$WORK/stage"
mkdir -p "$STAGE/.background"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
cp packaging/LISEZ-MOI.txt "$STAGE/$README"

# Background picture: rendered by the app (Retina 2x), 1x derived, combined into one TIFF.
SOUNDCORE_DMG_BG="$WORK/bg.png" "$APP/Contents/MacOS/soundcore-app" >/dev/null 2>&1 || true
[[ -s "$WORK/bg.png" ]] || { echo "background rendering failed" >&2; exit 1; }
W="$(sips -g pixelWidth "$WORK/bg.png" | awk '/pixelWidth/ {print $2}')"
if [[ "$W" -ge 1520 ]]; then
  sips -z 1140 1520 "$WORK/bg.png" --out "$WORK/bg@2x.png" >/dev/null
  sips -z 570 760 "$WORK/bg.png" --out "$WORK/bg1x.png" >/dev/null
  tiffutil -cathidpicheck "$WORK/bg1x.png" "$WORK/bg@2x.png" -out "$STAGE/.background/background.tiff" >/dev/null
else
  sips -s format tiff "$WORK/bg.png" --out "$STAGE/.background/background.tiff" >/dev/null
fi

# Read-write image, Finder layout written while mounted (hidden from Finder), then compressed.
if [[ -d "/Volumes/$VOLNAME" ]]; then hdiutil detach "/Volumes/$VOLNAME" -force >/dev/null; fi
hdiutil create -volname "$VOLNAME" -srcfolder "$STAGE" -fs HFS+ -format UDRW -ov "$WORK/rw.dmg" >/dev/null
MNT="$(hdiutil attach "$WORK/rw.dmg" -readwrite -nobrowse -noautoopen | awk -F'\t' '/\/Volumes\// {print $NF}' | tail -1)"
python3 -I scripts/dmg_layout.py "$MNT" "$NAME.app" "$README"
sync
hdiutil detach "$MNT" >/dev/null || hdiutil detach "$MNT" -force >/dev/null

rm -f "$OUT"
hdiutil convert "$WORK/rw.dmg" -format UDZO -imagekey zlib-level=9 -o "$OUT" >/dev/null
codesign --force --sign - "$OUT"

echo "✓ $OUT ($(du -h "$OUT" | cut -f1))"
shasum -a 256 "$OUT"
