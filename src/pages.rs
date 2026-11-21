//! The main content pages: home, My Computer, disc, library, playlist and settings.

use crate::app::*;
use crate::discs::{self, DiscKind, VolumeKind};
use crate::icons::{self, Icon};
use crate::library::{self, fmt_size, fmt_time, MediaItem, MediaKind, VideoGroup};
use crate::mpv::Engine;
use crate::ui::{badge, page_header, truncate};
use eframe::egui::{self, pos2, vec2, Align, Align2, Color32, FontId, Frame, Layout, Margin, Rect, RichText, Sense, Stroke, Ui};
use std::path::{Path, PathBuf};

type Action = Box<dyn FnOnce(&mut App)>;

fn divider(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
    ui.painter().line_segment([pos2(r.left(), r.center().y), pos2(r.right(), r.center().y)], Stroke::new(1.0, Color32::from_rgb(58, 58, 58)));
}

// ---------- Home ----------

pub fn home(app: &mut App, ui: &mut Ui) {
    page_header(app, ui, "Home", false);
    divider(ui);
    let mut action: Option<Action> = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(10.0);
        if app.player.is_none() {
            Frame::new().fill(Color32::from_rgb(70, 52, 20)).corner_radius(3.0).inner_margin(Margin::same(12)).show(ui, |ui| {
                ui.set_width(ui.available_width() - 24.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("One more step: install the free codec engine to play discs and files.").color(WARN).size(15.0));
                    if ui.button("Set it up").clicked() {
                        action = Some(Box::new(|a: &mut App| {
                            a.page = Page::Settings;
                            a.settings_tab = SettingsTab::Engine;
                        }));
                    }
                });
            });
            ui.add_space(14.0);
        }
        let drives = app.drives.clone();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(14.0, 14.0);
            for (i, d) in drives.iter().enumerate() {
                let (title, sub) = if d.kind.playable() {
                    (format!("Play {}", d.kind.label()), d.name())
                } else {
                    ("No disc".to_string(), format!("Drive {} is empty", d.device))
                };
                if hero_card(ui, Icon::Disc, &title, &sub, d.kind.playable()).clicked() {
                    action = Some(Box::new(move |a: &mut App| a.page = Page::Disc(i)));
                }
            }
            if hero_card(ui, Icon::File, "Open files", "Video, music and photos", true).clicked() {
                action = Some(Box::new(|a: &mut App| a.open_files()));
            }
            if hero_card(ui, Icon::Folder, "Open disc folder", "VIDEO_TS or BDMV", true).clicked() {
                action = Some(Box::new(|a: &mut App| a.open_disc_folder()));
            }
            if hero_card(ui, Icon::Disc, "Open disc image", "DVD or Blu-ray .iso", true).clicked() {
                action = Some(Box::new(|a: &mut App| a.open_iso()));
            }
            if hero_card(ui, Icon::Computer, "My Computer", "Browse your drives", true).clicked() {
                action = Some(Box::new(|a: &mut App| a.browse_to(None)));
            }
        });
        ui.add_space(24.0);
        ui.label(RichText::new("Continue watching").size(18.0).color(TEXT));
        ui.add_space(6.0);
        if app.settings.recent.is_empty() {
            ui.label(RichText::new("Things you play will appear here, and they'll pick up where you left off.").color(TEXT_DIM));
        }
        for r in app.settings.recent.clone() {
            let key = format!("{}|{}", r.url, r.device);
            let resume = app.settings.resume.get(&key).copied();
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width().min(760.0), 42.0), Sense::click());
            ui.painter().rect_filled(rect, 3.0, if resp.hovered() { CARD_HOVER } else { CARD });
            let icon = if r.device_opt.is_empty() { Icon::Film } else { Icon::Disc };
            icons::paint(ui.painter(), icon, Rect::from_center_size(pos2(rect.left() + 22.0, rect.center().y), vec2(20.0, 20.0)), TEXT);
            ui.painter().text(pos2(rect.left() + 44.0, rect.center().y), Align2::LEFT_CENTER, truncate(&r.title, 60), FontId::proportional(14.0), TEXT);
            let right = match resume {
                Some(t) => format!("{}  ·  resume at {}", r.kind, fmt_time(t)),
                None => r.kind.clone(),
            };
            ui.painter().text(pos2(rect.right() - 12.0, rect.center().y), Align2::RIGHT_CENTER, right, FontId::proportional(12.0), TEXT_DIM);
            if resp.clicked() {
                let disc = if r.device_opt.is_empty() {
                    None
                } else if r.url.starts_with("bd") {
                    Some(DiscKind::BluRay)
                } else if r.url.starts_with("cdda") {
                    Some(DiscKind::AudioCd)
                } else {
                    Some(DiscKind::DvdVideo)
                };
                let item = PlayItem {
                    title: r.title.clone(),
                    url: r.url.clone(),
                    kind: if disc.is_some() { MediaKind::DiscFolder } else { library::kind_of(Path::new(&r.url)).unwrap_or(MediaKind::Video) },
                    device_opt: r.device_opt.clone(),
                    device: r.device.clone(),
                    disc,
                };
                action = Some(Box::new(move |a: &mut App| a.play_list(vec![item], 0)));
            }
            ui.add_space(4.0);
        }
    });
    if let Some(a) = action {
        a(app);
    }
}

// ---------- My Computer ----------

fn folder_title(app: &App, p: &Path) -> String {
    if p.file_name().is_none() {
        if let Some(v) = app.volumes.iter().find(|v| v.path == p) {
            return v.name.clone();
        }
        return p.display().to_string();
    }
    p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default()
}

pub fn computer_page(app: &mut App, ui: &mut Ui) {
    let title = match &app.browse {
        None => "My Computer".to_string(),
        Some(p) => folder_title(app, p),
    };
    let at_root = app.browse.is_none();
    if page_header(app, ui, &title, !at_root) {
        let parent = app.browse.as_ref().and_then(|p| p.parent().map(Path::to_path_buf));
        app.browse_to(parent);
        return;
    }
    let mut action: Option<Action> = None;
    // Breadcrumb and sort.
    ui.horizontal(|ui| {
        ui.set_height(26.0);
        if let Some(p) = app.browse.clone() {
            let mut parts: Vec<PathBuf> = p.ancestors().map(Path::to_path_buf).collect();
            parts.reverse();
            if crumb(ui, "My Computer") {
                action = Some(Box::new(|a: &mut App| a.browse_to(None)));
            }
            for part in parts {
                arrow(ui);
                let name = folder_title(app, &part);
                if crumb(ui, &name) {
                    action = Some(Box::new(move |a: &mut App| a.browse_to(Some(part))));
                }
            }
            arrow(ui);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = if app.sort_by_date { "By Date ▾" } else { "By Title ▾" };
                let s = ui.add(egui::Button::new(RichText::new(label).size(13.0)).frame(false));
                egui::Popup::menu(&s).show(|ui| {
                    if ui.selectable_label(!app.sort_by_date, "By Title").clicked() {
                        app.sort_by_date = false;
                        app.browse_loaded = None;
                        ui.close();
                    }
                    if ui.selectable_label(app.sort_by_date, "By Date").clicked() {
                        app.sort_by_date = true;
                        app.browse_loaded = None;
                        ui.close();
                    }
                });
            });
        }
    });
    divider(ui);
    ui.add_space(8.0);

    match app.browse.clone() {
        None => {
            let mut tiles: Vec<(Icon, String, PathBuf, Option<VolumeKind>)> = Vec::new();
            for (icon, name, dir) in [
                (Icon::Computer, "Desktop", dirs::desktop_dir()),
                (Icon::Download, "Downloads", dirs::download_dir()),
                (Icon::Music, "Music", dirs::audio_dir()),
                (Icon::Photo, "Photos", dirs::picture_dir()),
                (Icon::Video, "Videos", dirs::video_dir()),
            ] {
                if let Some(d) = dir {
                    tiles.push((icon, name.to_string(), d, None));
                }
            }
            for v in &app.volumes {
                let icon = if v.kind == VolumeKind::Optical { Icon::Disc } else { Icon::Drive };
                tiles.push((icon, v.name.clone(), v.path.clone(), Some(v.kind)));
            }
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(20.0, 18.0);
                for (icon, name, path, kind) in tiles {
                    if square_tile(ui, icon, &name).clicked() {
                        let optical = kind == Some(VolumeKind::Optical);
                        action = Some(Box::new(move |a: &mut App| {
                            if optical {
                                let dev = path.display().to_string().trim_end_matches('\\').to_string();
                                if let Some(i) = a.drives.iter().position(|d| d.device == dev || d.mount.as_ref() == Some(&path)) {
                                    a.page = Page::Disc(i);
                                    return;
                                }
                            }
                            a.browse_to(Some(path));
                        }));
                    }
                }
            });
        }
        Some(path) => {
            if app.browse_loaded.as_ref() != Some(&path) {
                let mut entries = read_folder(&path);
                if app.sort_by_date {
                    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(b.modified.cmp(&a.modified)));
                } else {
                    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
                }
                app.browse_entries = entries;
                app.browse_loaded = Some(path.clone());
            }
            if let Some(kind) = (discs::classify_folder(&path) != DiscKind::Data).then(|| discs::classify_folder(&path)).filter(|k| matches!(k, DiscKind::DvdVideo | DiscKind::BluRay | DiscKind::UhdBluRay)) {
                ui.horizontal(|ui| {
                    badge(ui, kind.label());
                    ui.label(RichText::new("This folder is a disc.").color(TEXT_DIM));
                    if ui.add(egui::Button::new(RichText::new("  Play disc  ").color(Color32::WHITE)).fill(ACCENT)).clicked() {
                        let p = path.clone();
                        action = Some(Box::new(move |a: &mut App| {
                            if let Some(it) = PlayItem::disc_source(&p) {
                                a.play_list(vec![it], 0);
                            }
                        }));
                    }
                });
                ui.add_space(8.0);
            }
            let q = app.search.to_lowercase();
            let entries: Vec<BrowseEntry> = app.browse_entries.iter().filter(|e| q.is_empty() || e.name.to_lowercase().contains(&q)).cloned().collect();
            if entries.is_empty() {
                ui.label(RichText::new("No folders or media files here.").color(TEXT_DIM));
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(60.0, 14.0);
                    for (i, e) in entries.iter().enumerate().take(4000) {
                        let icon = if let Some(d) = e.disc {
                            let _ = d;
                            Icon::Disc
                        } else if e.is_dir {
                            Icon::Folder
                        } else {
                            match e.kind {
                                Some(MediaKind::Music) => Icon::Music,
                                Some(MediaKind::Photo) => Icon::Photo,
                                Some(MediaKind::DiscImage) => Icon::Disc,
                                _ => Icon::Film,
                            }
                        };
                        let tag = if let Some(d) = e.disc {
                            Some(d.label().to_string())
                        } else if !e.is_dir {
                            e.path.extension().map(|x| x.to_string_lossy().to_uppercase())
                        } else {
                            None
                        };
                        let resp = folder_tile(ui, icon, &e.name, tag.as_deref());
                        if resp.clicked() {
                            let e = e.clone();
                            let all = entries.clone();
                            action = Some(Box::new(move |a: &mut App| open_entry(a, &e, &all, i)));
                        }
                        let e2 = e.clone();
                        resp.context_menu(|ui| {
                            if !e2.is_dir && ui.button("Add to playlist").clicked() {
                                if let Some(it) = PlayItem::file(&e2.path) {
                                    action = Some(Box::new(move |a: &mut App| a.enqueue(it)));
                                }
                                ui.close();
                            }
                            if e2.is_dir && ui.button("Add folder to library").clicked() {
                                let p = e2.path.clone();
                                action = Some(Box::new(move |a: &mut App| {
                                    a.settings.folders.push(p);
                                    a.settings.save();
                                    a.scan_rx = Some(library::scan(a.settings.folders.clone()));
                                    a.notify("Folder added to your library.", false);
                                }));
                                ui.close();
                            }
                        });
                    }
                });
            });
        }
    }
    if let Some(a) = action {
        a(app);
    }
}

fn open_entry(app: &mut App, e: &BrowseEntry, all: &[BrowseEntry], index: usize) {
    if e.disc.is_some() {
        if let Some(it) = PlayItem::disc_source(&e.path) {
            app.play_list(vec![it], 0);
            return;
        }
    }
    if e.is_dir {
        app.browse_to(Some(e.path.clone()));
        return;
    }
    match e.kind {
        Some(MediaKind::Music) | Some(MediaKind::Photo) => {
            // Keep going through the rest of this folder's songs or photos.
            let same: Vec<&BrowseEntry> = all.iter().filter(|x| x.kind == e.kind).collect();
            let start = same.iter().position(|x| x.path == e.path).unwrap_or(0);
            let list: Vec<PlayItem> = same.iter().filter_map(|x| PlayItem::file(&x.path)).collect();
            app.play_list(list, start);
        }
        _ => {
            let _ = index;
            match PlayItem::file(&e.path) {
                Some(it) => app.play_list(vec![it], 0),
                None => app.notify("That image doesn't look like a DVD or Blu-ray.", true),
            }
        }
    }
}

fn crumb(ui: &mut Ui, text: &str) -> bool {
    ui.add(egui::Button::new(RichText::new(truncate(text, 30)).size(13.0).color(TEXT)).frame(false)).clicked()
}

fn arrow(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
    let c = r.center();
    ui.painter().add(egui::Shape::convex_polygon(
        vec![pos2(c.x - 3.0, c.y - 5.0), pos2(c.x + 4.0, c.y), pos2(c.x - 3.0, c.y + 5.0)],
        Color32::WHITE,
        Stroke::NONE,
    ));
}

fn square_tile(ui: &mut Ui, icon: Icon, label: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(96.0, 118.0), Sense::click());
    let sq = Rect::from_min_size(rect.min, vec2(96.0, 96.0));
    ui.painter().rect_filled(sq, 2.0, if resp.hovered() { Color32::from_rgb(52, 52, 52) } else { Color32::from_rgb(38, 38, 38) });
    if resp.hovered() {
        ui.painter().rect_stroke(sq, 2.0, Stroke::new(2.0, ACCENT), egui::StrokeKind::Inside);
    }
    icons::paint(ui.painter(), icon, Rect::from_center_size(sq.center(), vec2(34.0, 34.0)), Color32::from_rgb(230, 230, 230));
    ui.painter().text(pos2(sq.center().x, sq.bottom() + 14.0), Align2::CENTER_CENTER, truncate(label, 16), FontId::proportional(13.0), TEXT);
    resp.on_hover_text(label)
}

fn folder_tile(ui: &mut Ui, icon: Icon, label: &str, tag: Option<&str>) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(172.0, 150.0), Sense::click());
    let body = Rect::from_min_size(rect.min + vec2(0.0, 12.0), vec2(172.0, 96.0));
    // stacked-sheet look
    ui.painter().rect_filled(Rect::from_min_size(rect.min + vec2(8.0, 2.0), vec2(156.0, 6.0)), 1.0, Color32::from_rgb(74, 74, 74));
    ui.painter().rect_filled(Rect::from_min_size(rect.min + vec2(4.0, 7.0), vec2(164.0, 6.0)), 1.0, Color32::from_rgb(86, 86, 86));
    let top = if resp.hovered() { Color32::from_rgb(132, 132, 132) } else { Color32::from_rgb(110, 110, 110) };
    crate::ui::gradient(ui.painter(), body, top, Color32::from_rgb(56, 56, 56));
    if resp.hovered() {
        ui.painter().rect_stroke(body, 0.0, Stroke::new(2.0, ACCENT), egui::StrokeKind::Inside);
    }
    icons::paint(ui.painter(), icon, Rect::from_center_size(body.center(), vec2(64.0, 46.0)), Color32::from_rgb(150, 150, 150));
    if let Some(t) = tag {
        let tr = Rect::from_min_size(body.right_bottom() - vec2(70.0, 22.0), vec2(64.0, 17.0));
        ui.painter().rect_filled(tr, 2.0, Color32::from_black_alpha(170));
        ui.painter().text(tr.center(), Align2::CENTER_CENTER, truncate(t, 10), FontId::proportional(10.0), Color32::WHITE);
    }
    // up to two lines of name
    let name = label.to_string();
    let galley = ui.painter().layout(name, FontId::proportional(13.0), TEXT, 196.0);
    let rows = galley.rows.len();
    let pos = pos2(body.center().x - galley.size().x / 2.0, body.bottom() + 6.0);
    if rows <= 2 {
        ui.painter().galley(pos, galley, TEXT);
    } else {
        let short = ui.painter().layout(truncate(label, 48), FontId::proportional(13.0), TEXT, 196.0);
        let pos = pos2(body.center().x - short.size().x / 2.0, body.bottom() + 6.0);
        ui.painter().with_clip_rect(Rect::from_min_size(pos2(rect.left() - 20.0, body.bottom()), vec2(212.0, 40.0))).galley(pos, short, TEXT);
    }
    resp.on_hover_text(label)
}

// ---------- Disc ----------

pub fn disc_page(app: &mut App, ui: &mut Ui, i: usize) {
    let Some(d) = app.drives.get(i).cloned() else {
        app.page = Page::Computer;
        return;
    };
    if page_header(app, ui, "", true) {
        app.browse_to(None);
        return;
    }
    let mut action: Option<Action> = None;
    let item = PlayItem::drive(&d);
    let resume = app.settings.resume.get(&item.key()).copied();
    ui.add_space(150.0);
    ui.horizontal(|ui| {
        ui.add_space(18.0);
        ui.vertical(|ui| {
            ui.set_max_width(620.0);
            let title = if d.kind.playable() { format!("{} ({})", d.label, d.device) } else { format!("Empty drive ({})", d.device) };
            ui.label(RichText::new(title).size(24.0).color(Color32::WHITE));
            ui.add_space(4.0);
            badge(ui, d.kind.label());
            ui.add_space(18.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if d.kind.playable() {
                    let main_label = if resume.is_some() { "Resume" } else { "Play" };
                    if icon_button(ui, Icon::Play, main_label, true).clicked() {
                        let it = item.clone();
                        action = Some(Box::new(move |a: &mut App| a.play_list(vec![it], 0)));
                    }
                    if app.settings.disc_menus && matches!(d.kind, DiscKind::DvdVideo | DiscKind::BluRay | DiscKind::UhdBluRay) && icon_button(ui, Icon::Film, "Play movie (skip menus)", false).clicked() {
                        let it = item.clone();
                        action = Some(Box::new(move |a: &mut App| {
                            a.skip_menus_next = true;
                            a.play_list(vec![it], 0);
                        }));
                    }
                    if resume.is_some() && icon_button(ui, Icon::Repeat, "Restart", false).clicked() {
                        let it = item.clone();
                        action = Some(Box::new(move |a: &mut App| {
                            a.settings.resume.remove(&it.key());
                            a.play_list(vec![it], 0);
                        }));
                    }
                    if matches!(d.kind, DiscKind::Unknown | DiscKind::Data) {
                        let other = icon_button(ui, Icon::Disc, "Play as…", false);
                        egui::Popup::menu(&other).show(|ui| {
                            for k in [DiscKind::DvdVideo, DiscKind::BluRay, DiscKind::AudioCd] {
                                if ui.button(k.label()).clicked() {
                                    let mut dd = d.clone();
                                    dd.kind = k;
                                    ui.close();
                                    action = Some(Box::new(move |a: &mut App| a.play_list(vec![PlayItem::drive(&dd)], 0)));
                                }
                            }
                        });
                    }
                    if let Some(m) = d.mount.clone() {
                        if icon_button(ui, Icon::Folder, "Browse files", false).clicked() {
                            action = Some(Box::new(move |a: &mut App| a.browse_to(Some(m))));
                        }
                    }
                }
                if icon_button(ui, Icon::Eject, "Eject", false).clicked() {
                    let dd = d.clone();
                    action = Some(Box::new(move |a: &mut App| {
                        if a.current_drive().is_some_and(|c| c.device == dd.device) {
                            a.stop();
                        }
                        discs::eject(&dd);
                    }));
                }
            });
            if let Some(t) = resume {
                ui.add_space(6.0);
                ui.label(RichText::new(format!("You stopped at {}.", fmt_time(t))).color(TEXT_DIM));
            }
            let protected = matches!(d.kind, DiscKind::BluRay | DiscKind::UhdBluRay) && d.mount.as_ref().is_some_and(|m| discs::is_protected(m));
            let have_keys = library::keydb_path().is_some_and(|p| p.exists());
            if protected {
                ui.add_space(16.0);
                Frame::new().fill(Color32::from_rgb(48, 40, 24)).stroke(Stroke::new(1.0, Color32::from_rgb(110, 86, 40))).corner_radius(3.0).inner_margin(Margin::same(12)).show(ui, |ui| {
                    ui.label(RichText::new("This disc is copy-protected (AACS)").color(WARN).size(15.0));
                    let msg = if have_keys {
                        "Your key file is installed, so Play will try to unlock it. If the disc is newer than your key file, use one of the options below."
                    } else {
                        "Open-source players can only unlock it with a key file, which this player can't download or supply. Ways to watch it:"
                    };
                    ui.label(RichText::new(msg).color(TEXT));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let lbl = match &app.settings.licensed_player {
                            Some(p) if p.exists() => format!("Open in {}", p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()),
                            _ => "Open in a licensed player…".to_string(),
                        };
                        if icon_button(ui, Icon::Play, &lbl, true).on_hover_text("Hands this disc to a licensed Blu-ray player you own").clicked() {
                            let dd = d.clone();
                            action = Some(Box::new(move |a: &mut App| a.open_in_licensed_player(&dd)));
                        }
                        if icon_button(ui, Icon::File, "Import my key file…", false).clicked() {
                            action = Some(Box::new(|a: &mut App| {
                                a.page = Page::Settings;
                                a.settings_tab = SettingsTab::Discs;
                            }));
                        }
                    });
                    ui.add_space(4.0);
                    ui.label(RichText::new("Other lawful options: many discs include a digital-copy code for streaming, and backups you made yourself with a licensed tool play here as folders or ISO files.").color(TEXT_DIM).size(12.0));
                });
            }
            ui.add_space(22.0);
            let note = match d.kind {
                DiscKind::DvdVideo => "Opens the disc's own menu: click a button, or use the arrow keys and Enter. The house button in the controls (or Backspace) brings the menu back. Choose Play movie (skip menus) to go straight to the main feature.",
                DiscKind::BluRay => "Opens the disc's own menu (standard Blu-ray menus; Java-based menus fall back to the main movie). Most commercial Blu-ray discs are encrypted (AACS): the free libaacs library plus your own key file are needed — see Settings › Discs & Blu-ray. Home-made and unprotected discs play straight away.",
                DiscKind::UhdBluRay => "Ultra HD Blu-ray (4K / HDR). These discs use AACS 2.0 encryption, which open-source libraries can only open with matching keys; unprotected discs and your own backups play normally.",
                DiscKind::AudioCd => "Plays every track in order. Use next and previous to move between tracks.",
                DiscKind::Data => "A data disc. Use Browse files to play the videos, music or photos on it.",
                DiscKind::Unknown => "A disc is present but couldn't be read as files (often an audio CD on Linux). Press Play or choose Play as….",
                DiscKind::Empty => "Insert a DVD, CD or Blu-ray and it will appear here automatically.",
            };
            ui.label(RichText::new(note).color(TEXT_DIM));
            if matches!(d.kind, DiscKind::BluRay | DiscKind::UhdBluRay) {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Blu-ray codec information:").color(TEXT_DIM));
                    ui.hyperlink_to(crate::aacs::CODEC_INFO_URL, crate::aacs::CODEC_INFO_URL);
                });
            }
        });
    });
    if let Some(a) = action {
        a(app);
    }
}

fn icon_button(ui: &mut Ui, icon: Icon, label: &str, primary: bool) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(14.0), Color32::WHITE);
    let w = galley.size().x + 52.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w.max(114.0), 36.0), Sense::click());
    if primary {
        ui.painter().rect_filled(rect, 2.0, if resp.hovered() { ACCENT_HOVER } else { ACCENT });
    } else {
        if resp.hovered() {
            ui.painter().rect_filled(rect, 2.0, Color32::from_white_alpha(12));
        }
        ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_rgb(200, 200, 200)), egui::StrokeKind::Inside);
    }
    let content_w = 14.0 + 8.0 + galley.size().x;
    let x0 = rect.center().x - content_w / 2.0;
    icons::paint(ui.painter(), icon, Rect::from_center_size(pos2(x0 + 7.0, rect.center().y), vec2(16.0, 16.0)), Color32::WHITE);
    ui.painter().galley(pos2(x0 + 22.0, rect.center().y - galley.size().y / 2.0), galley, Color32::WHITE);
    resp
}

// ---------- Library ----------

pub fn library_page(app: &mut App, ui: &mut Ui, page: Page) {
    let title = match page {
        Page::Movies => "Movies",
        Page::TvShows => "TV Shows",
        Page::Music => "Music",
        Page::Photos => "Photos",
        _ => "Videos",
    };
    page_header(app, ui, title, false);
    let q = app.search.to_lowercase();
    let items: Vec<MediaItem> = app
        .library
        .iter()
        .filter(|m| match page {
            Page::Music => m.kind == MediaKind::Music,
            Page::Photos => m.kind == MediaKind::Photo,
            Page::Movies => matches!(m.kind, MediaKind::Video | MediaKind::DiscFolder | MediaKind::DiscImage) && library::video_group(m) == VideoGroup::Movie,
            Page::TvShows => m.kind == MediaKind::Video && library::video_group(m) == VideoGroup::TvShow,
            _ => m.kind == MediaKind::Video && library::video_group(m) == VideoGroup::Other || (!q.is_empty() && matches!(m.kind, MediaKind::Video | MediaKind::DiscFolder | MediaKind::DiscImage)),
        })
        .filter(|m| q.is_empty() || m.name.to_lowercase().contains(&q))
        .cloned()
        .collect();
    let photos = page == Page::Photos;
    let music = page == Page::Music;
    let mut action: Option<Action> = None;
    ui.horizontal(|ui| {
        ui.set_height(26.0);
        ui.label(RichText::new(format!("{} items", items.len())).color(TEXT_DIM));
        if !app.search.is_empty() {
            ui.label(RichText::new(format!("matching \"{}\"", app.search)).color(TEXT_DIM));
            if ui.small_button("Clear").clicked() {
                app.search.clear();
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Add folder…").clicked() {
                action = Some(Box::new(|a: &mut App| {
                    if let Some(f) = rfd::FileDialog::new().pick_folder() {
                        a.settings.folders.push(f);
                        a.settings.save();
                        a.scan_rx = Some(library::scan(a.settings.folders.clone()));
                    }
                }));
            }
            if ui.button("Refresh").clicked() {
                action = Some(Box::new(|a: &mut App| a.scan_rx = Some(library::scan(a.settings.folders.clone()))));
            }
            if !items.is_empty() {
                if !photos && ui.button("Shuffle all").clicked() {
                    let mut list: Vec<PlayItem> = items.iter().filter_map(|m| PlayItem::file(&m.path)).collect();
                    let mut seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(9) | 1;
                    for i in (1..list.len()).rev() {
                        seed ^= seed << 13;
                        seed ^= seed >> 7;
                        seed ^= seed << 17;
                        list.swap(i, (seed % (i as u64 + 1)) as usize);
                    }
                    action = Some(Box::new(move |a: &mut App| a.play_list(list, 0)));
                }
                let lbl = if photos { "Slideshow" } else { "Play all" };
                if ui.add(egui::Button::new(RichText::new(lbl).color(Color32::WHITE)).fill(ACCENT)).clicked() {
                    let list: Vec<PlayItem> = items.iter().filter_map(|m| PlayItem::file(&m.path)).collect();
                    action = Some(Box::new(move |a: &mut App| a.play_list(list, 0)));
                }
            }
        });
    });
    divider(ui);
    ui.add_space(8.0);
    if app.scan_rx.is_some() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("Looking through your folders…").color(TEXT_DIM));
        });
    } else if items.is_empty() {
        ui.add_space(20.0);
        ui.label(RichText::new("Nothing here yet").size(18.0).color(TEXT));
        let folders = app.settings.folders.iter().map(|f| f.display().to_string()).collect::<Vec<_>>().join(", ");
        ui.label(RichText::new(format!("The library looks in: {folders}. Use the + next to Media Library to add more.")).color(TEXT_DIM));
    }
    let mut clicked: Option<(usize, bool)> = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        if music {
            for (i, m) in items.iter().enumerate().take(5000) {
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width() - 12.0, 34.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(rect, 2.0, CARD_HOVER);
                }
                let playing = app.current.as_ref().is_some_and(|c| c.url == m.path.display().to_string());
                let col = if playing { ACCENT } else { TEXT };
                icons::paint(ui.painter(), if playing { Icon::Volume } else { Icon::Music }, Rect::from_center_size(pos2(rect.left() + 18.0, rect.center().y), vec2(16.0, 16.0)), col);
                ui.painter().text(pos2(rect.left() + 40.0, rect.center().y), Align2::LEFT_CENTER, truncate(&m.name, 50), FontId::proportional(14.0), col);
                ui.painter().text(pos2(rect.left() + rect.width() * 0.5, rect.center().y), Align2::LEFT_CENTER, truncate(&m.folder, 40), FontId::proportional(12.0), TEXT_DIM);
                ui.painter().text(pos2(rect.right() - 90.0, rect.center().y), Align2::RIGHT_CENTER, &m.ext, FontId::proportional(12.0), TEXT_DIM);
                ui.painter().text(pos2(rect.right() - 8.0, rect.center().y), Align2::RIGHT_CENTER, fmt_size(m.size), FontId::proportional(12.0), TEXT_DIM);
                if resp.clicked() {
                    clicked = Some((i, false));
                }
                resp.context_menu(|ui| {
                    if ui.button("Play").clicked() {
                        clicked = Some((i, false));
                        ui.close();
                    }
                    if ui.button("Add to playlist").clicked() {
                        clicked = Some((i, true));
                        ui.close();
                    }
                });
            }
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(60.0, 14.0);
                for (i, m) in items.iter().enumerate().take(3000) {
                    let icon = match m.kind {
                        MediaKind::Photo => Icon::Photo,
                        MediaKind::DiscFolder | MediaKind::DiscImage => Icon::Disc,
                        _ => Icon::Film,
                    };
                    let resp = folder_tile(ui, icon, &m.name, Some(&m.ext));
                    if resp.clicked() {
                        clicked = Some((i, false));
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Play").clicked() {
                            clicked = Some((i, false));
                            ui.close();
                        }
                        if ui.button("Add to playlist").clicked() {
                            clicked = Some((i, true));
                            ui.close();
                        }
                    });
                }
            });
        }
    });
    if let Some(a) = action {
        a(app);
    }
    if let Some((i, add)) = clicked {
        if add {
            if let Some(it) = PlayItem::file(&items[i].path) {
                app.enqueue(it);
            }
        } else if music || photos {
            let list: Vec<PlayItem> = items.iter().filter_map(|m| PlayItem::file(&m.path)).collect();
            let start = list.iter().position(|p| p.url == items[i].path.display().to_string()).unwrap_or(0);
            app.play_list(list, start);
        } else if let Some(it) = PlayItem::file(&items[i].path) {
            app.play_list(vec![it], 0);
        } else {
            app.notify("That folder or image doesn't look like a DVD or Blu-ray.", true);
        }
    }
}

// ---------- Playlist ----------

pub fn playlist_page(app: &mut App, ui: &mut Ui) {
    page_header(app, ui, "Playlists", false);
    ui.horizontal(|ui| {
        ui.set_height(26.0);
        ui.label(RichText::new(format!("{} items in the current playlist", app.playlist.len())).color(TEXT_DIM));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Clear").clicked() {
                app.playlist.clear();
                app.pl_index = None;
            }
            if ui.button("Add files…").clicked() {
                if let Some(files) = rfd::FileDialog::new().pick_files() {
                    for f in files {
                        if let Some(it) = PlayItem::file(&f) {
                            app.playlist.push(it);
                        }
                    }
                }
            }
            if ui.selectable_label(app.shuffle, "Shuffle").clicked() {
                app.shuffle = !app.shuffle;
            }
        });
    });
    divider(ui);
    ui.add_space(8.0);
    if app.playlist.is_empty() {
        ui.label(RichText::new("The playlist is empty. Right-click anything in your library or My Computer and choose \"Add to playlist\", or drop files onto the window.").color(TEXT_DIM));
        return;
    }
    let mut action: Option<(usize, i32)> = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, it) in app.playlist.iter().enumerate() {
            let cur = app.pl_index == Some(i) && app.current.is_some();
            ui.horizontal(|ui| {
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width() - 190.0, 32.0), Sense::click());
                let fill = if cur { Color32::from_rgb(20, 60, 110) } else if resp.hovered() { CARD_HOVER } else { Color32::TRANSPARENT };
                ui.painter().rect_filled(rect, 2.0, fill);
                ui.painter().text(pos2(rect.left() + 10.0, rect.center().y), Align2::LEFT_CENTER, format!("{}", i + 1), FontId::monospace(12.0), TEXT_DIM);
                ui.painter().text(pos2(rect.left() + 44.0, rect.center().y), Align2::LEFT_CENTER, truncate(&it.title, 70), FontId::proportional(14.0), TEXT);
                ui.painter().text(pos2(rect.right() - 10.0, rect.center().y), Align2::RIGHT_CENTER, it.badge(), FontId::proportional(12.0), TEXT_DIM);
                if resp.clicked() {
                    action = Some((i, 0));
                }
                if ui.small_button("Up").clicked() {
                    action = Some((i, -1));
                }
                if ui.small_button("Down").clicked() {
                    action = Some((i, 1));
                }
                if ui.small_button("Remove").clicked() {
                    action = Some((i, 9));
                }
            });
        }
    });
    if let Some((i, a)) = action {
        let swap = |app: &mut App, x: usize, y: usize| {
            app.playlist.swap(x, y);
            if app.pl_index == Some(x) {
                app.pl_index = Some(y);
            } else if app.pl_index == Some(y) {
                app.pl_index = Some(x);
            }
        };
        match a {
            0 => app.play_index(i),
            -1 if i > 0 => swap(app, i, i - 1),
            1 if i + 1 < app.playlist.len() => swap(app, i, i + 1),
            9 => {
                app.playlist.remove(i);
                if let Some(p) = app.pl_index {
                    if p > i {
                        app.pl_index = Some(p - 1);
                    } else if p == i {
                        app.pl_index = None;
                    }
                }
            }
            _ => {}
        }
    }
}

// ---------- Settings ----------

pub fn settings_page(app: &mut App, ui: &mut Ui) {
    page_header(app, ui, "Settings", false);
    ui.horizontal_wrapped(|ui| {
        for (t, l) in [
            (SettingsTab::Playback, "Playback"),
            (SettingsTab::Video, "Picture"),
            (SettingsTab::Audio, "Sound"),
            (SettingsTab::Subtitles, "Languages & subtitles"),
            (SettingsTab::Discs, "Discs & Blu-ray"),
            (SettingsTab::Engine, "Codec engine"),
            (SettingsTab::Library, "Library"),
            (SettingsTab::About, "About"),
        ] {
            if ui.selectable_label(app.settings_tab == t, RichText::new(l).size(14.0)).clicked() {
                app.settings_tab = t;
            }
        }
    });
    divider(ui);
    let before = serde_json::to_string(&app.settings).unwrap_or_default();
    let mut action: Option<Action> = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_max_width(780.0);
        ui.add_space(8.0);
        match app.settings_tab {
            SettingsTab::Playback => {
                let s = &mut app.settings;
                ui.checkbox(&mut s.resume_playback, "Resume movies and discs from where I stopped");
                ui.checkbox(&mut s.auto_play_disc, "Start playing a disc as soon as it is inserted");
                ui.checkbox(&mut s.disc_menus, "Start DVDs and Blu-rays at their own menus");
                ui.checkbox(&mut s.hardware_decoding, "Use the graphics card to decode video (faster, cooler, quieter)");
                ui.add_space(12.0);
                ui.label(RichText::new("Keyboard shortcuts").strong());
                for (k, v) in [
                    ("Space", "Play / pause"),
                    ("Left / Right", "Back 10 s / forward 30 s"),
                    ("Up / Down", "Volume"),
                    ("Page Up / Page Down", "Previous / next chapter"),
                    ("F or double-click", "Full screen (Esc to leave)"),
                    ("M", "Mute"),
                    ("S", "Snapshot of the current picture"),
                    ("I", "Media information"),
                    ("Home", "Back to the beginning"),
                    ("Ctrl+O", "Open files"),
                ] {
                    ui.horizontal(|ui| {
                        ui.add_sized(vec2(180.0, 18.0), egui::Label::new(RichText::new(k).monospace().color(ACCENT)));
                        ui.label(v);
                    });
                }
            }
            SettingsTab::Video => {
                let s = &mut app.settings;
                ui.label(RichText::new("Picture adjustments apply to everything you play.").color(TEXT_DIM));
                ui.add_space(6.0);
                ui.add(egui::Slider::new(&mut s.brightness, -50..=50).text("Brightness"));
                ui.add(egui::Slider::new(&mut s.contrast, -50..=50).text("Contrast"));
                ui.add(egui::Slider::new(&mut s.saturation, -50..=80).text("Colour"));
                ui.add(egui::Slider::new(&mut s.gamma, -50..=50).text("Gamma"));
                if ui.button("Reset picture").clicked() {
                    s.brightness = 0;
                    s.contrast = 0;
                    s.saturation = 0;
                    s.gamma = 0;
                }
                ui.add_space(12.0);
                ui.label(RichText::new("Picture boost").strong());
                ui.checkbox(&mut s.sharpen, "Sharpen (makes DVDs look crisper on HD screens)");
                ui.checkbox(&mut s.denoise, "Reduce noise and grain");
                ui.checkbox(&mut s.deinterlace, "Smooth interlaced video (TV recordings and many DVDs)");
            }
            SettingsTab::Audio => {
                let s = &mut app.settings;
                ui.label(RichText::new("Speakers").strong());
                let current = match s.audio_output.as_str() {
                    "stereo" => "Stereo (2 speakers or headphones)",
                    "5.1" => "5.1 surround",
                    "7.1" => "7.1 surround",
                    _ => "Automatic",
                };
                egui::ComboBox::from_id_salt("aout").selected_text(current).width(300.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut s.audio_output, "auto-safe".to_string(), "Automatic");
                    ui.selectable_value(&mut s.audio_output, "stereo".to_string(), "Stereo (2 speakers or headphones)");
                    ui.selectable_value(&mut s.audio_output, "5.1".to_string(), "5.1 surround");
                    ui.selectable_value(&mut s.audio_output, "7.1".to_string(), "7.1 surround");
                });
                ui.add_space(8.0);
                ui.checkbox(&mut s.hd_passthrough, "Send Dolby / DTS sound unchanged to my AV receiver (HDMI or optical)");
                ui.checkbox(&mut s.night_mode, "Night mode: even out loud explosions and quiet speech");
                ui.checkbox(&mut s.dialogue_boost, "Clearer dialogue");
                ui.add_space(8.0);
                let mut v = s.volume;
                if ui.add(egui::Slider::new(&mut v, 0.0..=150.0).text("Volume (%)")).changed() {
                    action = Some(Box::new(move |a: &mut App| a.set_volume(v)));
                }
            }
            SettingsTab::Subtitles => {
                let s = &mut app.settings;
                ui.label("Preferred audio languages (first match wins)");
                ui.add(egui::TextEdit::singleline(&mut s.audio_lang).hint_text("e.g. en,eng or fr,fre"));
                ui.add_space(4.0);
                ui.label("Preferred subtitle languages");
                ui.add(egui::TextEdit::singleline(&mut s.sub_lang).hint_text("e.g. en,eng"));
                ui.checkbox(&mut s.subtitles_on, "Turn subtitles on automatically");
                ui.add(egui::Slider::new(&mut s.sub_scale, 0.5..=2.5).text("Subtitle size"));
                ui.label(RichText::new("Subtitle files named like the video (.srt, .ass, .vtt, .sup) load automatically.").color(TEXT_DIM));
            }
            SettingsTab::Discs => {
                ui.label(RichText::new("DVD").strong());
                ui.label(RichText::new("DVDs play through libdvdnav. Protected (CSS) commercial DVDs also need libdvdcss, which most engine packages include or can add.").color(TEXT_DIM));
                ui.add_space(12.0);
                ui.label(RichText::new("Blu-ray and Ultra HD Blu-ray").strong());
                ui.label(RichText::new("Blu-ray discs play through libbluray. Most commercial discs are encrypted with AACS; open-source players need the libaacs library and a key file (KEYDB.cfg) that you provide. Unprotected discs, home-made discs and your own backup folders or ISOs play without it.").color(TEXT_DIM));
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Blu-ray codec information:").color(TEXT_DIM));
                    ui.hyperlink_to(crate::aacs::CODEC_INFO_URL, crate::aacs::CODEC_INFO_URL)
                        .on_hover_text("Opens in your web browser");
                });
                ui.add_space(6.0);
                let kp = library::keydb_path();
                let have = kp.as_ref().is_some_and(|p| p.exists());
                ui.horizontal(|ui| {
                    ui.label("Key file:");
                    ui.label(RichText::new(if have { "installed" } else { "not installed" }).color(if have { GOOD } else { WARN }));
                });
                if let Some(p) = &kp {
                    ui.label(RichText::new(format!("Location: {}", p.display())).color(TEXT_DIM).size(12.0));
                }
                if ui.button("Import key file (KEYDB.cfg)…").clicked() {
                    action = Some(Box::new(move |a: &mut App| {
                        if let (Some(src), Some(dst)) = (rfd::FileDialog::new().add_filter("Key file", &["cfg"]).pick_file(), kp) {
                            let _ = std::fs::create_dir_all(dst.parent().unwrap_or(Path::new(".")));
                            match std::fs::copy(&src, &dst) {
                                Ok(_) => a.notify("Key file imported.", false),
                                Err(e) => a.notify(format!("Couldn't copy the key file: {e}"), true),
                            }
                        }
                    }));
                }
                ui.add_space(6.0);
                match crate::aacs::info() {
                    Some(info) => {
                        ui.label(RichText::new(format!("● libaacs {} is installed (VideoLAN's open-source AACS library).", info.version)).color(GOOD));
                        ui.label(RichText::new(format!("Loaded from: {}", info.path)).color(TEXT_DIM).size(12.0));
                    }
                    None => {
                        ui.label(RichText::new("libaacs is not installed yet.").color(WARN));
                    }
                }
                ui.horizontal(|ui| {
                    let busy = app.aacs_rx.is_some();
                    let label = if crate::aacs::info().is_some() { "Update libaacs" } else { "Download libaacs" };
                    if ui.add_enabled(!busy, egui::Button::new(label)).on_hover_text("Fetches libaacs and libbdplus (no keys) and their helper libraries").clicked() {
                        let ctx = ui.ctx().clone();
                        action = Some(Box::new(move |a: &mut App| {
                            a.aacs_status = "Starting…".into();
                            a.aacs_rx = Some(library::install_aacs(ctx));
                        }));
                    }
                    if busy {
                        ui.spinner();
                    }
                    ui.hyperlink_to("About libaacs", crate::aacs::PROJECT_URL);
                    ui.hyperlink_to("Source code", crate::aacs::SOURCE_URL);
                    if let Some(info) = crate::aacs::info() {
                        let p = Path::new(&info.path);
                        if p.is_absolute() {
                            if let Some(dir) = p.parent().map(Path::to_path_buf) {
                                if ui.small_button("Open its folder").clicked() {
                                    open_path(&dir);
                                }
                            }
                        }
                    }
                });
                if !app.aacs_status.is_empty() {
                    ui.label(RichText::new(&app.aacs_status).color(ACCENT));
                }
                ui.label(RichText::new("libaacs, like this player, contains no keys or certificates.").color(TEXT_DIM).size(12.0));
                ui.label(RichText::new("The key file itself is not included: decryption keys are licensed material that can't be shipped with open-source software, so you add your own once with the button above.").color(TEXT_DIM).size(12.0));
                ui.add_space(12.0);
                ui.label(RichText::new("Licensed player for protected discs").strong());
                ui.label(RichText::new("If you own a licensed Blu-ray player program, protected discs can be handed to it with one click from the disc page.").color(TEXT_DIM));
                let mut lp = app.settings.licensed_player.clone();
                ui.horizontal(|ui| {
                    let shown = lp.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "Not chosen".into());
                    ui.label(RichText::new(shown).color(TEXT_DIM).size(12.0));
                    if ui.button("Choose…").clicked() {
                        if let Some(p) = rfd::FileDialog::new().set_title("Choose your licensed Blu-ray player program").pick_file() {
                            lp = Some(p);
                        }
                    }
                    if lp.is_some() && ui.button("Clear").clicked() {
                        lp = None;
                    }
                });
                app.settings.licensed_player = lp;
            }
            SettingsTab::Engine => engine_tab(app, ui, &mut action),
            SettingsTab::Library => {
                let s = &mut app.settings;
                ui.label("Folders included in your library:");
                let mut remove = None;
                for (i, f) in s.folders.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(f.display().to_string());
                        if ui.small_button("Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    s.folders.remove(i);
                }
                ui.horizontal(|ui| {
                    if ui.button("Add folder…").clicked() {
                        if let Some(f) = rfd::FileDialog::new().pick_folder() {
                            s.folders.push(f);
                        }
                    }
                    if ui.button("Scan now").clicked() {
                        action = Some(Box::new(|a: &mut App| a.scan_rx = Some(library::scan(a.settings.folders.clone()))));
                    }
                });
                ui.add_space(12.0);
                ui.label(format!("Snapshots are saved to: {}", s.snapshot_folder().display()));
                if ui.button("Change snapshot folder…").clicked() {
                    if let Some(f) = rfd::FileDialog::new().pick_folder() {
                        s.snapshot_dir = Some(f);
                    }
                }
                if ui.button("Forget recently played and resume points").clicked() {
                    s.recent.clear();
                    s.resume.clear();
                }
            }
            SettingsTab::About => {
                ui.label(RichText::new(format!("PlayCraft {}", env!("CARGO_PKG_VERSION"))).size(18.0).strong());
                ui.label("A free, open-source disc and media player for Windows, macOS and Linux.");
                ui.hyperlink_to("Source code on GitHub", env!("CARGO_PKG_REPOSITORY"));
                ui.add_space(8.0);
                ui.label(RichText::new("Activity log").strong());
                ui.label(RichText::new(format!("PlayCraft writes what it does (start-up, discs found, what played, disc menus, errors) to {}", crate::log::path().display())).color(TEXT_DIM).size(12.0));
                ui.horizontal(|ui| {
                    if ui.button("Open activity log").clicked() {
                        open_path(&crate::log::path());
                    }
                    if ui.button("Open its folder").clicked() {
                        open_path(&library::data_dir());
                    }
                });
                ui.add_space(6.0);
                ui.label(RichText::new("Built with Rust and egui. Playback by libmpv, FFmpeg, libdvdnav, libdvdread, libbluray and libcdio, all free, open-source projects. Licensed GPL-3.0-or-later.").color(TEXT_DIM));
                ui.add_space(6.0);
                ui.label(RichText::new("Disc decryption libraries (libdvdcss, libaacs, libbdplus) and key files are separate; check the laws where you live before using them.").color(TEXT_DIM));
            }
        }
    });
    if let Some(a) = action {
        a(app);
    }
    let after = serde_json::to_string(&app.settings).unwrap_or_default();
    if before != after {
        app.apply_settings();
        app.settings.save();
    }
}

fn engine_tab(app: &mut App, ui: &mut Ui, action: &mut Option<Action>) {
    match &app.engine {
        Ok(e) if app.player.is_some() => {
            ui.label(RichText::new("● The codec engine is installed and working").color(GOOD).size(16.0));
            ui.label(RichText::new(format!("Engine: libmpv (client API {})", e.api_version())).color(TEXT_DIM));
            ui.label(RichText::new(format!("Loaded from: {}", e.path)).color(TEXT_DIM).size(12.0));
        }
        Ok(_) => {
            ui.label(RichText::new("The engine was found but could not start.").color(WARN).size(16.0));
        }
        Err(e) => {
            ui.label(RichText::new("The codec engine is not installed yet").color(WARN).size(16.0));
            ui.label(RichText::new(format!("This PlayCraft is for {} processors; the engine must match.", Engine::processor_name())).color(TEXT_DIM));
            ui.label(RichText::new(format!("Details: {e}")).color(TEXT_DIM).size(12.0));
        }
    }
    ui.add_space(8.0);
    ui.label(library::engine_install_help());
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if cfg!(windows) {
            let busy = app.install_rx.is_some();
            let label = if app.player.is_some() { "Update codec engine" } else { "Install codec engine" };
            if ui.add_enabled(!busy, egui::Button::new(RichText::new(label).color(Color32::WHITE)).fill(ACCENT)).clicked() {
                let ctx = ui.ctx().clone();
                *action = Some(Box::new(move |a: &mut App| {
                    a.install_status = "Starting…".into();
                    // Release the running engine so its file can be replaced.
                    a.stop();
                    a.player = None;
                    a.engine = Err("updating".into());
                    a.install_rx = Some(library::install_engine(ctx));
                }));
            }
            if busy {
                ui.spinner();
            }
        }
        if ui.button("Check again").clicked() {
            let ctx = ui.ctx().clone();
            *action = Some(Box::new(move |a: &mut App| {
                a.player = None;
                a.engine = Engine::locate();
                a.start_player(&ctx);
            }));
        }
        if app.player.is_some() && ui.button("Play a test picture").clicked() {
            *action = Some(Box::new(|a: &mut App| a.play_test_pattern()));
        }
    });
    if !app.install_status.is_empty() {
        ui.label(RichText::new(&app.install_status).color(ACCENT));
    }
    ui.add_space(14.0);
    ui.label(RichText::new("What it plays").strong());
    for (k, v) in [
        ("Discs", "DVD-Video, audio CD, Blu-ray, Ultra HD Blu-ray, VIDEO_TS / BDMV folders, ISO images"),
        ("Video", "H.264, H.265/HEVC (incl. HDR10), AV1, VP9, MPEG-2, MPEG-4, VC-1, DivX/Xvid, WMV and more"),
        ("Audio", "Dolby Digital / Plus / TrueHD, DTS / DTS-HD, LPCM, AAC, FLAC, MP3, Opus, ALAC, WAV and more"),
        ("Files", "MKV, MP4, AVI, MOV, M2TS, TS, VOB, WebM, WMV, FLV, plus photos (JPG, PNG, HEIC, WebP…)"),
    ] {
        ui.horizontal_wrapped(|ui| {
            ui.add_sized(vec2(60.0, 18.0), egui::Label::new(RichText::new(k).color(ACCENT)));
            ui.label(RichText::new(v).color(TEXT_DIM));
        });
    }
    if !app.log.is_empty() {
        ui.add_space(12.0);
        egui::CollapsingHeader::new("Engine messages").show(ui, |ui| {
            for l in app.log.iter().rev().take(40) {
                ui.label(RichText::new(l).monospace().size(11.0).color(TEXT_DIM));
            }
        });
    }
}

fn open_path(p: &Path) {
    let cmd = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(cmd).arg(p).spawn();
}

fn hero_card(ui: &mut Ui, icon: Icon, title: &str, sub: &str, enabled: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(200.0, 120.0), Sense::click());
    let hovered = resp.hovered() && enabled;
    ui.painter().rect_filled(rect, 2.0, if hovered { Color32::from_rgb(52, 52, 52) } else { Color32::from_rgb(38, 38, 38) });
    if hovered {
        ui.painter().rect_stroke(rect, 2.0, Stroke::new(2.0, ACCENT), egui::StrokeKind::Inside);
    }
    let ic = Rect::from_min_size(rect.min + vec2(16.0, 16.0), vec2(34.0, 34.0));
    icons::paint(ui.painter(), icon, ic, if enabled { Color32::from_rgb(230, 230, 230) } else { TEXT_DIM });
    ui.painter().text(rect.min + vec2(16.0, 76.0), Align2::LEFT_CENTER, title, FontId::proportional(16.0), if enabled { TEXT } else { TEXT_DIM });
    ui.painter().text(rect.min + vec2(16.0, 98.0), Align2::LEFT_CENTER, truncate(sub, 28), FontId::proportional(12.0), TEXT_DIM);
    resp
}
