#!/usr/bin/env bash
# Make a drag-to-install disk image: PlayCraft.app, a shortcut to Applications, and the docs.
#   bash scripts/make-macos-dmg.sh <PlayCraft.app> <output.dmg>
set -euo pipefail
APP="$1"
DMG="$2"
cd "$(dirname "$0")/.."
STAGE=$(mktemp -d)
cp -R "$APP" "$STAGE/PlayCraft.app"
ln -s /Applications "$STAGE/Applications"
mkdir -p "$STAGE/Documentation"
cp README.md LICENSE NOTICE.md CHANGELOG.md "$STAGE/Documentation/"
cp docs/*.md "$STAGE/Documentation/"
rm -f "$DMG"
hdiutil create -volname "PlayCraft" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
rm -rf "$STAGE"
