# Changelog

All notable changes to PlayCraft are listed here, newest first.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.3.3] - 2026-11-21

### Fixed
- **Blank icon after installing.** The icon file stored every size as PNG, which some parts of
  Windows (such as installer shortcuts) show blank. It now uses classic bitmap images for the
  small sizes, with PNG only for the large 256-pixel size. The installer's icon entry also now
  ends in `.ico`, as Windows needs for Start-menu and desktop shortcuts.
- All platforms (Windows x64, ARM64 and x86; macOS; Linux) are rebuilt with the 0.3.2
  engine fix and this icon fix.
## [0.3.2] - 2026-11-21

### Fixed
- **"The codec engine is not installed yet … The specified module could not be found
  (os error 126)" on other PCs.** The playback engine needs the Vulkan loader
  (`vulkan-1.dll`). Most graphics drivers install it, so the engine worked on the
  development PC but not on PCs without it. PlayCraft now ships the open-source Khronos
  Vulkan loader with the engine and loads the engine's helper libraries from the engine's
  own folder first, so it no longer depends on what the PC has installed.
- *Install codec engine* now also installs the Vulkan loader.
- If the engine still can't load, the message names the exact file that is missing.

### Added
- `playcraft --check-engine` loads the playback engine and the disc libraries and reports
  each one. GitHub's release build now runs it on a fresh Windows machine for every
  Windows download, so a build that wouldn't work on a clean PC is caught before release.
## [0.3.1] - 2026-11-21

### Fixed
- **PlayCraft wouldn't start on some PCs.** If the modern graphics system (Direct3D 12 /
  Vulkan) isn't available, which is common on older graphics, virtual machines and remote
  desktops, PlayCraft now restarts itself with OpenGL and remembers that for next time.
- Start-up problems and crashes now show a message with the reason, and are written to the
  activity log, instead of PlayCraft closing silently.
- **Install codec engine works where it used to fail.** It now downloads with Windows' own
  curl.exe and unpacks with tar.exe (or 7-Zip), instead of PowerShell scripts that security
  software and proxy settings often block. If it still fails, the message says exactly why.

### Added
- `playcraft --install-engine-to <folder>` installs the playback engine from the command line.
## [0.3.0] - 2026-11-20

### Changed
- **The playback engine is built into the program.** On Windows, `playcraft.exe` now carries
  libmpv (FFmpeg), libbluray, libdvdnav, libdvdread, libdvdcss, libaacs and libbdplus inside
  itself. The portable download is a single program with no `engine` folder to keep beside it
  or lose. On first start, PlayCraft unpacks the engine to
  `%LOCALAPPDATA%\PlayCraft\runtime\<processor>-<id>\`. Each version and processor gets its own
  folder, so a copy for another version or processor is never used.
- **Fewer libraries.** The build keeps only the libraries something actually uses: 25 instead
  of 49 on 64-bit Windows.

### Fixed
- "The codec engine is not installed yet … LoadLibraryExW failed": PlayCraft skips engine files
  built for a different processor, explains which one it found, and lists every place it
  looked. *Install codec engine* downloads the engine for PlayCraft's own processor.
- Windows builds no longer need the Microsoft Visual C++ runtime.
## [0.2.0] - 2026-11-20

### Added
- **Disc menus.** DVD menus (libdvdnav) and Blu-ray HDMV menus (libbluray) open when a disc
  starts. Choose buttons with the mouse, the arrow keys and Enter, or the remote.
  Java-based (BD-J) Blu-ray menus are not supported and go straight to the main movie.
- "Play movie (skip menus)" button on the disc page, and a setting to turn menus off.
- **Remote control**, styled like a Blu-ray remote:
  - Keys: Top Menu, Pop Up/Menu, Return, Options, a diamond direction pad with a centre select
    key, Home, rewind/play/fast-forward, previous/pause/stop/next, Subtitle, Audio, a volume
    rocker and Mute.
  - Size: − and + make it smaller or bigger, and a popped-out remote rescales when you
    resize its window.
  - **Mini remote** with the direction pad, menu/back, play/stop and volume keys.
  - **Pop out** into its own always-on-top window that can sit anywhere on the desktop,
    even outside PlayCraft. Dragging the strip at the top of the docked remote pops it out.
  - **C / P keys** choose whether the volume and mute keys control the **C**omputer's own
    volume or only **P**layCraft's.
- **Activity log**: PlayCraft records what it does (start-up, playback engine, discs found,
  what played, disc menus, errors) in `playcraft.log` next to its settings. To find it,
  open Settings › About › Open activity log.
- This changelog, the full GPL-3.0 licence text (`LICENSE`), and `NOTICE.md` listing the
  bundled libraries' licences.
- Windows builds now also bundle libbluray, libdvdnav, libdvdread and libdvdcss, resolving
  their dependencies automatically and falling back to other download mirrors if one fails.
- New app icon.
- **Installers and downloads for every system**, named like the ArtCraft apps
  (`playcraft-<version>-<system>-<processor>.<type>`):
  - Windows `.msi` installers (Start-menu and desktop shortcuts) and portable `.zip`s for x64,
    ARM64 and x86. Each bundles the engine and disc libraries for its own processor.
  - macOS universal `.dmg`, plus Apple Silicon-only and Intel-only versions.
  - Linux AppImage (with `.zsync` updates), `.deb`, `.rpm`, Arch `.pkg.tar.zst` and `.tar.gz`
    for x86_64 and aarch64, and `.deb`/`.rpm`/`.tar.gz` for riscv64.
  - `SHA256SUMS.txt`; all built by GitHub Actions and attached to each release.
- Fixed the engine version shown in 32-bit builds.
- Documentation: `docs/INSTALL.md`, `docs/USER_GUIDE.md` and `docs/BUILDING.md`.

### Fixed
- In Blu-ray menus, moving the pointer no longer leaves every button it passed over
  highlighted.
- Choosing a disc-menu button (for example Scene Selection) no longer freezes the app.
- Disc-menu buttons no longer stay on the picture once the film starts playing.
- Switching the popped-out remote between full and mini no longer shrinks it.
- "Codec engine not installed … LoadLibraryExW failed": PlayCraft now skips engine files built
  for a different processor and says so plainly. Install codec engine now downloads the engine
  for PlayCraft's own processor (x64, x86 or ARM64), so it actually fixes the problem.
- Windows builds no longer need the Microsoft Visual C++ runtime.

## [0.1.0] - 2026-11-19

### Added
- First release, first named OpenDisc Player and then renamed **PlayCraft**.
- Plays DVD, audio CD, Blu-ray and Ultra HD Blu-ray discs, VIDEO_TS / BDMV folders, ISO
  images, and almost any video, music or photo file through libmpv (FFmpeg).
- Layout: grey sidebar (Home, Media Library: Movies, TV Shows, Videos, Photos, Music,
  Playlists; Source: My Computer and inserted discs), a My Computer browser and a
  control bar.
- Disc detection with automatic notices, eject, resume and restart, chapters and titles,
  audio and subtitle tracks, subtitle files, A–B repeat, speed, snapshots, full screen and
  media information.
- Picture settings (brightness, contrast, colour, gamma, sharpen, denoise, deinterlace)
  and sound settings (speaker layout, Dolby/DTS passthrough, night mode, clearer
  dialogue).
- Library scanning, search and playlists; keyboard shortcuts; drag and drop.
- Windows builds bundle the playback engine and libaacs/libbdplus, so nothing else needs
  downloading. There are in-app installers for the engine and for libaacs.
- Blu-ray key file import (keys are not included). For copy-protected discs, a
  "licensed player" hand-off.
- Linux (.deb and .tar.gz) and macOS (.app and .dmg) build scripts, and a GitHub Actions
  workflow that builds all three systems.
- Open-network-address option for stream links.
