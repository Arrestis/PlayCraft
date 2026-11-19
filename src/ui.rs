//! Window frame: sidebar, page headers, video area, control bar, keyboard and messages.

use crate::app::*;
use crate::discs::{self, DiscKind};
use crate::icons::{self, Icon};
use crate::library::{self, fmt_time};
use crate::mpv::Track;
use crate::pages;
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, CornerRadius, FontId, Frame, Layout, Margin, Rect, RichText, Sense, Stroke, Ui,
    UiBuilder,
};
use std::time::{Duration, Instant};

pub fn draw(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    app.frame_no += 1;
    app.pump(&ctx);
    handle_keys(app, &ctx);
    if app.current.is_some() && !app.paused {
        ctx.request_repaint_after(Duration::from_millis(250));
    }
    if ctx.input(|i| i.pointer.delta() != egui::Vec2::ZERO || i.pointer.any_down()) {
        app.last_activity = Instant::now();
    }
    let show_chrome = !app.fullscreen;
    let show_controls = show_chrome || app.last_activity.elapsed() < Duration::from_millis(2500) || app.paused;
    if app.fullscreen {
        if !show_controls {
            ctx.set_cursor_icon(egui::CursorIcon::None);
        }
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    if show_controls {
        let h = if app.current.is_some() { 96.0 } else { 62.0 };
        egui::Panel::bottom("controls")
            .exact_size(h)
            .frame(Frame::new().fill(BAR).inner_margin(Margin::symmetric(14, 0)))
            .show(ui, |ui| control_bar(app, ui));
    }
    if show_chrome {
        egui::Panel::left("sidebar")
            .exact_size(200.0)
            .frame(Frame::NONE)
            .show(ui, |ui| {
                let r = ui.max_rect();
                gradient(ui.painter(), r, SIDEBAR, SIDEBAR_BOTTOM);
                ui.scope_builder(UiBuilder::new().max_rect(r.shrink2(vec2(8.0, 0.0))), |ui| sidebar(app, ui));
            });
    }
    let page = app.page;
    egui::CentralPanel::default().frame(Frame::NONE).show(ui, |ui| {
        let r = ui.max_rect();
        if page == Page::NowPlaying {
            ui.painter().rect_filled(r, 0.0, Color32::BLACK);
            now_playing(app, ui);
            return;
        }
        gradient(ui.painter(), r, BG, BG_BOTTOM);
        let inner = Rect::from_min_max(r.min + vec2(46.0, 8.0), r.max - vec2(40.0, 4.0));
        ui.scope_builder(UiBuilder::new().max_rect(inner), |ui| match page {
            Page::Home => pages::home(app, ui),
            Page::Computer => pages::computer_page(app, ui),
            Page::Disc(i) => pages::disc_page(app, ui, i),
            Page::Movies => pages::library_page(app, ui, page),
            Page::TvShows => pages::library_page(app, ui, page),
            Page::Videos => pages::library_page(app, ui, page),
            Page::Music => pages::library_page(app, ui, page),
            Page::Photos => pages::library_page(app, ui, page),
            Page::Playlist => pages::playlist_page(app, ui),
            Page::Settings => pages::settings_page(app, ui),
            Page::NowPlaying => {}
        });
    });
    url_dialog(app, &ctx);
    draw_toast(app, &ctx);
    app.screenshot_tick(&ctx);
}

/// Vertical colour fade used for the sidebar and page background.
pub fn gradient(p: &egui::Painter, r: Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.left_top(), top);
    mesh.colored_vertex(r.right_top(), top);
    mesh.colored_vertex(r.right_bottom(), bottom);
    mesh.colored_vertex(r.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(mesh));
}

/// The header every page shares: window tools on the right, back arrow and title on the left,
/// and the search box. Returns true if the back arrow was pressed.
pub fn page_header(app: &mut App, ui: &mut Ui, title: &str, back: bool) -> bool {
    let mut back_clicked = false;
    ui.horizontal(|ui| {
        ui.set_height(30.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if icons::button(ui, Icon::Fullscreen, 24.0, "Full screen (F)", false).clicked() {
                let ctx = ui.ctx().clone();
                app.set_fullscreen(&ctx, true);
            }
            if icons::button(ui, Icon::Gear, 24.0, "Settings", app.page == Page::Settings).clicked() {
                app.page = Page::Settings;
            }
            if icons::button(ui, Icon::List, 24.0, "Playlist", app.page == Page::Playlist).clicked() {
                app.page = Page::Playlist;
            }
            let open = icons::button(ui, Icon::Folder, 24.0, "Open files, disc folders, disc images or a network address", false);
            open_menu(app, &open);
        });
    });
    ui.horizontal(|ui| {
        ui.set_height(36.0);
        if back {
            let (r, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
            ui.painter().rect_filled(r, 3.0, if resp.hovered() { CARD_HOVER } else { CARD });
            let c = r.center();
            let s = Stroke::new(2.0, TEXT);
            ui.painter().line_segment([pos2(c.x + 3.0, c.y - 7.0), pos2(c.x - 4.0, c.y)], s);
            ui.painter().line_segment([pos2(c.x - 4.0, c.y), pos2(c.x + 3.0, c.y + 7.0)], s);
            if resp.on_hover_text("Back").clicked() {
                back_clicked = true;
            }
            ui.add_space(8.0);
        }
        ui.label(RichText::new(title).size(24.0).color(Color32::WHITE));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut app.search)
                    .hint_text("Search")
                    .desired_width(190.0)
                    .background_color(Color32::from_rgb(52, 52, 52)),
            );
            if resp.changed() && !app.search.is_empty() && !matches!(app.page, Page::Movies | Page::TvShows | Page::Videos | Page::Music | Page::Photos) {
                app.page = Page::Videos;
            }
        });
    });
    back_clicked
}

pub fn open_menu(app: &mut App, resp: &egui::Response) {
    egui::Popup::menu(resp).width(250.0).show(|ui| {
        if ui.button("Media files…    Ctrl+O").clicked() {
            ui.close();
            app.open_files();
        }
        if ui.button("DVD / Blu-ray folder…").clicked() {
            ui.close();
            app.open_disc_folder();
        }
        if ui.button("Disc image (ISO)…").clicked() {
            ui.close();
            app.open_iso();
        }
        if ui.button("Network address (stream link)…").clicked() {
            ui.close();
            app.url_input = Some(String::new());
        }
        ui.separator();
        let drives = app.drives.clone();
        for d in drives.iter().filter(|d| d.kind.playable()) {
            if ui.button(format!("Play {} ({})", d.name(), d.device)).clicked() {
                ui.close();
                app.play_list(vec![PlayItem::drive(d)], 0);
            }
        }
        if drives.iter().all(|d| !d.kind.playable()) {
            ui.label(RichText::new("No disc inserted").color(TEXT_DIM));
        }
    });
}

fn url_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut text) = app.url_input.take() else { return };
    let mut keep = true;
    let mut go = false;
    egui::Window::new("Open a network address")
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label("Paste a link to a video or audio stream (http, https, rtsp, smb…):");
            let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(420.0).hint_text("https://…"));
            r.request_focus();
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                go = true;
            }
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new("Play").color(Color32::WHITE)).fill(ACCENT)).clicked() {
                    go = true;
                }
                if ui.button("Cancel").clicked() {
                    keep = false;
                }
            });
        });
    if go {
        app.open_url(&text);
    } else if keep {
        app.url_input = Some(text);
    }
}

fn nav_item(ui: &mut Ui, app: &mut App, icon: Icon, label: &str, page: Page, extra: Option<(&str, Color32)>) -> egui::Response {
    let selected = app.page == page;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::click());
    let row = rect.shrink2(vec2(0.0, 4.0));
    if selected {
        ui.painter().rect_filled(row, 3.0, Color32::from_rgb(78, 78, 78));
    } else if resp.hovered() {
        ui.painter().rect_filled(row, 3.0, Color32::from_rgb(62, 62, 62));
    }
    let col = Color32::from_rgb(232, 232, 232);
    let icon_rect = Rect::from_center_size(pos2(rect.left() + 16.0, rect.center().y), vec2(18.0, 18.0));
    icons::paint(ui.painter(), icon, icon_rect, col);
    ui.painter().text(pos2(rect.left() + 36.0, rect.center().y), Align2::LEFT_CENTER, label, FontId::proportional(14.0), col);
    if let Some((t, c)) = extra {
        ui.painter().text(pos2(rect.right() - 8.0, rect.center().y), Align2::RIGHT_CENTER, t, FontId::proportional(11.0), c);
    }
    if resp.clicked() {
        app.page = page;
    }
    resp
}

fn section(ui: &mut Ui, t: &str) -> Option<egui::Response> {
    ui.add_space(6.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment([rect.left_center(), rect.right_center()], Stroke::new(1.0, Color32::from_rgb(70, 70, 70)));
    ui.add_space(6.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
    ui.painter().text(pos2(rect.left() + 4.0, rect.center().y), Align2::LEFT_CENTER, t, FontId::proportional(14.0), Color32::WHITE);
    ui.painter().text(pos2(rect.left() + 4.4, rect.center().y), Align2::LEFT_CENTER, t, FontId::proportional(14.0), Color32::WHITE);
    if t == "Media Library" {
        let plus = Rect::from_center_size(pos2(rect.right() - 10.0, rect.center().y), vec2(20.0, 20.0));
        let resp = ui.interact(plus, ui.scope_id().with("lib_plus"), Sense::click());
        let c = if resp.hovered() { ACCENT } else { Color32::from_rgb(220, 220, 220) };
        icons::paint(ui.painter(), Icon::Plus, plus, c);
        return Some(resp.on_hover_text("Add a folder to the library"));
    }
    None
}

fn sidebar(app: &mut App, ui: &mut Ui) {
    ui.add_space(2.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
    let mut text_x = rect.left() + 2.0;
    if let Some(t) = &app.logo_tex {
        let r = Rect::from_min_size(pos2(rect.left() + 2.0, rect.center().y - 15.0), vec2(30.0, 30.0));
        ui.painter().image(t.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        text_x = r.right() + 8.0;
    }
    ui.painter().text(pos2(text_x, rect.center().y), Align2::LEFT_CENTER, "PlayCraft", FontId::proportional(27.0), Color32::WHITE);
    nav_item(ui, app, Icon::Home, "Home", Page::Home, None);
    if app.current.is_some() {
        nav_item(ui, app, Icon::Play, "Now Playing", Page::NowPlaying, Some(("●", GOOD)));
    }
    if let Some(plus) = section(ui, "Media Library") {
        if plus.clicked() {
            if let Some(f) = rfd::FileDialog::new().set_title("Add a folder to your library").pick_folder() {
                app.settings.folders.push(f);
                app.settings.save();
                app.scan_rx = Some(library::scan(app.settings.folders.clone()));
            }
        }
    }
    nav_item(ui, app, Icon::Film, "Movies", Page::Movies, None);
    nav_item(ui, app, Icon::Film, "TV Shows", Page::TvShows, None);
    nav_item(ui, app, Icon::Camera, "Videos", Page::Videos, None);
    nav_item(ui, app, Icon::Photo, "Photos", Page::Photos, None);
    nav_item(ui, app, Icon::Music, "Music", Page::Music, None);
    nav_item(ui, app, Icon::List, "Playlists", Page::Playlist, None);
    section(ui, "Source");
    if nav_item(ui, app, Icon::Computer, "My Computer", Page::Computer, None).clicked() {
        app.browse_to(None);
    }
    let drives = app.drives.clone();
    for (i, d) in drives.iter().enumerate() {
        if !d.kind.playable() {
            continue;
        }
        let tag = match d.kind {
            DiscKind::DvdVideo => "DVD",
            DiscKind::BluRay => "BD",
            DiscKind::UhdBluRay => "UHD",
            DiscKind::AudioCd => "CD",
            _ => "",
        };
        nav_item(ui, app, Icon::Disc, &truncate(&d.name(), 15), Page::Disc(i), Some((tag, TEXT_DIM)));
    }

    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        ui.add_space(8.0);
        let (txt, col) = if app.player.is_some() { ("Codec engine ready", GOOD) } else { ("Codec engine not installed", WARN) };
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
        ui.painter().circle_filled(pos2(rect.left() + 8.0, rect.center().y), 4.0, col);
        ui.painter().text(pos2(rect.left() + 20.0, rect.center().y), Align2::LEFT_CENTER, txt, FontId::proportional(12.0), col);
        if resp.on_hover_text("Codec engine settings").clicked() {
            app.page = Page::Settings;
            app.settings_tab = SettingsTab::Engine;
        }
        if app.page != Page::NowPlaying && app.current.is_some() {
            ui.add_space(6.0);
            let w = ui.available_width();
            let (rect, resp) = ui.allocate_exact_size(vec2(w, w * 9.0 / 16.0), Sense::click());
            app.paint_video(ui, rect);
            if resp.on_hover_text("Back to the player").clicked() {
                app.page = Page::NowPlaying;
            }
        }
    });
}

fn now_playing(app: &mut App, ui: &mut Ui) {
    let rect = ui.max_rect();
    let resp = ui.interact(rect, ui.scope_id().with("video_area"), Sense::click());
    if app.current.is_none() {
        let r = Rect::from_center_size(rect.center() - vec2(0.0, 40.0), vec2(140.0, 140.0));
        paint_disc_art(ui.painter(), r, 0.0);
        ui.painter().text(rect.center() + vec2(0.0, 60.0), Align2::CENTER_CENTER, "Nothing is playing", FontId::proportional(20.0), TEXT);
        ui.painter().text(
            rect.center() + vec2(0.0, 88.0),
            Align2::CENTER_CENTER,
            "Insert a disc, open a file, or drop files onto this window",
            FontId::proportional(14.0),
            TEXT_DIM,
        );
        return;
    }
    app.paint_video(ui, rect);
    // Disc menus: point at buttons to select them, click to choose.
    let menu = app.nav_in_menu();
    if menu {
        if let Some(pos) = resp.hover_pos() {
            if app.nav_last_mouse.map_or(true, |p| p.distance(pos) > 2.0) {
                app.nav_last_mouse = Some(pos);
                if let Some((x, y)) = app.nav_point(pos) {
                    app.nav_input(crate::nav::NavInput::MouseMove(x, y));
                }
            }
        }
    }
    if resp.double_clicked() && !menu {
        let ctx = ui.ctx().clone();
        let fs = !app.fullscreen;
        app.set_fullscreen(&ctx, fs);
    } else if resp.clicked() {
        match resp.interact_pointer_pos().and_then(|p| app.nav_point(p)) {
            Some((x, y)) if menu => app.nav_input(crate::nav::NavInput::MouseClick(x, y)),
            _ => app.toggle_pause(),
        }
    }
    if menu {
        ui.painter().text(
            rect.center_bottom() - vec2(0.0, 14.0),
            Align2::CENTER_BOTTOM,
            "Disc menu: click a button, or use the arrow keys and Enter",
            FontId::proportional(13.0),
            Color32::from_white_alpha(150),
        );
    }
    resp.context_menu(|ui| more_menu(app, ui));
    if !app.fullscreen {
        // Small back arrow, like the other pages.
        let b = Rect::from_min_size(rect.min + vec2(14.0, 14.0), vec2(28.0, 28.0));
        let br = ui.interact(b, ui.scope_id().with("np_back"), Sense::click());
        if br.hovered() {
            ui.painter().rect_filled(b, 3.0, Color32::from_black_alpha(160));
            let c = b.center();
            let s = Stroke::new(2.0, Color32::WHITE);
            ui.painter().line_segment([pos2(c.x + 3.0, c.y - 7.0), pos2(c.x - 4.0, c.y)], s);
            ui.painter().line_segment([pos2(c.x - 4.0, c.y), pos2(c.x + 3.0, c.y + 7.0)], s);
        }
        if br.clicked() {
            app.page = Page::Computer;
        }
    }
    if app.paused {
        let c = rect.center();
        ui.painter().circle_filled(c, 40.0, Color32::from_black_alpha(150));
        icons::paint(ui.painter(), Icon::Pause, Rect::from_center_size(c, vec2(44.0, 44.0)), Color32::WHITE);
    }
    if app.show_info {
        let lines = [
            ("Title", app.current.as_ref().map(|c| c.title.clone()).unwrap_or_default()),
            ("Source", app.current.as_ref().map(|c| c.badge()).unwrap_or_default()),
            ("Container", app.info.format.clone()),
            ("Video", app.info.video.clone()),
            ("Audio", app.info.audio.clone()),
            ("Decoding", app.info.hw.clone()),
        ];
        let box_r = Rect::from_min_size(rect.min + vec2(56.0, 16.0), vec2(440.0, 22.0 * lines.len() as f32 + 16.0));
        ui.painter().rect_filled(box_r, 6.0, Color32::from_black_alpha(190));
        for (i, (k, v)) in lines.iter().enumerate() {
            let y = box_r.top() + 18.0 + i as f32 * 22.0;
            ui.painter().text(pos2(box_r.left() + 12.0, y), Align2::LEFT_CENTER, k, FontId::proportional(13.0), TEXT_DIM);
            ui.painter().text(pos2(box_r.left() + 100.0, y), Align2::LEFT_CENTER, truncate(v, 48), FontId::proportional(13.0), TEXT);
        }
    }
}

fn more_menu(app: &mut App, ui: &mut Ui) {
    ui.set_min_width(230.0);
    ui.label(RichText::new("Speed").color(TEXT_DIM));
    ui.horizontal(|ui| {
        for s in [0.5, 0.75, 1.0, 1.25, 1.5, 2.0] {
            if ui.selectable_label((app.speed - s).abs() < 0.01, format!("{s}×")).clicked() {
                if let Some(p) = &app.player {
                    p.set("speed", &s.to_string());
                }
            }
        }
    });
    ui.separator();
    let ab_label = match app.ab {
        (None, _) => "Repeat a section: set A",
        (Some(_), None) => "Repeat a section: set B",
        _ => "Stop section repeat",
    };
    if ui.button(ab_label).clicked() {
        app.ab_repeat();
        ui.close();
    }
    if ui.button("Take a snapshot (S)").clicked() {
        app.snapshot();
        ui.close();
    }
    if ui.button("Load subtitle file…").clicked() {
        ui.close();
        app.load_subtitle();
    }
    ui.separator();
    ui.label(RichText::new("Aspect ratio").color(TEXT_DIM));
    ui.horizontal(|ui| {
        for (l, v) in [("Auto", "no"), ("4:3", "4:3"), ("16:9", "16:9"), ("2.35:1", "2.35:1")] {
            if ui.button(l).clicked() {
                if let Some(p) = &app.player {
                    p.set("video-aspect-override", v);
                }
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new("Zoom").color(TEXT_DIM));
        for (l, v) in [("Fit", "0"), ("Fill screen", "1.0")] {
            if ui.button(l).clicked() {
                if let Some(p) = &app.player {
                    p.set("panscan", v);
                }
            }
        }
    });
    ui.separator();
    ui.checkbox(&mut app.show_info, "Show media information (I)");
    if ui.button("Eject disc").clicked() {
        let d = app.current_drive().or_else(|| app.drives.iter().find(|d| d.kind.playable()).cloned());
        if let Some(d) = d {
            if app.current.as_ref().is_some_and(|c| c.disc.is_some()) {
                app.stop();
            }
            discs::eject(&d);
        }
        ui.close();
    }
}

fn control_bar(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let full = ui.max_rect();
    ui.painter().line_segment([full.left_top(), full.right_top()], Stroke::new(1.0, Color32::from_rgb(64, 64, 64)));
    let playing = app.current.is_some();
    let mut row = full;
    if playing {
        let seek_rect = Rect::from_min_max(full.min + vec2(0.0, 6.0), pos2(full.right(), full.top() + 26.0));
        ui.scope_builder(UiBuilder::new().max_rect(seek_rect), |ui| seek_bar(app, ui));
        row.min.y = seek_rect.bottom();
        // Title and time on the left.
        let left = Rect::from_min_max(row.min, pos2(row.left() + 300.0, row.bottom()));
        ui.scope_builder(UiBuilder::new().max_rect(left).layout(Layout::top_down(Align::Min)), |ui| {
            ui.add_space(10.0);
            let title = app.current.as_ref().map(|c| c.title.clone()).unwrap_or_default();
            ui.label(RichText::new(truncate(&title, 36)).size(14.0).color(TEXT));
            ui.horizontal(|ui| {
                let t = app.seek_preview.unwrap_or(app.time);
                ui.label(RichText::new(format!("{} / {}", fmt_time(t), fmt_time(app.duration))).size(12.0).color(TEXT_DIM));
                if let Some(c) = &app.current {
                    badge(ui, &c.badge());
                }
                if app.chapter >= 0 && app.chapters.len() > 1 {
                    badge(ui, &format!("Ch {}/{}", app.chapter + 1, app.chapters.len()));
                }
            });
        });
    }

    // Centre group.
    let btn = 30.0;
    let gap = 10.0;
    let count = if playing { if app.nav.is_some() { 14.0 } else { 13.0 } } else { 3.0 };
    let width = count * (btn + gap) + 40.0 + if playing { 80.0 } else { 0.0 };
    let centre = Rect::from_center_size(pos2(full.center().x + if playing { 60.0 } else { 0.0 }, row.center().y), vec2(width, 44.0));
    ui.scope_builder(UiBuilder::new().max_rect(centre).layout(Layout::left_to_right(Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = gap;
        if playing {
            if icons::button(ui, Icon::Shuffle, btn, "Shuffle", app.shuffle).clicked() {
                app.shuffle = !app.shuffle;
            }
            let rep_tip = match app.repeat {
                Repeat::Off => "Repeat: off",
                Repeat::One => "Repeat: this item",
                Repeat::All => "Repeat: whole playlist",
            };
            if icons::button(ui, Icon::Repeat, btn, rep_tip, app.repeat != Repeat::Off).clicked() {
                app.repeat = match app.repeat {
                    Repeat::Off => Repeat::All,
                    Repeat::All => Repeat::One,
                    Repeat::One => Repeat::Off,
                };
            }
            if icons::button(ui, Icon::Stop, btn, "Stop", false).clicked() {
                app.stop();
            }
            if icons::button(ui, Icon::Prev, btn, "Previous chapter / track (PgUp)", false).clicked() {
                app.chapter_step(-1);
            }
            if icons::button(ui, Icon::Rewind, btn, "Back 10 seconds (←)", false).clicked() {
                app.seek_rel(-10.0);
            }
        }
        let icon = if playing && !app.paused { Icon::Pause } else { Icon::Play };
        if icons::big_button(ui, icon, 40.0, "Play / Pause (Space)").clicked() {
            app.toggle_pause();
        }
        if playing {
            if icons::button(ui, Icon::Forward, btn, "Forward 30 seconds (→)", false).clicked() {
                app.seek_rel(30.0);
            }
            if icons::button(ui, Icon::Next, btn, "Next chapter / track (PgDn)", false).clicked() {
                app.chapter_step(1);
            }
            if app.nav.is_some() {
                let in_menu = app.nav_in_menu();
                let menu_btn = icons::button(ui, Icon::Home, btn, "Disc menu (Backspace)", in_menu);
                if menu_btn.clicked() {
                    let popup = app.nav.as_ref().is_some_and(|n| n.shared.popup.load(std::sync::atomic::Ordering::Acquire));
                    app.nav_input(if popup && !in_menu { crate::nav::NavInput::Popup } else { crate::nav::NavInput::Menu });
                    app.page = Page::NowPlaying;
                }
            }
            let audio_btn = icons::button(ui, Icon::Audio, btn, "Audio language and format", false);
            egui::Popup::menu(&audio_btn).width(300.0).show(|ui| track_menu(app, ui, "audio"));
            let subs_on = app.tracks.iter().any(|t| t.kind == "sub" && t.selected);
            let sub_btn = icons::button(ui, Icon::Subtitles, btn, "Subtitles", subs_on);
            egui::Popup::menu(&sub_btn).width(300.0).show(|ui| track_menu(app, ui, "sub"));
            let chapters_btn = icons::button(ui, Icon::Chapters, btn, "Titles and chapters", false);
            egui::Popup::menu(&chapters_btn).width(260.0).show(|ui| chapter_menu(app, ui));
        }
        let vic = if app.muted || app.settings.volume == 0.0 { Icon::Mute } else { Icon::Volume };
        let vol = icons::button(ui, vic, btn, "Volume (click to mute, M)", false);
        if vol.clicked() {
            if let Some(p) = &app.player {
                let _ = p.command(&["cycle", "mute"]);
            }
        }
        if playing {
            volume_slider(app, ui);
        }
        let more = icons::button(ui, Icon::More, btn, "More: speed, A-B repeat, snapshot, aspect ratio, eject", app.show_info);
        egui::Popup::menu(&more).width(250.0).show(|ui| more_menu(app, ui));
        if playing && icons::button(ui, Icon::Fullscreen, btn, "Full screen (F)", app.fullscreen).clicked() {
            let fs = !app.fullscreen;
            app.set_fullscreen(&ctx, fs);
        }
    });
}

fn seek_bar(app: &mut App, ui: &mut Ui) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::click_and_drag());
    let track = Rect::from_center_size(rect.center(), vec2(rect.width() - 8.0, 4.0));
    let frac = if app.duration > 0.0 { (app.time / app.duration).clamp(0.0, 1.0) as f32 } else { 0.0 };
    let p = ui.painter();
    p.rect_filled(track, 2.0, Color32::from_rgb(70, 70, 70));
    if let Some(a) = app.ab.0 {
        if app.duration > 0.0 {
            let b = app.ab.1.unwrap_or(app.time);
            let xa = track.left() + track.width() * (a / app.duration) as f32;
            let xb = track.left() + track.width() * (b / app.duration) as f32;
            p.rect_filled(Rect::from_x_y_ranges(xa..=xb.max(xa + 2.0), track.y_range()), 0.0, Color32::from_rgb(255, 196, 0));
        }
    }
    p.rect_filled(Rect::from_min_size(track.min, vec2(track.width() * frac, track.height())), 2.0, ACCENT);
    if app.duration > 0.0 {
        for c in &app.chapters {
            let x = track.left() + track.width() * (c.time / app.duration) as f32;
            p.line_segment([pos2(x, track.top()), pos2(x, track.bottom())], Stroke::new(2.0, BAR));
        }
    }
    let knob = pos2(track.left() + track.width() * frac, track.center().y);
    p.circle_filled(knob, if resp.hovered() || resp.dragged() { 7.0 } else { 5.0 }, Color32::WHITE);
    let Some(pos) = resp.hover_pos().or(resp.interact_pointer_pos()) else { return };
    if app.duration <= 0.0 {
        return;
    }
    let f = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0) as f64;
    let t = f * app.duration;
    let chapter = app.chapters.iter().rposition(|c| c.time <= t).map(|i| {
        let c = &app.chapters[i];
        if c.title.is_empty() { format!(" · Chapter {}", i + 1) } else { format!(" · {}", c.title) }
    });
    let tip = format!("{}{}", fmt_time(t), chapter.unwrap_or_default());
    let galley = ui.painter().layout_no_wrap(tip, FontId::proportional(12.0), Color32::WHITE);
    let x = pos.x.clamp(rect.left() + 60.0, rect.right() - 60.0);
    let bg = Rect::from_center_size(pos2(x, rect.top() - 14.0), galley.size() + vec2(12.0, 6.0));
    let tip_painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("seek_tip")));
    tip_painter.rect_filled(bg, 4.0, Color32::from_black_alpha(230));
    tip_painter.galley(bg.min + vec2(6.0, 3.0), galley, Color32::WHITE);
    if resp.dragged() {
        app.seek_preview = Some(t);
        if let Some(pl) = &app.player {
            let _ = pl.command(&["seek", &format!("{t:.2}"), "absolute+keyframes"]);
        }
    }
    if resp.drag_stopped() || resp.clicked() {
        app.seek_abs(t);
        app.seek_preview = None;
    }
}

fn volume_slider(app: &mut App, ui: &mut Ui) {
    let (vr, vresp) = ui.allocate_exact_size(vec2(80.0, 20.0), Sense::click_and_drag());
    let vtrack = Rect::from_center_size(vr.center(), vec2(vr.width() - 8.0, 4.0));
    let vf = (app.settings.volume / 150.0) as f32;
    let full = Rect::from_min_size(vtrack.min, vec2(vtrack.width() * (100.0 / 150.0), 4.0));
    ui.painter().rect_filled(vtrack, 2.0, Color32::from_rgb(70, 70, 70));
    ui.painter().line_segment([pos2(full.right(), vtrack.top() - 3.0), pos2(full.right(), vtrack.bottom() + 3.0)], Stroke::new(1.0, TEXT_DIM));
    let fillc = if app.muted { TEXT_DIM } else if app.settings.volume > 100.0 { WARN } else { Color32::from_rgb(220, 220, 220) };
    ui.painter().rect_filled(Rect::from_min_size(vtrack.min, vec2(vtrack.width() * vf, 4.0)), 2.0, fillc);
    ui.painter().circle_filled(pos2(vtrack.left() + vtrack.width() * vf, vtrack.center().y), 5.0, Color32::WHITE);
    if let Some(pos) = vresp.interact_pointer_pos() {
        if vresp.dragged() || vresp.clicked() {
            let f = ((pos.x - vtrack.left()) / vtrack.width()).clamp(0.0, 1.0) as f64;
            app.set_volume(f * 150.0);
        }
    }
    if vresp.drag_stopped() || vresp.clicked() {
        app.settings.save();
    }
    vresp.on_hover_text(format!("Volume {:.0}%  (↑ ↓ to change; above 100% boosts quiet discs)", app.settings.volume));
}

fn track_menu(app: &mut App, ui: &mut Ui, kind: &str) {
    if kind == "sub" {
        if let Some(n) = app.nav.clone() {
            ui.label(RichText::new("Subtitles").strong());
            let mut on = n.shared.show_subs.load(std::sync::atomic::Ordering::Acquire);
            if ui.checkbox(&mut on, "Show the disc's subtitles").changed() {
                n.set_show_subs(on);
                app.settings.subtitles_on = on;
                app.settings.save();
            }
            ui.label(RichText::new("Choose the subtitle language from the disc menu.").color(TEXT_DIM).size(12.0));
            return;
        }
    }
    let tracks: Vec<Track> = app.tracks.iter().filter(|t| t.kind == kind).cloned().collect();
    ui.label(RichText::new(if kind == "audio" { "Audio" } else { "Subtitles" }).strong());
    if kind == "sub" {
        let none_sel = !tracks.iter().any(|t| t.selected);
        if ui.selectable_label(none_sel, "Off").clicked() {
            app.select_track("sub", None);
            ui.close();
        }
    }
    if tracks.is_empty() {
        ui.label(RichText::new("None on this title").color(TEXT_DIM));
    }
    for t in tracks {
        if ui.selectable_label(t.selected, t.label()).clicked() {
            app.select_track(kind, Some(t.id));
            ui.close();
        }
    }
    ui.separator();
    let (label, prop) = if kind == "sub" { ("Subtitle timing", "sub-delay") } else { ("Audio sync", "audio-delay") };
    ui.horizontal(|ui| {
        ui.label(label);
        for (l, v) in [("−0.1 s", "-0.1"), ("+0.1 s", "0.1")] {
            if ui.small_button(l).clicked() {
                if let Some(p) = &app.player {
                    let _ = p.command(&["add", prop, v]);
                }
            }
        }
        if ui.small_button("Reset").clicked() {
            if let Some(p) = &app.player {
                p.set(prop, "0");
            }
        }
    });
    if kind == "sub" && ui.button("Load subtitle file…").clicked() {
        ui.close();
        app.load_subtitle();
    }
}

fn chapter_menu(app: &mut App, ui: &mut Ui) {
    if app.disc_titles > 1 {
        ui.label(RichText::new("Titles").strong());
        egui::ScrollArea::vertical().id_salt("titles").max_height(160.0).show(ui, |ui| {
            for i in 0..app.disc_titles.min(99) {
                if ui.selectable_label(app.disc_title == i, format!("Title {}", i + 1)).clicked() {
                    if let Some(p) = &app.player {
                        p.set("disc-title", &i.to_string());
                    }
                    ui.close();
                }
            }
        });
        ui.separator();
    }
    ui.label(RichText::new("Chapters").strong());
    if app.chapters.is_empty() {
        ui.label(RichText::new("No chapters").color(TEXT_DIM));
    }
    let chapters = app.chapters.clone();
    egui::ScrollArea::vertical().id_salt("chapters").max_height(300.0).show(ui, |ui| {
        for (i, c) in chapters.iter().enumerate() {
            let name = if c.title.is_empty() { format!("Chapter {}", i + 1) } else { c.title.clone() };
            if ui.selectable_label(app.chapter == i as i64, format!("{name}   {}", fmt_time(c.time))).clicked() {
                if let Some(p) = &app.player {
                    p.set("chapter", &i.to_string());
                }
                ui.close();
            }
        }
    });
}

fn handle_keys(app: &mut App, ctx: &egui::Context) {
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    use egui::Key;
    let (keys, ctrl) = ctx.input(|i| {
        let keys: Vec<Key> = [
            Key::Space, Key::ArrowLeft, Key::ArrowRight, Key::ArrowUp, Key::ArrowDown, Key::F, Key::F11, Key::Escape, Key::M,
            Key::S, Key::I, Key::PageUp, Key::PageDown, Key::Home, Key::O, Key::Enter, Key::Backspace,
        ]
        .into_iter()
        .filter(|k| i.key_pressed(*k))
        .collect();
        (keys, i.modifiers.command)
    });
    // In a disc menu, arrows and Enter move between and choose its buttons.
    if app.nav_in_menu() && app.page == Page::NowPlaying {
        use crate::nav::NavInput;
        let mut rest = Vec::new();
        for k in keys {
            match k {
                Key::ArrowUp => app.nav_input(NavInput::Up),
                Key::ArrowDown => app.nav_input(NavInput::Down),
                Key::ArrowLeft => app.nav_input(NavInput::Left),
                Key::ArrowRight => app.nav_input(NavInput::Right),
                Key::Enter | Key::Space => app.nav_input(NavInput::Enter),
                other => rest.push(other),
            }
        }
        handle_play_keys(app, ctx, rest, ctrl);
        return;
    }
    handle_play_keys(app, ctx, keys, ctrl);
}

fn handle_play_keys(app: &mut App, ctx: &egui::Context, keys: Vec<egui::Key>, ctrl: bool) {
    use egui::Key;
    for k in keys {
        match k {
            Key::Backspace if app.nav.is_some() => app.nav_input(crate::nav::NavInput::Menu),
            Key::Space => app.toggle_pause(),
            Key::ArrowLeft => app.seek_rel(-10.0),
            Key::ArrowRight => app.seek_rel(30.0),
            Key::ArrowUp => app.set_volume(app.settings.volume + 5.0),
            Key::ArrowDown => app.set_volume(app.settings.volume - 5.0),
            Key::F | Key::F11 => {
                let fs = !app.fullscreen;
                app.set_fullscreen(ctx, fs);
            }
            Key::Escape if app.fullscreen => app.set_fullscreen(ctx, false),
            Key::M => {
                if let Some(p) = &app.player {
                    let _ = p.command(&["cycle", "mute"]);
                }
            }
            Key::S => app.snapshot(),
            Key::I => app.show_info = !app.show_info,
            Key::PageUp => app.chapter_step(-1),
            Key::PageDown => app.chapter_step(1),
            Key::Home => app.seek_abs(0.0),
            Key::O if ctrl => app.open_files(),
            _ => {}
        }
    }
}

fn draw_toast(app: &mut App, ctx: &egui::Context) {
    let Some((msg, at, warn)) = app.toast.clone() else { return };
    let life = if warn { 9.0 } else { 4.0 };
    if at.elapsed().as_secs_f32() > life {
        app.toast = None;
        return;
    }
    let x = if app.fullscreen { 20.0 } else { 216.0 };
    egui::Area::new(egui::Id::unique("toast"))
        .anchor(Align2::LEFT_TOP, vec2(x, 8.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            Frame::new()
                .fill(if warn { Color32::from_rgb(90, 62, 20) } else { Color32::from_rgb(16, 52, 104) })
                .corner_radius(3.0)
                .inner_margin(Margin::symmetric(12, 4))
                .show(ui, |ui| {
                    ui.set_max_width(600.0);
                    ui.label(RichText::new(msg).size(13.0).color(Color32::WHITE));
                });
        });
    ctx.request_repaint_after(Duration::from_millis(250));
}

// ---------- shared helpers ----------

pub fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = Color32::from_rgb(38, 38, 38);
    v.extreme_bg_color = Color32::from_rgb(52, 52, 52);
    v.selection.bg_fill = Color32::from_rgb(20, 90, 170);
    v.hyperlink_color = ACCENT;
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(48, 48, 48);
    v.widgets.inactive.bg_fill = Color32::from_rgb(48, 48, 48);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(66, 66, 66);
    v.widgets.hovered.bg_fill = Color32::from_rgb(66, 66, 66);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(80, 80, 80);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.corner_radius = CornerRadius::same(3);
    v.widgets.hovered.corner_radius = CornerRadius::same(3);
    v.widgets.active.corner_radius = CornerRadius::same(3);
    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| {
        s.spacing.button_padding = vec2(10.0, 5.0);
        s.spacing.item_spacing = vec2(8.0, 6.0);
    });
}

pub fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n.saturating_sub(1)).collect::<String>())
    } else {
        s.to_string()
    }
}

pub fn badge(ui: &mut Ui, text: &str) {
    if text.is_empty() {
        return;
    }
    Frame::new().fill(Color32::from_rgb(20, 70, 130)).corner_radius(3.0).inner_margin(Margin::symmetric(6, 1)).show(ui, |ui| {
        ui.label(RichText::new(text).size(11.0).color(Color32::from_rgb(200, 225, 255)));
    });
}

/// A shiny disc drawn with rings and a rainbow sheen.
pub fn paint_disc_art(p: &egui::Painter, rect: Rect, spin: f32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.5;
    p.circle_filled(c, r, Color32::from_rgb(190, 198, 212));
    let n = 48;
    for k in 0..n {
        let a0 = std::f32::consts::TAU * k as f32 / n as f32 + spin;
        let a1 = a0 + std::f32::consts::TAU / n as f32 + 0.01;
        let hue = (k as f32 / n as f32 * 2.0).fract();
        let col = hsv(hue, 0.35, 1.0, 120);
        let pts = vec![
            c + vec2(a0.cos(), a0.sin()) * r * 0.36,
            c + vec2(a0.cos(), a0.sin()) * r * 0.98,
            c + vec2(a1.cos(), a1.sin()) * r * 0.98,
            c + vec2(a1.cos(), a1.sin()) * r * 0.36,
        ];
        p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
    }
    p.circle_stroke(c, r * 0.98, Stroke::new(r * 0.03, Color32::from_rgb(220, 226, 236)));
    p.circle_filled(c, r * 0.36, Color32::from_rgb(170, 178, 192));
    p.circle_stroke(c, r * 0.28, Stroke::new(r * 0.02, Color32::from_rgb(140, 148, 160)));
    p.circle_filled(c, r * 0.14, Color32::from_rgb(20, 20, 20));
}

pub fn hsv(h: f32, s: f32, v: f32, a: u8) -> Color32 {
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - f * s), v * (1.0 - (1.0 - f) * s));
    let (r, g, b) = match i as i32 % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color32::from_rgba_unmultiplied((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, a)
}
