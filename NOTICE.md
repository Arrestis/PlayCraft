# PlayCraft: licence notice

PlayCraft
Copyright (C) 2026 PlayCraft contributors

This program is free software: you can redistribute it and/or modify it under the terms of
the GNU General Public License as published by the Free Software Foundation, either
version 3 of the License, or (at your option) any later version.

This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
See the GNU General Public License in `LICENSE` for more details.

SPDX-License-Identifier: GPL-3.0-or-later

Because PlayCraft is licensed under the GPL, anyone who shares PlayCraft, or a program
built from it, must also share the source code under the same licence. This keeps
PlayCraft open source.

## Bundled third-party components

Windows builds bundle these libraries in the `engine` folder. Each keeps its own licence:

| Component | Licence |
|---|---|
| libmpv (mpv) | GPL-2.0-or-later build |
| FFmpeg | GPL |
| libdvdnav, libdvdread | GPL-2.0-or-later |
| libdvdcss | GPL-2.0-or-later |
| libbluray | LGPL-2.1-or-later |
| libaacs, libbdplus | LGPL-2.1-or-later |
| libgcrypt, libgpg-error | LGPL-2.1-or-later |
| libxml2, freetype, fontconfig and other helper libraries | their own free-software licences |
| gcc runtime, winpthreads | GPL with runtime exception / MIT-style |

PlayCraft ships no decryption keys or certificates.
