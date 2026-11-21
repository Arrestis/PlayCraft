PlayCraft is a free, open-source DVD, CD and Blu-ray player for Windows, macOS and Linux
(GPL-3.0).

## Downloads

| System | Installer | Other |
|---|---|---|
| Windows 64-bit, Intel / AMD | `playcraft-0.3.4-windows-x64.msi` | `playcraft-0.3.4-windows-x64-portable.zip` |
| Windows on ARM (Snapdragon, etc.) | `playcraft-0.3.4-windows-arm64.msi` | `playcraft-0.3.4-windows-arm64-portable.zip` |
| Windows 32-bit (x86) | `playcraft-0.3.4-windows-x86.msi` | `playcraft-0.3.4-windows-x86-portable.zip` |
| macOS (Apple Silicon and Intel) | `playcraft-0.3.4-macos-universal.dmg` | `-macos-arm64.dmg`, `-macos-x86_64.dmg` |
| Linux, any distribution | `playcraft-0.3.4-linux-x86_64.AppImage`, `-linux-aarch64.AppImage` | `.AppImage.zsync` for updaters |
| Debian, Ubuntu, Mint, Pop!_OS | `playcraft-0.3.4-linux-x86_64.deb`, `-aarch64.deb`, `-riscv64.deb` | |
| Fedora, openSUSE, RHEL, Rocky | `playcraft-0.3.4-linux-x86_64.rpm`, `-aarch64.rpm`, `-riscv64.rpm` | |
| Arch, Manjaro, EndeavourOS | `playcraft-0.3.4-linux-x86_64.pkg.tar.zst`, `-aarch64.pkg.tar.zst` | |
| Other Linux | `playcraft-0.3.4-linux-<arch>.tar.gz` | |

`SHA256SUMS.txt` lists a checksum for every file.

To install, see [docs/INSTALL.md](https://github.com/Arrestis/PlayCraft/blob/main/docs/INSTALL.md).
For how to use PlayCraft, see [docs/USER_GUIDE.md](https://github.com/Arrestis/PlayCraft/blob/main/docs/USER_GUIDE.md).

## What's new in 0.3.4

- **Clean disc menus:** no hint text or player messages are drawn over a disc's menu or the movie any more (subtitles still show).
- **Blu-ray codec information link:** a link to https://fvonline-db.bplaced.net/ on the Discs & Blu-ray settings page and on Blu-ray disc pages.

## Also in this release (from 0.3.2 and 0.3.3)

- **Fixed the blank icon after installing** (Start menu, desktop and Apps list).
- **Fixes "The codec engine is not installed yet … (os error 126)" on other PCs.** The engine needed the Vulkan loader (`vulkan-1.dll`), which many PCs don't have. It is now included, so the built-in engine works on any Windows 10/11 PC.
- **Starts on more PCs** (from 0.3.1): falls back to OpenGL graphics when needed, and explains any start-up problem.
- Every Windows download is now tested on a fresh machine during GitHub's build.

The full list of changes is in [CHANGELOG.md](https://github.com/Arrestis/PlayCraft/blob/main/CHANGELOG.md).