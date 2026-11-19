#!/usr/bin/env bash
# Build PlayCraft for Linux.
# Produces dist/playcraft_<version>_amd64.deb (installs the engine and disc libraries
# automatically through apt) and dist/PlayCraft-linux-x64.tar.gz.
set -euo pipefail
cd "$(dirname "$0")/.."

if command -v apt-get >/dev/null; then
  sudo apt-get update
  # Build tools plus the runtime engine (libmpv = FFmpeg, DVD, Blu-ray and CD support).
  sudo apt-get install -y build-essential pkg-config libxkbcommon-dev curl
  sudo apt-get install -y libmpv2 || sudo apt-get install -y libmpv1
  sudo apt-get install -y libbluray2 libdvdnav4
  sudo apt-get install -y libaacs0 libbdplus0 || true
fi

# libaacs (VideoLAN's open-source AACS library; it contains no keys). If the system has no
# package for it, build the official release from VideoLAN's download server.
AACS_VERSION=0.12.0
if ! ldconfig -p 2>/dev/null | grep -q 'libaacs\.so\.0'; then
  echo "Building libaacs $AACS_VERSION from VideoLAN source…"
  if command -v apt-get >/dev/null; then
    sudo apt-get install -y libgcrypt20-dev libgpg-error-dev bison flex meson ninja-build autoconf automake libtool
  fi
  SRC=$(mktemp -d)
  for ext in tar.xz tar.bz2; do
    if curl -sSfL "https://download.videolan.org/pub/videolan/libaacs/$AACS_VERSION/libaacs-$AACS_VERSION.$ext" -o "$SRC/libaacs.$ext"; then
      tar -xf "$SRC/libaacs.$ext" -C "$SRC"
      break
    fi
  done
  cd "$SRC/libaacs-$AACS_VERSION"
  if [ -f meson.build ]; then
    meson setup build --prefix=/usr/local && ninja -C build && sudo ninja -C build install
  else
    ./configure --prefix=/usr/local && make -j"$(nproc)" && sudo make install
  fi
  sudo ldconfig
  cd - >/dev/null
  rm -rf "$SRC"
fi

if ! command -v cargo >/dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  . "$HOME/.cargo/env"
fi

cargo build --release
mkdir -p dist

# .deb package
cargo install cargo-deb --locked 2>/dev/null || true
cargo deb --no-build --output dist/

# Plain archive for other distributions
STAGE=dist/PlayCraft
rm -rf "$STAGE"
mkdir -p "$STAGE"
cp target/release/playcraft README.md LICENSE "$STAGE/"
cp packaging/playcraft.desktop assets/playcraft.png "$STAGE/"
tar -C dist -czf dist/PlayCraft-linux-x64.tar.gz PlayCraft
rm -rf "$STAGE"
echo "Done: see the dist folder."
