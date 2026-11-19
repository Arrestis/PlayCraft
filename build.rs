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
    for s in [16u32, 24, 32, 48, 64, 128, 256] {
        let Some(img) = icon_rgba(&bytes, s) else { return };
        match image::codecs::ico::IcoFrame::as_png(img.as_raw(), s, s, image::ExtendedColorType::Rgba8) {
            Ok(f) => frames.push(f),
            Err(e) => {
                println!("cargo:warning=icon frame failed: {e}");
                return;
            }
        }
    }
    let ico = out.join("app.ico");
    let written = std::fs::File::create(&ico)
        .map_err(|e| e.to_string())
        .and_then(|f| image::codecs::ico::IcoEncoder::new(f).encode_images(&frames).map_err(|e| e.to_string()));
    if let Err(e) = written {
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

fn main() {
    embed_icon();
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=PLAYCRAFT_SKIP_ENGINE");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") || std::env::var_os("PLAYCRAFT_SKIP_ENGINE").is_some() {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    // OUT_DIR = <target>/<profile>/build/<pkg>-<hash>/out
    let Some(exe_dir) = out.ancestors().nth(3).map(Path::to_path_buf) else { return };
    let engine = exe_dir.join("engine");
    let _ = std::fs::create_dir_all(&engine);
    let work = out.join("downloads");
    let _ = std::fs::create_dir_all(&work);
    let arch = if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("aarch64") { "aarch64" } else { "x86_64" };

    if !engine.join("libmpv-2.dll").exists() {
        if let Err(e) = fetch_mpv(&engine, &work, arch) {
            println!("cargo:warning=Could not bundle the playback engine ({e}); the program will offer to install it.");
        }
    }
    // Disc navigation (menus) and decryption libraries, with every dependency they need.
    let marker = engine.join(".disc-libs-v2");
    if arch == "x86_64" && !marker.exists() {
        match fetch_msys(&["libaacs", "libbdplus", "libbluray", "libdvdnav", "libdvdread", "libdvdcss", "gcc-libs", "libstdc++"], &engine, &work) {
            Ok(()) => {
                let _ = std::fs::write(&marker, "ok");
            }
            Err(e) => println!("cargo:warning=Could not bundle the disc libraries ({e})."),
        }
    }
}

/// Download MSYS2 (mingw64) packages and everything they depend on, and copy their DLLs
/// into `engine`.
fn fetch_msys(roots: &[&str], engine: &Path, work: &Path) -> Result<(), String> {
    const PREFIX: &str = "mingw-w64-x86_64-";
    let mirrors = [
        "https://repo.msys2.org/mingw/mingw64/",
        "https://mirror.msys2.org/mingw/mingw64/",
        "https://mirrors.kernel.org/msys2/mingw/mingw64/",
        "https://ftp.osuosl.org/pub/msys2/mingw/mingw64/",
    ];
    let mut found = None;
    let mut last_err = String::new();
    for m in mirrors {
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
        let pkg = if pkg == "cc-libs" { "gcc-libs".to_string() } else { pkg };
        // The server writes "+" as "%2B" in file names (libstdc++).
        let prefix = format!("{PREFIX}{}-", pkg.replace('+', "%2B"));
        let name = links(&index, &prefix, ".pkg.tar.zst")
            .into_iter()
            .filter(|n| n[prefix.len()..].starts_with(|c: char| c.is_ascii_digit()))
            .last();
        let Some(name) = name else {
            if roots.contains(&pkg.as_str()) && pkg != "libdvdcss" {
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
    let bin = unpack.join("mingw64").join("bin");
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
    run(Command::new("curl.exe").args(["-sSfL", "-A", "playcraft-build", url]))
}

fn curl_file(url: &str, dest: &Path) -> Result<(), String> {
    run(Command::new("curl.exe").args(["-sSfL", "-A", "playcraft-build", "-o"]).arg(dest).arg(url)).map(|_| ())
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

