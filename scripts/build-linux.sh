#!/usr/bin/env bash
# Build and package PlayCraft for Linux, named like the ArtCraft apps:
#
#   dist/playcraft-<version>-linux-<arch>.AppImage        any distribution, everything inside
#   dist/playcraft-<version>-linux-<arch>.AppImage.zsync  lets AppImage updaters fetch new versions
#   dist/playcraft-<version>-linux-<arch>.deb             Debian, Ubuntu, Mint, Pop!_OS…
#   dist/playcraft-<version>-linux-<arch>.rpm             Fedora, openSUSE, RHEL, Rocky, Alma…
#   dist/playcraft-<version>-linux-<arch>.pkg.tar.zst     Arch, Manjaro, EndeavourOS…
#   dist/playcraft-<version>-linux-<arch>.tar.gz          anything else
#
# Run on Debian/Ubuntu (x86_64 or aarch64). To cross-build for another processor
# (e.g. riscv64), set TARGET=riscv64gc-unknown-linux-gnu: that makes .deb, .rpm and .tar.gz
# using `cross`.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
TARGET="${TARGET:-}"
if [ -n "$TARGET" ]; then
  ARCH="${TARGET%%-*}"
  case "$ARCH" in riscv64gc) ARCH=riscv64 ;; esac
  BIN="target/$TARGET/release/playcraft"
else
  ARCH=$(uname -m)
  BIN="target/release/playcraft"
fi
NAME="playcraft-$VERSION-linux-$ARCH"
echo "Building $NAME"

if command -v apt-get >/dev/null; then
  sudo apt-get update
  sudo apt-get install -y build-essential pkg-config libxkbcommon-dev curl file zstd \
    libarchive-tools desktop-file-utils rpm
  if [ -z "$TARGET" ]; then
    # Runtime engine and disc libraries (bundled into the AppImage, listed as package dependencies).
    sudo apt-get install -y libmpv2 || sudo apt-get install -y libmpv1
    sudo apt-get install -y libbluray2 libdvdnav4
    sudo apt-get install -y libaacs0 libbdplus0 || true
  fi
fi
if ! command -v cargo >/dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  . "$HOME/.cargo/env"
fi

# ---- build -------------------------------------------------------------------------
if [ -n "$TARGET" ]; then
  cargo install cross --locked 2>/dev/null || true
  cross build --release --target "$TARGET"
else
  cargo build --release
fi
mkdir -p dist

# ---- .deb --------------------------------------------------------------------------
cargo install cargo-deb --locked 2>/dev/null || true
if [ -n "$TARGET" ]; then
  cargo deb --no-build --no-strip --target "$TARGET" --variant cross --output "dist/$NAME.deb"
else
  cargo deb --no-build --output "dist/$NAME.deb"
fi

# ---- .rpm --------------------------------------------------------------------------
cargo install cargo-generate-rpm --locked 2>/dev/null || true
if [ -n "$TARGET" ]; then
  cargo generate-rpm --target "$TARGET" --output "dist/$NAME.rpm" || echo "rpm packaging skipped"
else
  cargo generate-rpm --output "dist/$NAME.rpm" || echo "rpm packaging skipped"
fi

# ---- .tar.gz -----------------------------------------------------------------------
STAGE="dist/$NAME"
rm -rf "$STAGE"
mkdir -p "$STAGE"
cp "$BIN" README.md LICENSE NOTICE.md CHANGELOG.md "$STAGE/"
cp packaging/playcraft.desktop assets/playcraft.png "$STAGE/"
cp -r docs "$STAGE/docs"
tar -C dist -czf "dist/$NAME.tar.gz" "$NAME"
rm -rf "$STAGE"

if [ -n "$TARGET" ]; then
  echo "Cross build done: see the dist folder."
  exit 0
fi

# ---- Arch .pkg.tar.zst -------------------------------------------------------------
PKG="dist/pkg-arch"
rm -rf "$PKG"
install -Dm755 "$BIN" "$PKG/usr/bin/playcraft"
install -Dm644 packaging/playcraft.desktop "$PKG/usr/share/applications/playcraft.desktop"
install -Dm644 assets/playcraft.png "$PKG/usr/share/icons/hicolor/512x512/apps/playcraft.png"
install -Dm644 LICENSE "$PKG/usr/share/licenses/playcraft/LICENSE"
install -Dm644 README.md "$PKG/usr/share/doc/playcraft/README.md"
SIZE=$(du -sb "$PKG" | cut -f1)
cat > "$PKG/.PKGINFO" <<EOF
pkgname = playcraft
pkgbase = playcraft
pkgver = $VERSION-1
pkgdesc = Free, open-source DVD, CD and Blu-ray player
url = https://github.com/Arrestis/PlayCraft
builddate = $(date +%s)
packager = PlayCraft contributors
size = $SIZE
arch = $ARCH
license = GPL-3.0-or-later
depend = mpv
depend = libbluray
depend = libdvdnav
optdepend = libaacs: playing AACS-protected Blu-ray discs (key file not included)
optdepend = libbdplus: BD+ protected Blu-ray discs
EOF
(
  cd "$PKG"
  LANG=C bsdtar -czf .MTREE --format=mtree \
    --options='!all,use-set,type,uid,gid,mode,time,size,md5,sha256,link' .PKGINFO usr
  LANG=C bsdtar --zstd -cf "../$NAME.pkg.tar.zst" .PKGINFO .MTREE usr
)
rm -rf "$PKG"

# ---- AppImage ----------------------------------------------------------------------
APPDIR="dist/AppDir"
rm -rf "$APPDIR"
install -Dm755 "$BIN" "$APPDIR/usr/bin/playcraft"
install -Dm644 packaging/playcraft.desktop "$APPDIR/usr/share/applications/playcraft.desktop"
install -Dm644 assets/playcraft.png "$APPDIR/usr/share/icons/hicolor/512x512/apps/playcraft.png"
LD_ARCH="$ARCH"
curl -sSfL --retry 4 -o dist/linuxdeploy "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-$LD_ARCH.AppImage"
chmod +x dist/linuxdeploy
# libmpv and the disc libraries are loaded while PlayCraft runs, so list them explicitly.
LIBS=()
for lib in libmpv.so.2 libmpv.so.1 libbluray.so.2 libbluray.so.3 libdvdnav.so.4 libaacs.so.0 libbdplus.so.0; do
  p=$(ldconfig -p | awk -v l="$lib" '$1 == l { print $NF; exit }')
  if [ -n "$p" ]; then LIBS+=(--library "$p"); fi
done
export APPIMAGE_EXTRACT_AND_RUN=1
export ARCH="$LD_ARCH"
export LDAI_OUTPUT="$NAME.AppImage"
export OUTPUT="$NAME.AppImage"
export LDAI_UPDATE_INFORMATION="gh-releases-zsync|Arrestis|PlayCraft|latest|playcraft-*-linux-$LD_ARCH.AppImage.zsync"
export UPDATE_INFORMATION="$LDAI_UPDATE_INFORMATION"
dist/linuxdeploy --appdir "$APPDIR" \
  --desktop-file packaging/playcraft.desktop \
  --icon-file assets/playcraft.png \
  "${LIBS[@]}" \
  --output appimage
mv -f "$NAME.AppImage" dist/ 2>/dev/null || true
mv -f "$NAME.AppImage.zsync" dist/ 2>/dev/null || true
rm -rf "$APPDIR" dist/linuxdeploy

echo "Done: see the dist folder."
ls -l dist
