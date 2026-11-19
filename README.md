# PlayCraft

A free, open-source disc and media player for **Windows, macOS and Linux**.

- **Discs:** DVD-Video, audio CD, Blu-ray, Ultra HD Blu-ray, plus `VIDEO_TS` / `BDMV` folders and ISO images
- **Files:** almost any video, music or photo format (MKV, MP4, AVI, MOV, M2TS, WebM, FLAC, MP3, JPG, HEIC…) through FFmpeg
- **Remote:** a pop-out remote styled like a Blu-ray remote, with a mini mode and resizing. Its volume keys can control the computer's own volume.
- **Disc menus:** DVD menus and Blu-ray (HDMV) menus, with mouse and keyboard control. Java-based (BD-J) Blu-ray menus fall back to the main movie.
- **Playback:** resume and restart, chapters and titles, audio and subtitle tracks, subtitle files, A–B repeat, speed, snapshots, full screen
- **Picture and sound:** brightness, contrast, colour and gamma; sharpen, denoise and deinterlace; Dolby/DTS passthrough, night mode, clearer dialogue
- **Library:** Movies, TV Shows, Videos, Photos and Music from your folders, playlists, and a My Computer browser

Playback runs on [libmpv](https://mpv.io), which includes FFmpeg, libdvdnav, libbluray and libcdio.

## Source code

The code lives in the GitHub repository **PlayCraft**: https://github.com/Arrestis/PlayCraft

To publish or update it from this folder, run:

```
powershell -ExecutionPolicy Bypass -File scripts\publish-to-github.ps1    # Windows
bash scripts/publish-to-github.sh                                         # macOS / Linux
```

You sign in through GitHub's own web page, so no password is typed into the script or stored. The script then creates the repository if it doesn't exist yet and uploads the code.

## Build

You need [Rust](https://rustup.rs).

```
cargo build --release
```

On **Windows**, the build puts everything needed in `target/release/engine/` next to the program, so nothing else has to be downloaded:

- `libmpv-2.dll`: FFmpeg with every common codec, plus DVD, Blu-ray and CD support
- `libaacs` and `libbdplus` with their helper libraries, for Blu-ray decryption

To build without this download, set `PLAYCRAFT_SKIP_ENGINE=1`. To share the program, ship `playcraft.exe` together with the `engine` folder.

On **Linux**, run this on a Debian or Ubuntu machine:

```
bash scripts/build-linux.sh
```

It creates `dist/playcraft_*.deb`, which installs the engine and disc libraries automatically through apt, and `dist/PlayCraft-linux-x64.tar.gz`.

On **macOS**, run this on a Mac with [Homebrew](https://brew.sh):

```
bash scripts/build-macos.sh
```

It creates `dist/PlayCraft.app` (the engine and Blu-ray libraries are inside it) and `dist/PlayCraft-macos.dmg`. The app is self-signed, so right-click it and choose **Open** the first time.

On GitHub, `.github/workflows/release.yml` runs all three builds automatically: a Windows zip, a Linux `.deb` and `.tar.gz`, and a macOS `.dmg`. Run it from the **Actions** tab, or push a tag such as `v0.1.0` to publish a release.

## Commercial Blu-ray discs

Unprotected and home-made Blu-rays play straight away. Most commercial Blu-rays are encrypted with AACS. The decryption **libraries** come with this player, but the **key file** (`KEYDB.cfg`) does not: those keys are licensed material and can't be shipped with open-source software. If you have a key file you are entitled to use, add it once under *Settings › Discs & Blu-ray › Import key file*. Ultra HD discs (AACS 2.0) need keys for each disc. Check the law where you live.

## Command line

```
playcraft [files, disc folders or ISOs…]
playcraft --test-picture              # play the engine's built-in test picture
playcraft --screenshot out.png        # draw ~120 frames, save the window, exit
```

## Licence

PlayCraft is free, open-source software under the **GNU General Public License v3.0 or later**; the full text is in `LICENSE`. Anyone who shares PlayCraft, or a program built from it, must share its source code under the same licence. Bundled components keep their own licences; see `NOTICE.md`.

## Activity log and changes

PlayCraft writes what it does to `playcraft.log` in its settings folder (start-up, playback engine, discs found, what played, disc menus, errors). Open it from *Settings › About › Open activity log*. Once the log grows past 2 MB, it starts a new file and keeps the previous one as `playcraft.old.log`.

Each version's changes are listed in `CHANGELOG.md`.
