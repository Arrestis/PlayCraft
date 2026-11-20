//! Build step: bundle the playback engine next to the program on Windows, so the person
//! never has to download anything after building.
//!
//! It fetches (once, then reuses):
//!   * libmpv-2.dll  – mpv's engine with FFmpeg (every common codec), libdvdnav/libdvdread,
//!                     libbluray and libcdio built in (shinchiro/mpv-winbuild-cmake, GPL builds)
//!   * libaacs / libbdplus and their helpers (libgcrypt, libgpg-error…) from MSYS2 – the
//!     open-source Blu-ray decryption libraries libbluray loads at runtime.
//!
//! Everything lands in `<target>/<profile>/engine/`, which the program searches first.
//! Set PLAYCRAFT_SKIP_ENGINE=1 to skip. Failures only print a warning; the build still succeeds
//! and the program offers its own installer instead.

use std::path::{Path, PathBuf};
use std::process::Command;

include!("src/icon_mask.rs");

/// Give the Windows program file the person's icon (shown in Explorer and on the taskbar).
fn embed_icon() {
    println!("cargo:rerun-if-changed=assets/icon.jpg");
    println!("cargo:rerun-if-changed=src/icon_mask.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let Ok(bytes) = std::fs::read("assets/icon.jpg") else {
        println!("cargo:warning=assets/icon.jpg not found; the program keeps the default icon.");
        return;
    };
    let mut frames = Vec::new();
    for s in [16u32, 20, 24, 32, 40, 48, 64, 96, 128, 256] {
        let Some(img) = icon_rgba(&bytes, s) else { return };
        frames.push((s, img.into_raw()));
    }
    let ico = out.join("app.ico");
    if let Err(e) = std::fs::write(&ico, classic_ico(&frames)) {
        println!("cargo:warning=could not write the icon: {e}");
        return;
    }
    let rc = out.join("app.rc");
    let ico_str = ico.display().to_string().replace('\\', "\\\\");
    if std::fs::write(&rc, format!("1 ICON \"{ico_str}\"\n")).is_err() {
        return;
    }
    if let Err(e) = embed_resource::compile(&rc, embed_resource::NONE).manifest_optional() {
        println!("cargo:warning=could not embed the icon into the program file: {e}");
    }
}

/// Write a classic Windows .ico: bitmap images for every size Windows shows on shortcuts and
/// menus (some parts of Windows, such as installer shortcuts, show PNG-only icons blank) and
/// a PNG image only for the large 256-pixel size.
fn classic_ico(frames: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut images: Vec<Vec<u8>> = Vec::new();
    for (s, rgba) in frames {
        let s = *s as usize;
        if s >= 256 {
            let mut png = Vec::new();
            let enc = image::codecs::png::PngEncoder::new(&mut png);
            use image::ImageEncoder;
            if enc.write_image(rgba, s as u32, s as u32, image::ExtendedColorType::Rgba8).is_ok() {
                images.push(png);
            }
            continue;
        }
        let mask_row = (s.div_ceil(32)) * 4;
        let mut bmp = Vec::with_capacity(40 + s * s * 4 + mask_row * s);
        // BITMAPINFOHEADER (height doubled: colour image plus the transparency mask)
        bmp.extend_from_slice(&40u32.to_le_bytes());
        bmp.extend_from_slice(&(s as i32).to_le_bytes());
        bmp.extend_from_slice(&((s * 2) as i32).to_le_bytes());
        bmp.extend_from_slice(&1u16.to_le_bytes());
        bmp.extend_from_slice(&32u16.to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes());
        bmp.extend_from_slice(&((s * s * 4) as u32).to_le_bytes());
        bmp.extend_from_slice(&[0u8; 16]);
        // Pixels, bottom row first, as blue-green-red-alpha.
        for y in (0..s).rev() {
            for x in 0..s {
                let i = (y * s + x) * 4;
                bmp.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
            }
        }
        // AND mask: 1 = transparent (used by very old renderers; alpha wins elsewhere).
        for y in (0..s).rev() {
            let mut row = vec![0u8; mask_row];
            for x in 0..s {
                if rgba[(y * s + x) * 4 + 3] < 128 {
                    row[x / 8] |= 0x80 >> (x % 8);
                }
            }
            bmp.extend_from_slice(&row);
        }
        images.push(bmp);
    }
    let mut ico = Vec::new();
    ico.extend_from_slice(&[0, 0, 1, 0]);
    ico.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len();
    for ((s, _), img) in frames.iter().zip(&images) {
        let dim = if *s >= 256 { 0 } else { *s as u8 };
        ico.extend_from_slice(&[dim, dim, 0, 0]);
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&(img.len() as u32).to_le_bytes());
        ico.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += img.len();
    }
    for img in &images {
        ico.extend_from_slice(img);
    }
    ico
}

fn main() {
    embed_icon();
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=PLAYCRAFT_SKIP_ENGINE");
    println!("cargo:rerun-if-env-changed=PLAYCRAFT_EMBED_ENGINE");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    // The packed engine built into playcraft.exe (empty unless a Windows release build).
    let _ = std::fs::write(out.join("engine.pack"), b"");
    let _ = std::fs::write(out.join("engine.id"), "none");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") || std::env::var_os("PLAYCRAFT_SKIP_ENGINE").is_some() {
        return;
    }
    // OUT_DIR = <target>/<profile>/build/<pkg>-<hash>/out (newer Cargo adds another level:
    // <target>/<profile>/build/<pkg>/<hash>/out). The program sits next to that "build" folder.
    let Some(exe_dir) = out
        .ancestors()
        .find(|p| p.file_name().is_some_and(|n| n == "build"))
        .and_then(Path::parent)
        .map(Path::to_path_buf)
    else {
        return;
    };
    // A small icon file next to the program, for the Windows installer's shortcuts.
    let _ = std::fs::copy(out.join("app.ico"), exe_dir.join("playcraft.ico"));
    let engine = exe_dir.join("engine");
    let _ = std::fs::create_dir_all(&engine);
    let work = out.join("downloads");
    let _ = std::fs::create_dir_all(&work);
    // 64-bit (x64/AMD64), 32-bit (x86) and ARM builds each get matching libraries.
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "aarch64",
        Ok("x86") => "i686",
        _ => "x86_64",
    };

    println!("cargo:warning=Bundling the {arch} playback engine into {}", engine.display());
    if !engine.join("libmpv-2.dll").exists() {
        match fetch_mpv(&engine, &work, arch) {
            Ok(()) => println!("cargo:warning=Playback engine bundled"),
            Err(e) => println!("cargo:warning=Could not bundle the playback engine ({e}); the program will offer to install it."),
        }
    }
    // Disc navigation (menus) and decryption libraries, with every dependency they need.
    // v3: also the Vulkan loader (vulkan-1.dll), which libmpv needs and many PCs lack.
    let marker = engine.join(".disc-libs-v3");
    // MSYS2 environment and package prefix for each processor.
    let msys = match arch {
        "x86_64" => Some(("mingw64", "x86_64")),
        "i686" => Some(("mingw32", "i686")),
        "aarch64" => Some(("clangarm64", "clang-aarch64")),
        _ => None,
    };
    if let (Some((env, cpu)), false) = (msys, marker.exists()) {
        let mut roots = vec!["libaacs", "libbdplus", "libbluray", "libdvdnav", "libdvdread", "libdvdcss", "vulkan-loader"];
        if cpu.starts_with("clang") {
            roots.extend(["libc++", "libunwind", "libwinpthread"]);
        } else {
            roots.extend(["gcc-libs", "libstdc++"]);
        }
        match fetch_msys(&roots, &engine, &work, env, cpu) {
            Ok(()) => {
                println!("cargo:warning=Disc libraries bundled");
                let _ = std::fs::write(&marker, "ok");
            }
            Err(e) => println!("cargo:warning=Could not bundle the disc libraries ({e})."),
        }
    }

    // Keep only the libraries something actually uses.
    let removed = prune_engine(&engine);
    if removed > 0 {
        println!("cargo:warning=Removed {removed} unused engine libraries");
    }

    // Release builds carry the whole engine inside playcraft.exe, so the program is one file.
    let release = std::env::var("PROFILE").as_deref() == Ok("release");
    let wanted = std::env::var("PLAYCRAFT_EMBED_ENGINE").map(|v| v != "0").unwrap_or(release);
    // A marker next to the program tells the packaging script the engine is inside it.
    let marker = exe_dir.join("engine-embedded.txt");
    let _ = std::fs::remove_file(&marker);
    if wanted && engine.join("libmpv-2.dll").exists() {
        match pack_engine(&engine, &out) {
            Ok((files, bytes)) => {
                println!("cargo:warning=Built {files} engine libraries into the program ({} MB packed)", bytes / 1_048_576);
                let _ = std::fs::write(&marker, format!("{files} engine libraries are built into playcraft.exe\n"));
            }
            Err(e) => println!("cargo:warning=Could not build the engine into the program ({e}); it stays in the engine folder."),
        }
    }
}

/// The DLL names a Windows library imports (from its PE import table).
fn pe_imports(path: &Path) -> Vec<String> {
    let Ok(b) = std::fs::read(path) else { return Vec::new() };
    let rd16 = |o: usize| b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]]) as usize).unwrap_or(0);
    let rd32 = |o: usize| b.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize).unwrap_or(0);
    let pe = rd32(0x3c);
    if b.get(pe..pe + 4) != Some(b"PE\0\0") {
        return Vec::new();
    }
    let nsec = rd16(pe + 6);
    let opt = pe + 24;
    let magic = rd16(opt);
    let dd = if magic == 0x20b { opt + 112 } else { opt + 96 };
    let imp_rva = rd32(dd + 8);
    let sec = opt + rd16(pe + 20);
    let to_off = |rva: usize| -> Option<usize> {
        (0..nsec).find_map(|i| {
            let s = sec + 40 * i;
            let (va, vsz, raw, rsz) = (rd32(s + 12), rd32(s + 8), rd32(s + 20), rd32(s + 16));
            (rva >= va && rva < va + vsz.max(rsz)).then(|| rva - va + raw)
        })
    };
    let mut names = Vec::new();
    let Some(mut o) = to_off(imp_rva) else { return names };
    for _ in 0..512 {
        let name_rva = rd32(o + 12);
        if name_rva == 0 {
            break;
        }
        if let Some(n) = to_off(name_rva) {
            let end = b[n..].iter().position(|&c| c == 0).map(|e| n + e).unwrap_or(n);
            names.push(String::from_utf8_lossy(&b[n..end]).to_lowercase());
        }
        o += 20;
    }
    names
}

/// Remove engine DLLs that neither PlayCraft nor any needed library uses. Returns how many.
fn prune_engine(engine: &Path) -> usize {
    let Ok(rd) = std::fs::read_dir(engine) else { return 0 };
    let dlls: Vec<String> = rd
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.to_lowercase().ends_with(".dll"))
        .collect();
    // Loaded directly by PlayCraft (or by libbluray / libdvdread at run time).
    let mut keep: Vec<String> = dlls
        .iter()
        .filter(|n| {
            let l = n.to_lowercase();
            l == "libmpv-2.dll"
                || l == "libaacs.dll"
                || l == "libbdplus.dll"
                || l.starts_with("libbluray-")
                || l.starts_with("libdvdnav-")
                || l.starts_with("libdvdcss-")
        })
        .map(|n| n.to_lowercase())
        .collect();
    let mut i = 0;
    while i < keep.len() {
        for dep in pe_imports(&engine.join(&keep[i])) {
            if dlls.iter().any(|d| d.to_lowercase() == dep) && !keep.contains(&dep) {
                keep.push(dep);
            }
        }
        i += 1;
    }
    let mut removed = 0;
    for d in &dlls {
        if !keep.contains(&d.to_lowercase()) && std::fs::remove_file(engine.join(d)).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Pack every engine file into OUT_DIR/engine.pack (simple format, each file deflated):
/// "PCENG1", u32 count, then per file: u16 name length, name, u64 size, u64 packed size, data.
fn pack_engine(engine: &Path, out: &Path) -> Result<(usize, usize), String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(engine)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
        .collect();
    files.sort();
    let mut pack = Vec::with_capacity(64 << 20);
    pack.extend_from_slice(b"PCENG1");
    pack.extend_from_slice(&(files.len() as u32).to_le_bytes());
    let mut id: u64 = 0xcbf2_9ce4_8422_2325;
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let data = std::fs::read(f).map_err(|e| e.to_string())?;
        let packed = miniz_oxide::deflate::compress_to_vec(&data, 6);
        pack.extend_from_slice(&(name.len() as u16).to_le_bytes());
        pack.extend_from_slice(name.as_bytes());
        pack.extend_from_slice(&(data.len() as u64).to_le_bytes());
        pack.extend_from_slice(&(packed.len() as u64).to_le_bytes());
        pack.extend_from_slice(&packed);
        for b in name.bytes().chain((data.len() as u64).to_le_bytes()).chain(packed.iter().step_by(4096).copied()) {
            id = (id ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
    }
    std::fs::write(out.join("engine.pack"), &pack).map_err(|e| e.to_string())?;
    std::fs::write(out.join("engine.id"), format!("{id:016x}")).map_err(|e| e.to_string())?;
    Ok((files.len(), pack.len()))
}

/// Download MSYS2 (mingw64) packages and everything they depend on, and copy their DLLs
/// into `engine`.
/// `env` is the MSYS2 environment ("mingw64" for 64-bit, "mingw32" for 32-bit) and `cpu`
/// the package architecture ("x86_64" or "i686").
fn fetch_msys(roots: &[&str], engine: &Path, work: &Path, env: &str, cpu: &str) -> Result<(), String> {
    let prefix_owned = format!("mingw-w64-{cpu}-");
    #[allow(non_snake_case)]
    let PREFIX: &str = &prefix_owned;
    let mirrors = [
        format!("https://repo.msys2.org/mingw/{env}/"),
        format!("https://mirror.msys2.org/mingw/{env}/"),
        format!("https://mirrors.kernel.org/msys2/mingw/{env}/"),
        format!("https://ftp.osuosl.org/pub/msys2/mingw/{env}/"),
    ];
    let mut found = None;
    let mut last_err = String::new();
    for m in &mirrors {
        match curl_text(m) {
            Ok(t) if t.contains(PREFIX) => {
                found = Some((m, t));
                break;
            }
            Ok(_) => last_err = format!("{m}: unexpected page"),
            Err(e) => last_err = e,
        }
    }
    let Some((mirror, index)) = found else { return Err(last_err) };
    let unpack = work.join("msys");
    let _ = std::fs::remove_dir_all(&unpack);
    let _ = std::fs::create_dir_all(&unpack);
    let mut queue: Vec<String> = roots.iter().map(|s| s.to_string()).collect();
    let mut seen = std::collections::HashSet::new();
    while let Some(pkg) = queue.pop() {
        if !seen.insert(pkg.clone()) || seen.len() > 80 {
            continue;
        }
        // Skip toolchain-only packages that never ship runtime DLLs we need.
        if matches!(pkg.as_str(), "python" | "perl" | "ca-certificates") {
            continue;
        }
        // "cc-libs" is the C/C++ runtime, published as gcc-libs.
        let pkg = if pkg == "cc-libs" {
            if cpu.starts_with("clang") { "libc++".to_string() } else { "gcc-libs".to_string() }
        } else {
            pkg
        };
        // The server writes "+" as "%2B" in file names (libstdc++).
        let prefix = format!("{PREFIX}{}-", pkg.replace('+', "%2B"));
        let name = links(&index, &prefix, ".pkg.tar.zst")
            .into_iter()
            .filter(|n| n[prefix.len()..].starts_with(|c: char| c.is_ascii_digit()))
            .last();
        let Some(name) = name else {
            if ["libaacs", "libbluray", "libdvdnav"].contains(&pkg.as_str()) {
                return Err(format!("{pkg} not found"));
            }
            continue;
        };
        let file = work.join(&name);
        curl_file(&format!("{mirror}{name}"), &file)?;
        let pkginfo = unpack.join(".PKGINFO");
        let _ = std::fs::remove_file(&pkginfo);
        run(Command::new("tar.exe").arg("-xf").arg(&file).arg("-C").arg(&unpack))?;
        let _ = std::fs::remove_file(&file);
        if let Ok(info) = std::fs::read_to_string(&pkginfo) {
            for line in info.lines() {
                if let Some(dep) = line.strip_prefix("depend = ") {
                    let dep = dep.split(|c| c == '<' || c == '>' || c == '=').next().unwrap_or("").trim();
                    if let Some(short) = dep.strip_prefix(PREFIX) {
                        queue.push(short.to_string());
                    }
                }
            }
        }
    }
    let bin = unpack.join(env).join("bin");
    let rd = std::fs::read_dir(&bin).map_err(|e| e.to_string())?;
    for e in rd.flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if n.to_lowercase().ends_with(".dll") {
            std::fs::copy(e.path(), engine.join(&n)).map_err(|e| e.to_string())?;
        }
    }
    // libbluray looks for these exact names.
    for (from, to) in [("libaacs-0.dll", "libaacs.dll"), ("libbdplus-0.dll", "libbdplus.dll")] {
        if engine.join(from).exists() {
            std::fs::copy(engine.join(from), engine.join(to)).map_err(|e| e.to_string())?;
        }
    }
    let _ = std::fs::remove_dir_all(&unpack);
    for must in ["libaacs", "libbluray-", "libdvdnav-"] {
        let ok = std::fs::read_dir(engine).map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().starts_with(must))).unwrap_or(false);
        if !ok {
            return Err(format!("{must} missing after unpacking"));
        }
    }
    Ok(())
}

fn run(cmd: &mut Command) -> Result<String, String> {
    let o = cmd.output().map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

fn curl_text(url: &str) -> Result<String, String> {
    run(Command::new("curl.exe").args(["-sSfL", "--retry", "4", "--retry-all-errors", "-A", "playcraft-build", url]))
}

fn curl_file(url: &str, dest: &Path) -> Result<(), String> {
    run(Command::new("curl.exe").args(["-sSfL", "--retry", "4", "--retry-all-errors", "-A", "playcraft-build", "-o"]).arg(dest).arg(url)).map(|_| ())
}

/// All href="…" values in a page that start with `prefix` and end with `suffix`.
fn links(html: &str, prefix: &str, suffix: &str) -> Vec<String> {
    html.split("href=\"")
        .skip(1)
        .filter_map(|s| s.split('"').next())
        .filter(|h| {
            let name = h.rsplit('/').next().unwrap_or(h);
            name.starts_with(prefix) && name.ends_with(suffix)
        })
        .map(str::to_string)
        .collect()
}

fn fetch_mpv(engine: &Path, work: &Path, arch: &str) -> Result<(), String> {
    const REPO: &str = "https://github.com/shinchiro/mpv-winbuild-cmake";
    // The "latest" page redirects to /releases/tag/<tag>.
    let headers = run(Command::new("curl.exe").args(["-sSIL", "-A", "playcraft-build", &format!("{REPO}/releases/latest")]))?;
    let tag = headers
        .lines()
        .filter_map(|l| l.strip_prefix("location: ").or_else(|| l.strip_prefix("Location: ")))
        .filter_map(|l| l.trim().rsplit("/tag/").next().filter(|_| l.contains("/tag/")))
        .last()
        .ok_or("no release found")?
        .to_string();
    let page = curl_text(&format!("{REPO}/releases/expanded_assets/{tag}"))?;
    let href = links(&page, &format!("mpv-dev-{arch}-2"), ".7z").into_iter().next().ok_or("no engine file in the release")?;
    let url = if href.starts_with("http") { href } else { format!("https://github.com{href}") };
    let archive = work.join("mpv-dev.7z");
    curl_file(&url, &archive)?;
    let ok = run(Command::new("tar.exe").arg("-xf").arg(&archive).arg("-C").arg(engine).arg("libmpv-2.dll")).is_ok();
    if !ok || !engine.join("libmpv-2.dll").exists() {
        run(Command::new("7z.exe").arg("e").arg(&archive).arg(format!("-o{}", engine.display())).arg("libmpv-2.dll").arg("-y"))?;
    }
    let _ = std::fs::remove_file(&archive);
    if engine.join("libmpv-2.dll").exists() {
        Ok(())
    } else {
        Err("unpacking failed".into())
    }
}

