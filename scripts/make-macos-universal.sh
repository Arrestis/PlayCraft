#!/usr/bin/env bash
# Combine the Apple Silicon and Intel builds into one universal PlayCraft.app and
# dist/playcraft-<version>-macos-universal.dmg (runs natively on every Mac).
#   bash scripts/make-macos-universal.sh dist/playcraft-app-arm64.tar.gz dist/playcraft-app-x86_64.tar.gz
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
ARM_TGZ="$1"
X86_TGZ="$2"
WORK=$(mktemp -d)
mkdir -p "$WORK/arm" "$WORK/x86"
tar -C "$WORK/arm" -xzf "$ARM_TGZ"
tar -C "$WORK/x86" -xzf "$X86_TGZ"
ARM="$WORK/arm/PlayCraft.app"
X86="$WORK/x86/PlayCraft.app"
OUT="$WORK/PlayCraft.app"
cp -R "$ARM" "$OUT"

# Merge every Mach-O file (the program and each engine library). Files that only one
# build has are copied as they are.
while IFS= read -r -d '' f; do
  rel="${f#"$ARM"/}"
  if [ -f "$X86/$rel" ] && file "$f" | grep -q 'Mach-O'; then
    lipo -create "$f" "$X86/$rel" -output "$OUT/$rel"
  fi
done < <(find "$ARM/Contents/MacOS" -type f -print0)
while IFS= read -r -d '' f; do
  rel="${f#"$X86"/}"
  if [ ! -e "$OUT/$rel" ]; then
    mkdir -p "$(dirname "$OUT/$rel")"
    cp "$f" "$OUT/$rel"
  fi
done < <(find "$X86/Contents/MacOS" -type f -print0)

codesign --force --deep --sign - "$OUT" || true
lipo -info "$OUT/Contents/MacOS/playcraft"
mkdir -p dist
bash scripts/make-macos-dmg.sh "$OUT" "dist/playcraft-$VERSION-macos-universal.dmg"
rm -rf "$WORK"
echo "Made dist/playcraft-$VERSION-macos-universal.dmg"
