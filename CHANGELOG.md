# Changelog

All notable changes to PlayCraft are listed here, newest first.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

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
