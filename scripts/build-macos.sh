#!/usr/bin/env bash
# Build PlayCraft for macOS.
# Produces dist/PlayCraft.app (with the playback engine and disc libraries inside, so
# nothing else needs installing) and dist/playcraft-<version>-macos-<arm64|x86_64>.dmg.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v brew >/dev/null; then
  echo "Homebrew is needed: https://brew.sh" >&2
  exit 1
fi
brew install mpv libbluray libdvdnav libdvdcss libaacs libbdplus dylibbundler
if ! command -v cargo >/dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  . "$HOME/.cargo/env"
fi

cargo build --release

mkdir -p dist
APP="dist/PlayCraft.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS/engine" "$APP/Contents/Resources"
cp target/release/playcraft "$APP/Contents/MacOS/"

# Engine (FFmpeg, DVD, Blu-ray) and the Blu-ray decryption libraries, with all their dependencies.
cp "$(brew --prefix mpv)/lib/libmpv.2.dylib" "$APP/Contents/MacOS/engine/"
cp "$(brew --prefix libaacs)/lib/libaacs.0.dylib" "$APP/Contents/MacOS/engine/libaacs.dylib"
cp "$(brew --prefix libbdplus)/lib/libbdplus.0.dylib" "$APP/Contents/MacOS/engine/libbdplus.dylib"
# Disc menu navigators (Blu-ray HDMV menus and DVD menus).
BLURAY_LIB=$(ls "$(brew --prefix libbluray)"/lib/libbluray.*.dylib | head -n1)
DVDNAV_LIB=$(ls "$(brew --prefix libdvdnav)"/lib/libdvdnav.*.dylib | head -n1)
cp "$BLURAY_LIB" "$DVDNAV_LIB" "$APP/Contents/MacOS/engine/"
chmod u+w "$APP"/Contents/MacOS/engine/*
dylibbundler -of -b \
  -x "$APP/Contents/MacOS/engine/libmpv.2.dylib" \
  -x "$APP/Contents/MacOS/engine/$(basename "$BLURAY_LIB")" \
  -x "$APP/Contents/MacOS/engine/$(basename "$DVDNAV_LIB")" \
  -x "$APP/Contents/MacOS/engine/libaacs.dylib" \
  -x "$APP/Contents/MacOS/engine/libbdplus.dylib" \
  -d "$APP/Contents/MacOS/engine/" -p @loader_path/

# App icon from assets/icon.jpg
ICONSET=dist/icon.iconset
rm -rf "$ICONSET"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -s format png -z $s $s assets/icon.jpg --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2))
  sips -s format png -z $d $d assets/icon.jpg --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf "$ICONSET"

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>PlayCraft</string>
  <key>CFBundleDisplayName</key><string>PlayCraft</string>
  <key>CFBundleIdentifier</key><string>org.playcraft.player</string>
  <key>CFBundleExecutable</key><string>playcraft</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
EOF

# Ad-hoc signature so macOS will open it (right-click > Open the first time).
codesign --force --deep --sign - "$APP" || true

ARCH=$(uname -m)   # arm64 = Apple Silicon, x86_64 = Intel
# Keep the .app too, so scripts/make-macos-universal.sh can combine both processors.
tar -C dist -czf "dist/playcraft-app-$ARCH.tar.gz" PlayCraft.app
bash scripts/make-macos-dmg.sh "$APP" "dist/playcraft-$VERSION-macos-$ARCH.dmg"
echo "Done: see the dist folder."
