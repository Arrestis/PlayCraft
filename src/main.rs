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

fn main() -> eframe::Result<()> {
    let mut screenshot: Option<PathBuf> = None;
    let mut files = Vec::new();
    let mut demo = false;
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        match a.to_str() {
            Some("--screenshot") => screenshot = args.next().map(PathBuf::from),
            Some("--test-picture") => demo = true,
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
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if screenshot.is_some() {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            b.with_activate_ignoring_other_apps(false);
        }));
    }
    eframe::run_native(
        "PlayCraft",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, screenshot, files, demo)))),
    )
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
