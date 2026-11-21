#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
//! PlayCraft: a free, open-source DVD, CD, Blu-ray and media player.
//!
//! Usage:
//!   playcraft [files or disc folders…]
//!   playcraft --screenshot <file.png>   draw ~120 frames, save the window and exit
//!   playcraft --test-picture            play the engine's built-in test picture

mod aacs;
mod app;
mod discs;
mod embed;
mod icon_mask;
mod icons;
mod library;
mod log;
mod mpv;
mod nav;
mod sysvol;
mod pages;
mod ui;

use eframe::egui;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

fn main() -> eframe::Result<()> {
    let mut screenshot: Option<PathBuf> = None;
    let mut files = Vec::new();
    let mut demo = false;
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        match a.to_str() {
            Some("--screenshot") => screenshot = args.next().map(PathBuf::from),
            Some("--test-picture") => demo = true,
            // playcraft --check-engine: unpack and load the playback engine and the disc
            // libraries, report each one, and exit 0 if playback would work (used to test
            // builds on clean PCs).
            Some("--check-engine") => {
                library::migrate_old_data();
                log::init();
                embed::prepare();
                app::setup_library_paths();
                let mut ok = true;
                // Keep the engine loaded until the end, so its helper libraries can be inspected.
                let engine = mpv::Engine::locate();
                match &engine {
                    Ok(e) => println!("OK      playback engine: {} (client API {})", e.path, e.api_version()),
                    Err(e) => {
                        ok = false;
                        println!("FAILED  playback engine: {e}");
                    }
                }
                for (name, kind) in [("Blu-ray menus (libbluray)", nav::NavKind::Bluray), ("DVD menus (libdvdnav)", nav::NavKind::Dvd)] {
                    match nav::available(kind) {
                        Ok(()) => println!("OK      {name}"),
                        Err(e) => println!("MISSING {name}: {e}"),
                    }
                }
                match aacs::info() {
                    Some(i) => println!("OK      libaacs {} ({})", i.version, i.path),
                    None => println!("MISSING libaacs"),
                }
                #[cfg(windows)]
                unsafe {
                    use windows_sys::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW};
                    let name: Vec<u16> = "vulkan-1.dll".encode_utf16().chain(Some(0)).collect();
                    let h = GetModuleHandleW(name.as_ptr());
                    if h.is_null() {
                        println!("INFO    vulkan-1.dll not loaded");
                    } else {
                        let mut buf = [0u16; 1024];
                        let n = GetModuleFileNameW(h, buf.as_mut_ptr(), buf.len() as u32) as usize;
                        println!("INFO    vulkan-1.dll loaded from {}", String::from_utf16_lossy(&buf[..n]));
                    }
                }
                log::info(format!("Engine check: {}", if ok { "passed" } else { "FAILED" }));
                std::process::exit(if ok { 0 } else { 1 });
            }
            // playcraft --check-disc <drive or folder>: report what the engine can read from a
            // disc (its disc readers, the menu navigator, the first seconds of data) and exit.
            Some("--check-disc") => {
                let dev = args.next().map(|a| a.to_string_lossy().to_string()).unwrap_or_default();
                library::migrate_old_data();
                log::init();
                embed::prepare();
                app::setup_library_paths();
                std::process::exit(app::check_disc(&dev));
            }
            // playcraft --install-engine-to <folder>: download the playback engine there and
            // exit (a command-line way to install it on a PC where the button doesn't work).
            Some("--install-engine-to") => {
                let dir = args.next().map(PathBuf::from).unwrap_or_else(library::engine_dir);
                log::init();
                let r = library::fetch_engine_windows(&dir, &|m: String| println!("{m}"));
                match &r {
                    Ok(p) => println!("Installed: {p}"),
                    Err(e) => {
                        println!("Failed: {e}");
                        show_error(&format!("The playback engine could not be installed:\n\n{e}"));
                    }
                }
                std::process::exit(if r.is_ok() { 0 } else { 1 });
            }
            Some("--screenshot-frames") => {
                if let Some(n) = args.next().and_then(|n| n.to_str().and_then(|s| s.parse::<u32>().ok())) {
                    app::SCREENSHOT_FRAMES.store(n.max(10), std::sync::atomic::Ordering::Relaxed);
                }
            }
            _ => files.push(PathBuf::from(a)),
        }
    }

    #[cfg(windows)]
    unsafe {
        // Don't pop up "no disk in drive" dialogs while looking at empty drives.
        windows_sys::Win32::System::Diagnostics::Debug::SetErrorMode(
            windows_sys::Win32::System::Diagnostics::Debug::SEM_FAILCRITICALERRORS,
        );
    }

    library::migrate_old_data();
    log::init();
    embed::prepare();
    if let Some(p) = &screenshot {
        log::info(format!("Screenshot mode: saving the window to {}", p.display()));
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("PlayCraft")
        .with_inner_size([1280.0, 780.0])
        .with_min_inner_size([900.0, 560.0])
        .with_drag_and_drop(true)
        .with_icon(std::sync::Arc::new(app_icon()));
    if screenshot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(windows)]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    // Graphics: the modern renderer (Direct3D 12 / Vulkan / Metal) normally; OpenGL on PCs
    // where that isn't available (older graphics, virtual machines, remote desktops).
    let glow = use_opengl();
    log::info(format!("Graphics: {}", if glow { "OpenGL" } else { "Direct3D/Vulkan/Metal (wgpu)" }));
    install_crash_handler(glow);

    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions {
        viewport,
        renderer: if glow { eframe::Renderer::Glow } else { eframe::Renderer::Wgpu },
        ..Default::default()
    };
    #[cfg(target_os = "macos")]
    if screenshot.is_some() {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            b.with_activate_ignoring_other_apps(false);
        }));
    }
    let result = eframe::run_native(
        "PlayCraft",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, screenshot, files, demo)))),
    );
    if let Err(e) = &result {
        log::error(format!("PlayCraft could not open its window: {e}"));
        if !glow && !FIRST_FRAME.load(Ordering::Acquire) && restart_with_opengl() {
            return Ok(());
        }
        show_error(&format!(
            "PlayCraft could not open its window.\n\n{e}\n\nDetails are in the activity log:\n{}",
            log::path().display()
        ));
    }
    result
}

/// Set once the first frame has been drawn, so start-up failures can be told apart.
pub static FIRST_FRAME: AtomicBool = AtomicBool::new(false);

fn renderer_file() -> PathBuf {
    library::data_dir().join("renderer.txt")
}

/// OpenGL if asked for (PLAYCRAFT_RENDERER=glow) or if the modern renderer failed before.
fn use_opengl() -> bool {
    match std::env::var("PLAYCRAFT_RENDERER").as_deref() {
        Ok("glow") | Ok("opengl") => return true,
        Ok("wgpu") => return false,
        _ => {}
    }
    std::fs::read_to_string(renderer_file()).is_ok_and(|s| s.trim() == "glow")
}

/// Start a fresh copy of PlayCraft using OpenGL. Returns true if it started.
fn restart_with_opengl() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    log::warn("Restarting with OpenGL graphics");
    std::process::Command::new(exe)
        .args(std::env::args_os().skip(1))
        .env("PLAYCRAFT_RENDERER", "glow")
        .spawn()
        .is_ok()
}

/// Called after the first frame: remember that OpenGL works on this PC if we fell back to it.
pub fn note_first_frame() {
    if !FIRST_FRAME.swap(true, Ordering::AcqRel) && std::env::var("PLAYCRAFT_RENDERER").as_deref() == Ok("glow") {
        let _ = std::fs::write(renderer_file(), "glow");
        log::info("OpenGL graphics work on this PC; PlayCraft will use them from now on");
    }
}

/// Log crashes and explain them instead of closing silently. A crash before the window
/// first appears with the modern renderer retries with OpenGL.
fn install_crash_handler(glow: bool) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = info.payload().downcast_ref::<&str>().map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown error".into());
        let place = info.location().map(|l| format!(" at {}:{}", l.file(), l.line())).unwrap_or_default();
        log::error(format!("PlayCraft stopped unexpectedly: {msg}{place}"));
        if !glow && !FIRST_FRAME.load(Ordering::Acquire) && restart_with_opengl() {
            std::process::exit(0);
        }
        show_error(&format!(
            "PlayCraft stopped unexpectedly:\n\n{msg}\n\nDetails are in the activity log:\n{}",
            log::path().display()
        ));
        default(info);
    }));
}

/// A plain error box (Windows), or a message on the terminal elsewhere.
fn show_error(text: &str) {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
        let t: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let c: Vec<u16> = "PlayCraft".encode_utf16().chain(Some(0)).collect();
        MessageBoxW(std::ptr::null_mut(), t.as_ptr(), c.as_ptr(), MB_OK | MB_ICONERROR);
    }
    #[cfg(not(windows))]
    eprintln!("{text}");
}

/// The person's icon picture, built into the program.
pub const ICON_BYTES: &[u8] = include_bytes!("../assets/icon.jpg");

/// The window icon, made from the bundled picture with rounded corners.
pub fn app_icon() -> egui::IconData {
    match icon_mask::icon_rgba(ICON_BYTES, 256) {
        Some(img) => egui::IconData { width: img.width(), height: img.height(), rgba: img.into_raw() },
        None => egui::IconData::default(),
    }
}
