# Installing PlayCraft

Download the file for your computer from the
[Releases page](https://github.com/Arrestis/PlayCraft/releases). Files are named
`playcraft-<version>-<system>-<processor>.<type>`.

| System | Installer | Portable / other |
|---|---|---|
| Windows 10/11, 64-bit (most PCs, Intel or AMD) | `…-windows-x64.msi` | `…-windows-x64-portable.zip` |
| Windows on ARM (Snapdragon, Surface Pro X…) | `…-windows-arm64.msi` | `…-windows-arm64-portable.zip` |
| Windows, 32-bit (older PCs) | `…-windows-x86.msi` | `…-windows-x86-portable.zip` |
| Mac (Apple Silicon or Intel) | `…-macos-universal.dmg` | `…-macos-arm64.dmg`, `…-macos-x86_64.dmg` |
| Any Linux | `…-linux-x86_64.AppImage` / `…-linux-aarch64.AppImage` | `.tar.gz` |
| Debian, Ubuntu, Mint, Pop!_OS, elementary… | `…-linux-<arch>.deb` | |
| Fedora, openSUSE, RHEL, Rocky, Alma… | `…-linux-<arch>.rpm` | |
| Arch, Manjaro, EndeavourOS… | `…-linux-<arch>.pkg.tar.zst` | |

Linux `<arch>` is `x86_64` (most PCs), `aarch64` (ARM, e.g. Raspberry Pi 5) or `riscv64`.
To check a download against `SHA256SUMS.txt`, run `sha256sum -c SHA256SUMS.txt --ignore-missing`.

## Windows

**Installer (.msi):** double-click it and follow the steps. PlayCraft is added to the
Start menu and the desktop. To remove it, use *Settings › Apps*.

**Portable (.zip):**

1. Unzip it anywhere: Desktop, Documents or a USB stick.
2. Run `PlayCraft\playcraft.exe`.

The playback engine (FFmpeg) and the DVD and Blu-ray libraries are built into `playcraft.exe`.
The first start takes a few seconds longer while PlayCraft unpacks them.


If Windows SmartScreen warns about an unknown app, choose **More info › Run anyway**.
PlayCraft isn't code-signed yet.

## Linux

**AppImage (any distribution):**

```
chmod +x playcraft-*-linux-x86_64.AppImage
./playcraft-*-linux-x86_64.AppImage
```

The AppImage carries its own playback engine and disc libraries. AppImage updaters
(AppImageUpdate, Gear Lever) can update it using the `.zsync` file.

**Debian / Ubuntu and relatives:**

```
sudo apt install ./playcraft-*-linux-x86_64.deb
```

**Fedora / openSUSE and relatives:**

```
sudo dnf install ./playcraft-*-linux-x86_64.rpm        # Fedora, RHEL
sudo zypper install ./playcraft-*-linux-x86_64.rpm     # openSUSE
```

On Fedora, the playback engine comes from RPM Fusion:
`sudo dnf install mpv-libs libbluray libdvdnav libaacs`.

**Arch / Manjaro:**

```
sudo pacman -U playcraft-*-linux-x86_64.pkg.tar.zst
```

**Other distributions:** unpack the `.tar.gz`, install your distribution's mpv library,
libbluray and libdvdnav, then run `./playcraft`.

## macOS

1. Open the `.dmg` and drag **PlayCraft** onto **Applications**.
2. The first time, right-click PlayCraft and choose **Open**. The app isn't notarised by
   Apple, so macOS asks once.

The universal version runs natively on both Apple Silicon and Intel Macs.

## Commercial Blu-ray discs

The decryption libraries (libaacs, libbdplus) are included, but no keys are. Most
shop-bought Blu-rays need a key file (`KEYDB.cfg`) that you supply. Add it under
*Settings › Discs & Blu-ray › Import key file*. Check the law where you live.
