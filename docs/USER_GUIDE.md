# PlayCraft user guide

## The window

- **Sidebar.** *Home* has quick actions and *Continue watching*. *Media Library* has
  Movies, TV Shows, Videos, Photos, Music and Playlists; the **+** adds a folder.
  *Source* has My Computer and any disc in a drive.
- **My Computer** shows your folders and drives. Click a folder to open it, a file to play
  it, or a disc folder (VIDEO_TS / BDMV) or ISO image to play it as a disc.
- **Control bar** (bottom). When nothing is playing, it shows play, volume and more
  options. During playback it shows the seek bar, transport keys, audio, subtitles,
  chapters, volume, full screen and the **remote** button at the far right.

## Discs

Insert a DVD, audio CD or Blu-ray and it appears in the sidebar with its name. Its page
offers **Play** (or **Resume / Restart** if you stopped part-way),
**Play movie (skip menus)**, **Browse files** and **Eject**.

### Disc menus

DVDs and Blu-rays open at their own menu:

- Point at a button to highlight it, then click to choose it.
- Or use the **arrow keys** and **Enter**.
- The **house button** in the control bar (or **Backspace**) brings the menu back during
  the film.
- Blu-rays whose menus need Java (BD-J) can't show them; they start the main film instead.

To skip menus every time, turn off *Settings › Playback › Start DVDs and Blu-rays at their
own menus*.

## The remote

Click the remote button at the right end of the control bar.

| Key | What it does |
|---|---|
| TOP MENU | The disc's main (title) menu |
| POP UP / MENU | A Blu-ray's pop-up menu (a DVD's root menu) |
| RETURN | Back one step in the disc menu |
| OPTIONS | Speed, A–B repeat, snapshot, aspect ratio, eject |
| Arrows + centre key | In a disc menu, move and select. Otherwise ◀ ▶ skip back/forward and ▲ ▼ change the volume |
| HOME | PlayCraft's home page |
| ◀◀ ▶ ▶▶ / ⏮ ❚❚ ■ ⏭ | Rewind, play, fast-forward / previous chapter, pause, stop, next chapter |
| SUBTITLE / AUDIO | Next subtitle or audio track |
| VOL + / − and mute | Volume |
| **C** / **P** | The volume keys control the **C**omputer's own volume (lit) or only **P**layCraft |

Remote controls:

- **− / +** make the remote smaller or bigger.
- **Mini** switches to a compact remote.
- **Out** (or dragging the strip at the top) turns it into its own window that you can
  move anywhere, even outside PlayCraft. Drag its edge to resize it.
- **Dock** puts it back above the bar.

## Keyboard shortcuts

| Key | Action |
|---|---|
| Space | Play / pause |
| ← / → | Back 10 s / forward 30 s |
| ↑ / ↓ | Volume |
| Page Up / Page Down | Previous / next chapter |
| F, F11 or double-click | Full screen (Esc to leave) |
| M | Mute |
| S | Snapshot of the picture |
| I | Media information |
| Home | Back to the beginning |
| Backspace | Disc menu |
| Ctrl+O | Open files |

## Settings

- **Playback:** resume where you stopped, autoplay inserted discs, disc menus, use the
  graphics card for decoding.
- **Picture:** brightness, contrast, colour, gamma, sharpen, noise reduction, deinterlace.
- **Sound:** speaker layout, Dolby/DTS passthrough to an AV receiver, night mode, clearer
  dialogue.
- **Languages & subtitles:** preferred audio and subtitle languages, subtitle size.
- **Discs & Blu-ray:** libaacs version and download, key-file import, and a licensed-player
  hand-off for protected discs.
- **Codec engine:** engine status, install or update, and a test picture.
- **About:** version, source code link and the **activity log**.

## Activity log

PlayCraft writes what it does to `playcraft.log`:

- Windows: `%LOCALAPPDATA%\PlayCraft\`
- macOS: `~/Library/Application Support/PlayCraft/`
- Linux: `~/.local/share/PlayCraft/`

Open it from *Settings › About › Open activity log*. Attach it when reporting a problem.

## Troubleshooting

- **"Codec engine not installed"** (bottom-left of the sidebar): on Windows, keep the
  `engine` folder next to `playcraft.exe`; on Linux and Mac, install your system's mpv
  library (see [INSTALL.md](INSTALL.md)).
- **A Blu-ray won't play:** it is probably copy-protected. See *Commercial Blu-ray discs*
  in [INSTALL.md](INSTALL.md).
- **The menu doesn't appear:** the disc may use Java menus; PlayCraft plays the main film
  instead. The activity log says why.
