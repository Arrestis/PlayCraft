# Building PlayCraft from source

PlayCraft is written in Rust with [egui](https://github.com/emilk/egui). Playback uses
libmpv (FFmpeg, libdvdnav, libbluray, libcdio), loaded when the program runs, so building
needs only Rust.

## Requirements

- [Rust](https://rustup.rs) (stable)
- Windows: Visual Studio Build Tools (C++), plus `curl.exe` and `tar.exe`, which come with
  Windows 10/11
- Linux: `build-essential pkg-config libxkbcommon-dev`
- macOS: Xcode command-line tools and [Homebrew](https://brew.sh)

## Windows

```
cargo build --release                                   # 64-bit (x64 / AMD64)
rustup target add i686-pc-windows-msvc
cargo build --release --target i686-pc-windows-msvc      # 32-bit (x86)
```

`build.rs` downloads and bundles the engine next to the program, in `engine/`:

- libmpv from [shinchiro/mpv-winbuild-cmake](https://github.com/shinchiro/mpv-winbuild-cmake)
- libbluray, libdvdnav, libdvdread, libdvdcss, libaacs, libbdplus and every library they
  depend on, from [MSYS2](https://www.msys2.org)

To skip this download, set `PLAYCRAFT_SKIP_ENGINE=1`. To make a portable build, zip
`playcraft.exe` together with its `engine` folder.

## Linux

```
bash scripts/build-linux.sh
```

It installs the build and runtime packages through apt, then writes the `.deb`, `.rpm` and
`.tar.gz` files to `dist/`.

## macOS

```
bash scripts/build-macos.sh
```

It installs mpv, libbluray, libdvdnav, libaacs and libbdplus with Homebrew. It then
bundles them inside `PlayCraft.app` and writes a `.dmg` to `dist/`.

## Releases

`.github/workflows/release.yml` builds every package on GitHub:

- Windows x64 and x86 (portable zip)
- Linux x86_64 and aarch64 (.deb, .rpm and .tar.gz)
- macOS Apple Silicon and Intel (.dmg)

Pushing a tag publishes a release with all of them attached:

```
git tag v0.2.0
git push origin v0.2.0
```

## Source layout

| File | What it does |
|---|---|
| `src/main.rs` | Start-up, window options, `--screenshot` |
| `src/app.rs` | Program state, playback, disc and file handling |
| `src/ui.rs` | Sidebar, control bar, remote, keyboard |
| `src/pages.rs` | Home, My Computer, disc, library, playlist and settings pages |
| `src/mpv.rs` | Loads libmpv and draws its video into the window |
| `src/nav.rs` | DVD and Blu-ray menu navigation (libdvdnav, libbluray) |
| `src/discs.rs` | Finding drives and recognising discs |
| `src/library.rs` | Settings, media library, engine and libaacs installers |
| `src/aacs.rs` | Finding libaacs and its version |
| `src/sysvol.rs` | The computer's own volume and mute |
| `src/log.rs` | Activity log |
| `src/icons.rs`, `src/icon_mask.rs` | Drawn icons, app icon |

## Command line

```
playcraft [files, disc folders or ISOs…]
playcraft --test-picture          # engine test picture
playcraft --screenshot out.png    # draw ~120 frames, save the window, exit
```

## Licence

GPL-3.0-or-later. Contributions are accepted under the same licence.
