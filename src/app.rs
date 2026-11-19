use crate::discs::{self, DiscKind, Drive};
use crate::library::{self, fmt_time, InstallMsg, MediaItem, MediaKind, RecentItem, Settings};
use crate::mpv::{self, Chapter, Engine, Player, PlayerEvent, Track};
use eframe::egui::{self, pos2, vec2, Color32, Rect, TextureHandle, TextureOptions, Ui};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How many frames to draw before saving the --screenshot picture.
pub static SCREENSHOT_FRAMES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(120);

pub const ACCENT: Color32 = Color32::from_rgb(28, 140, 255);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(70, 165, 255);
pub const BG: Color32 = Color32::from_rgb(10, 10, 10);
pub const BG_BOTTOM: Color32 = Color32::from_rgb(34, 34, 34);
pub const SIDEBAR: Color32 = Color32::from_rgb(43, 43, 43);
pub const SIDEBAR_BOTTOM: Color32 = Color32::from_rgb(52, 52, 52);
pub const BAR: Color32 = Color32::from_rgb(33, 33, 33);
pub const CARD: Color32 = Color32::from_rgb(40, 40, 40);
pub const CARD_HOVER: Color32 = Color32::from_rgb(56, 56, 56);
pub const TEXT_DIM: Color32 = Color32::from_rgb(150, 150, 150);
pub const TEXT: Color32 = Color32::from_rgb(235, 235, 235);
pub const WARN: Color32 = Color32::from_rgb(255, 184, 76);
pub const GOOD: Color32 = Color32::from_rgb(90, 210, 140);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Home,
    NowPlaying,
    Disc(usize),
    Computer,
    Movies,
    TvShows,
    Videos,
    Music,
    Photos,
    Playlist,
    Settings,
}

#[derive(Clone, Debug)]
pub struct BrowseEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub kind: Option<MediaKind>,
    pub disc: Option<DiscKind>,
    pub modified: std::time::SystemTime,
}

pub fn read_folder(path: &Path) -> Vec<BrowseEntry> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(path) else { return out };
    for e in rd.flatten().take(5000) {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name.starts_with('$') || name.eq_ignore_ascii_case("System Volume Information") {
            continue;
        }
        let p = e.path();
        let meta = e.metadata().ok();
        let modified = meta.as_ref().and_then(|m| m.modified().ok()).unwrap_or(std::time::UNIX_EPOCH);
        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        if is_dir {
            let upper = name.to_uppercase();
            let disc = if upper == "VIDEO_TS" {
                Some(DiscKind::DvdVideo)
            } else if upper == "BDMV" {
                Some(DiscKind::BluRay)
            } else if p.join("VIDEO_TS").is_dir() {
                Some(DiscKind::DvdVideo)
            } else if p.join("BDMV").is_dir() {
                Some(DiscKind::BluRay)
            } else {
                None
            };
            out.push(BrowseEntry { name, path: p, is_dir: true, kind: None, disc, modified });
        } else if let Some(kind) = library::kind_of(&p) {
            out.push(BrowseEntry { name, path: p, is_dir: false, kind: Some(kind), disc: None, modified });
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Playback,
    Video,
    Audio,
    Subtitles,
    Discs,
    Engine,
    Library,
    About,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Repeat {
    Off,
    One,
    All,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct PlayItem {
    pub title: String,
    pub url: String,
    pub kind: MediaKind,
    pub device_opt: String,
    pub device: String,
    pub disc: Option<DiscKind>,
}

impl PlayItem {
    pub fn file(path: &Path) -> Option<PlayItem> {
        let kind = library::kind_of(path).unwrap_or(MediaKind::Video);
        if path.is_dir() || kind == MediaKind::DiscImage {
            return PlayItem::disc_source(path);
        }
        Some(PlayItem {
            title: library::display_name(path),
            url: path.display().to_string(),
            kind,
            device_opt: String::new(),
            device: String::new(),
            disc: None,
        })
    }

    /// A disc folder (VIDEO_TS / BDMV) or a disc image (ISO).
    pub fn disc_source(path: &Path) -> Option<PlayItem> {
        let (kind, device) = if path.is_dir() {
            let root = discs::disc_root(path);
            let k = discs::classify_folder(path);
            let k = if matches!(k, DiscKind::Data | DiscKind::Unknown) { discs::classify_folder(&root) } else { k };
            (k, root.display().to_string())
        } else {
            (iso_kind(path), path.display().to_string())
        };
        if !matches!(kind, DiscKind::DvdVideo | DiscKind::BluRay | DiscKind::UhdBluRay | DiscKind::AudioCd) {
            return None;
        }
        let (url, opt, dev) = discs::engine_url(kind, &device, None);
        Some(PlayItem {
            title: discs::source_name(Path::new(&device)),
            url,
            kind: MediaKind::DiscFolder,
            device_opt: opt.to_string(),
            device: dev,
            disc: Some(kind),
        })
    }

    pub fn drive(d: &Drive) -> PlayItem {
        let (url, opt, dev) = discs::engine_url(d.kind, &d.device, None);
        PlayItem {
            title: d.name(),
            url,
            kind: MediaKind::DiscFolder,
            device_opt: opt.to_string(),
            device: dev,
            disc: Some(if d.kind == DiscKind::Unknown { DiscKind::DvdVideo } else { d.kind }),
        }
    }

    pub fn key(&self) -> String {
        format!("{}|{}", self.url, self.device)
    }

    pub fn badge(&self) -> String {
        match self.disc {
            Some(k) => k.label().to_string(),
            None => Path::new(&self.url).extension().map(|e| e.to_string_lossy().to_uppercase()).unwrap_or_default(),
        }
    }
}

/// Look inside the first part of an ISO image for the folder names that mark a DVD or a Blu-ray.
fn iso_kind(path: &Path) -> DiscKind {
    use std::io::Read;
    let mut buf = vec![0u8; 8 << 20];
    let n = std::fs::File::open(path).and_then(|mut f| f.read(&mut buf)).unwrap_or(0);
    let hay = &buf[..n];
    let has = |needle: &[u8]| hay.windows(needle.len()).any(|w| w == needle);
    if has(b"BDMV") {
        DiscKind::BluRay
    } else if has(b"VIDEO_TS") {
        DiscKind::DvdVideo
    } else if std::fs::metadata(path).map(|m| m.len()).unwrap_or(0) > 9_000_000_000 {
        DiscKind::BluRay
    } else {
        DiscKind::DvdVideo
    }
}

#[derive(Default, Clone)]
pub struct MediaInfo {
    pub video: String,
    pub audio: String,
    pub hw: String,
    pub format: String,
}

pub struct ScreenshotJob {
    pub path: PathBuf,
    pub frames: u32,
    pub requested: bool,
    pub done: Arc<AtomicBool>,
}

pub struct App {
    pub settings: Settings,
    pub engine: Result<Engine, String>,
    pub player: Option<Player>,
    pub page: Page,
    pub settings_tab: SettingsTab,
    pub video_tex: Option<TextureHandle>,
    pub logo_tex: Option<TextureHandle>,
    pub nav: Option<Arc<crate::nav::NavSession>>,
    pub nav_fallback: Option<PlayItem>,
    pub skip_menus_next: bool,
    pub nav_tex: Option<TextureHandle>,
    pub nav_gen: u64,
    pub nav_frame: (f32, f32),
    pub nav_fraction: f64,
    pub nav_video_rect: Option<Rect>,
    pub remote_open: bool,
    pub remote_floating: bool,
    pub remote_anchor: Option<Rect>,
    pub nav_last_mouse: Option<egui::Pos2>,
    pub aacs_rx: Option<Receiver<InstallMsg>>,
    pub aacs_status: String,
    pub has_video: bool,
    pub current: Option<PlayItem>,
    pub time: f64,
    pub duration: f64,
    pub paused: bool,
    pub muted: bool,
    pub speed: f64,
    pub chapter: i64,
    pub chapters: Vec<Chapter>,
    pub tracks: Vec<Track>,
    pub disc_titles: i64,
    pub disc_title: i64,
    pub info: MediaInfo,
    pub playlist: Vec<PlayItem>,
    pub pl_index: Option<usize>,
    pub repeat: Repeat,
    pub shuffle: bool,
    pub ab: (Option<f64>, Option<f64>),
    pub drives: Vec<Drive>,
    drive_rx: Receiver<(Vec<Drive>, Vec<discs::Volume>)>,
    known_discs: Vec<String>,
    pub volumes: Vec<discs::Volume>,
    pub browse: Option<PathBuf>,
    pub browse_entries: Vec<BrowseEntry>,
    pub browse_loaded: Option<PathBuf>,
    pub sort_by_date: bool,
    pub url_input: Option<String>,
    pub library: Vec<MediaItem>,
    pub scan_rx: Option<Receiver<Vec<MediaItem>>>,
    pub search: String,
    pub toast: Option<(String, Instant, bool)>,
    pub log: VecDeque<String>,
    pub install_rx: Option<Receiver<InstallMsg>>,
    pub install_status: String,
    pub fullscreen: bool,
    pub last_activity: Instant,
    pub last_track_poll: Instant,
    pending_resume: Option<f64>,
    pub seek_preview: Option<f64>,
    pub show_info: bool,
    screenshot: Option<ScreenshotJob>,
    pub frame_no: u64,
    startup_files: Vec<PathBuf>,
    pub demo: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>, files: Vec<PathBuf>, demo: bool) -> App {
        crate::ui::setup_style(&cc.egui_ctx);
        let settings = Settings::load();
        // Let the Blu-ray library find helper libraries placed next to the engine.
        // The bundled engine folder next to the program comes first.
        // A libaacs the person downloaded from Settings (user folder) wins over the bundled one.
        let mut extra: Vec<PathBuf> = vec![library::engine_dir()];
        if let Some(d) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("engine"))) {
            extra.push(d);
        }
        extra.retain(|d| d.is_dir());
        // Point libbluray at the decryption libraries.
        let ext = if cfg!(windows) { "dll" } else if cfg!(target_os = "macos") { "dylib" } else { "so" };
        for (var, name) in [("LIBAACS_PATH", "libaacs"), ("LIBBDPLUS_PATH", "libbdplus")] {
            if std::env::var_os(var).is_none() {
                if let Some(d) = extra.iter().find(|d| d.join(format!("{name}.{ext}")).exists()) {
                    std::env::set_var(var, d.join(name));
                }
            }
        }
        if !extra.is_empty() {
            let path = std::env::var_os("PATH").unwrap_or_default();
            let mut paths: Vec<PathBuf> = std::env::split_paths(&path).collect();
            for (i, d) in extra.into_iter().enumerate() {
                paths.insert(i, d);
            }
            if let Ok(joined) = std::env::join_paths(paths) {
                std::env::set_var("PATH", joined);
            }
        }
        let engine = Engine::locate();
        let drive_rx = spawn_drive_watcher(cc.egui_ctx.clone());
        let scan_rx = Some(library::scan(settings.folders.clone()));
        let mut app = App {
            engine,
            player: None,
            page: Page::Computer,
            settings_tab: SettingsTab::Playback,
            video_tex: None,
            nav: None,
            nav_fallback: None,
            skip_menus_next: false,
            nav_tex: None,
            nav_gen: 0,
            nav_frame: (0.0, 0.0),
            nav_fraction: 0.0,
            nav_video_rect: None,
            remote_open: false,
            remote_floating: false,
            remote_anchor: None,
            nav_last_mouse: None,
            aacs_rx: None,
            aacs_status: String::new(),
            logo_tex: crate::icon_mask::icon_rgba(crate::ICON_BYTES, 96).map(|img| {
                let size = [img.width() as usize, img.height() as usize];
                let ci = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                cc.egui_ctx.load_texture("logo", ci, TextureOptions::LINEAR)
            }),
            has_video: false,
            current: None,
            time: 0.0,
            duration: 0.0,
            paused: false,
            muted: false,
            speed: 1.0,
            chapter: -1,
            chapters: Vec::new(),
            tracks: Vec::new(),
            disc_titles: 0,
            disc_title: -1,
            info: MediaInfo::default(),
            playlist: Vec::new(),
            pl_index: None,
            repeat: Repeat::Off,
            shuffle: false,
            ab: (None, None),
            drives: Vec::new(),
            drive_rx,
            known_discs: Vec::new(),
            volumes: Vec::new(),
            browse: None,
            browse_entries: Vec::new(),
            browse_loaded: None,
            sort_by_date: false,
            url_input: None,
            library: Vec::new(),
            scan_rx,
            search: String::new(),
            toast: None,
            log: VecDeque::new(),
            install_rx: None,
            install_status: String::new(),
            fullscreen: false,
            last_activity: Instant::now(),
            last_track_poll: Instant::now(),
            pending_resume: None,
            seek_preview: None,
            show_info: false,
            screenshot: screenshot.map(|path| ScreenshotJob { path, frames: 0, requested: false, done: Arc::new(AtomicBool::new(false)) }),
            frame_no: 0,
            startup_files: files,
            demo,
            settings,
        };
        app.start_player(&cc.egui_ctx);
        app
    }

    pub fn start_player(&mut self, ctx: &egui::Context) {
        if self.player.is_some() {
            return;
        }
        let Ok(engine) = &self.engine else { return };
        let s = &self.settings;
        let opts: Vec<(&str, String)> = vec![
            ("hwdec", if s.hardware_decoding { "auto-copy".into() } else { "no".into() }),
            ("volume", format!("{}", s.volume)),
            ("alang", s.audio_lang.clone()),
            ("slang", s.sub_lang.clone()),
            ("sid", if s.subtitles_on { "auto".into() } else { "no".into() }),
            ("screenshot-directory", s.snapshot_folder().display().to_string()),
        ];
        match Player::new(engine, ctx, &opts) {
            Ok(p) => {
                self.player = Some(p);
                self.apply_settings();
            }
            Err(e) => self.engine = Err(e),
        }
    }

    pub fn apply_settings(&self) {
        let Some(p) = &self.player else { return };
        let s = &self.settings;
        p.set("hwdec", if s.hardware_decoding { "auto-copy" } else { "no" });
        p.set("deinterlace", if s.deinterlace { "yes" } else { "no" });
        p.set("sub-scale", &format!("{:.2}", s.sub_scale));
        p.set("alang", &s.audio_lang);
        p.set("slang", &s.sub_lang);
        p.set("audio-channels", &s.audio_output);
        p.set("audio-spdif", if s.hd_passthrough { "ac3,eac3,dts,dts-hd,truehd" } else { "" });
        p.set("screenshot-directory", &s.snapshot_folder().display().to_string());
        let mut vf = Vec::new();
        if s.brightness != 0 || s.contrast != 0 || s.saturation != 0 || s.gamma != 0 {
            vf.push(format!(
                "eq=brightness={:.3}:contrast={:.3}:saturation={:.3}:gamma={:.3}",
                s.brightness as f64 / 100.0 * 0.5,
                1.0 + s.contrast as f64 / 100.0,
                1.0 + s.saturation as f64 / 100.0,
                1.0 + s.gamma as f64 / 100.0
            ));
        }
        if s.denoise {
            vf.push("hqdn3d=2:1.5:3:3".into());
        }
        if s.sharpen {
            vf.push("unsharp=5:5:0.7:3:3:0".into());
        }
        p.set("vf", &if vf.is_empty() { String::new() } else { format!("lavfi=[{}]", vf.join(",")) });
        let mut af = Vec::new();
        if s.dialogue_boost {
            af.push("equalizer=f=2200:t=q:w=1.2:g=5");
        }
        if s.night_mode {
            af.push("dynaudnorm=f=200:g=11:m=8");
        }
        p.set("af", &if af.is_empty() { String::new() } else { format!("lavfi=[{}]", af.join(",")) });
    }

    // ---------- playback ----------

    pub fn notify(&mut self, msg: impl Into<String>, warn: bool) {
        self.toast = Some((msg.into(), Instant::now(), warn));
    }

    pub fn remember_position(&mut self) {
        if let Some(cur) = &self.current {
            if self.duration > 60.0 && self.time > 15.0 && self.time < self.duration - 30.0 {
                self.settings.resume.insert(cur.key(), self.time);
            } else {
                self.settings.resume.remove(&cur.key());
            }
            if self.settings.resume.len() > 300 {
                if let Some(k) = self.settings.resume.keys().next().cloned() {
                    self.settings.resume.remove(&k);
                }
            }
        }
    }

    pub fn play(&mut self, item: PlayItem) {
        if self.player.is_none() {
            self.settings_tab = SettingsTab::Engine;
            self.page = Page::Settings;
            self.notify("The playback engine isn't installed yet. Install it below, then press Play again.", true);
            return;
        }
        self.remember_position();
        // Disc menus: drive the disc with its own navigator when possible.
        self.nav = None;
        crate::nav::set_current(None);
        let skip_menus = std::mem::take(&mut self.skip_menus_next);
        let nav_kind = match item.disc {
            Some(DiscKind::DvdVideo) => Some(crate::nav::NavKind::Dvd),
            Some(DiscKind::BluRay | DiscKind::UhdBluRay) => Some(crate::nav::NavKind::Bluray),
            _ => None,
        };
        let supports = self.player.as_ref().is_some_and(|p| p.supports_nav());
        if nav_kind.is_some() {
            eprintln!("[menus] kind={nav_kind:?} enabled={} skip={skip_menus} engine_support={supports} device={}", self.settings.disc_menus, item.device);
        }
        if let (Some(kind), true, false, true) = (nav_kind, self.settings.disc_menus, skip_menus, supports) {
            let lang: String = self.settings.audio_lang.chars().take(2).collect();
            match crate::nav::NavSession::open(kind, &item.device, &lang) {
                Ok(s) => {
                    s.set_show_subs(self.settings.subtitles_on);
                    crate::nav::set_current(Some(s.clone()));
                    self.nav = Some(s);
                    self.nav_fallback = Some(item.clone());
                }
                Err(e) => {
                    eprintln!("[menus] could not start: {e}");
                    self.log.push_back(format!("[menus] {e}"));
                }
            }
        }
        let p = self.player.as_ref().unwrap();
        set_nav_mode(p, self.nav.as_ref().map(|n| n.kind), self.settings.subtitles_on);
        let url = if self.nav.is_some() { "discnav://disc".to_string() } else { item.url.clone() };
        if self.nav.is_none() && !item.device_opt.is_empty() {
            p.set(&item.device_opt, &item.device);
        }
        if let Err(e) = p.command(&["loadfile", &url, "replace"]) {
            self.notify(format!("Couldn't open {}: {e}", item.title), true);
            return;
        }
        p.set("pause", "no");
        p.set("ab-loop-a", "no");
        p.set("ab-loop-b", "no");
        self.ab = (None, None);
        self.pending_resume = if self.settings.resume_playback { self.settings.resume.get(&item.key()).copied() } else { None };
        self.tracks.clear();
        self.chapters.clear();
        self.disc_titles = 0;
        self.time = 0.0;
        self.duration = 0.0;
        self.has_video = false;
        if !item.url.starts_with("av://") {
            self.settings.add_recent(RecentItem {
                title: item.title.clone(),
                url: item.url.clone(),
                kind: item.badge(),
                device_opt: item.device_opt.clone(),
                device: item.device.clone(),
            });
            self.settings.save();
        }
        self.current = Some(item);
        self.page = Page::NowPlaying;
    }

    pub fn play_list(&mut self, items: Vec<PlayItem>, start: usize) {
        if items.is_empty() {
            return;
        }
        self.playlist = items;
        let i = start.min(self.playlist.len() - 1);
        self.pl_index = Some(i);
        self.play(self.playlist[i].clone());
    }

    pub fn enqueue(&mut self, item: PlayItem) {
        self.notify(format!("Added \"{}\" to the playlist", item.title), false);
        self.playlist.push(item);
        if self.current.is_none() {
            let i = self.playlist.len() - 1;
            self.play_index(i);
        }
    }

    pub fn play_index(&mut self, i: usize) {
        if i < self.playlist.len() {
            self.pl_index = Some(i);
            self.play(self.playlist[i].clone());
        }
    }

    pub fn next_item(&mut self, user: bool) {
        if self.playlist.is_empty() {
            if !user {
                self.stop();
            }
            return;
        }
        let n = self.playlist.len();
        let cur = self.pl_index.unwrap_or(0);
        let next = if self.shuffle && n > 1 {
            let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(7) as usize;
            let mut x = seed % n;
            if x == cur {
                x = (x + 1) % n;
            }
            Some(x)
        } else if cur + 1 < n {
            Some(cur + 1)
        } else if self.repeat == Repeat::All || user {
            Some(0)
        } else {
            None
        };
        match next {
            Some(i) => self.play_index(i),
            None => self.stop(),
        }
    }

    pub fn prev_item(&mut self) {
        if let Some(cur) = self.pl_index {
            if cur > 0 && self.time < 5.0 {
                self.play_index(cur - 1);
                return;
            }
        }
        self.seek_abs(0.0);
    }

    pub fn stop(&mut self) {
        self.remember_position();
        self.settings.save();
        if let Some(p) = &self.player {
            let _ = p.command(&["stop"]);
        }
        self.nav = None;
        crate::nav::set_current(None);
        self.nav_tex = None;
        self.current = None;
        self.has_video = false;
        self.time = 0.0;
        self.duration = 0.0;
        self.tracks.clear();
        self.chapters.clear();
        if self.page == Page::NowPlaying {
            self.page = Page::Home;
        }
    }

    pub fn toggle_pause(&mut self) {
        if self.current.is_none() {
            if let Some(i) = self.pl_index.or((!self.playlist.is_empty()).then_some(0)) {
                self.play_index(i);
            } else if let Some(d) = self.drives.iter().find(|d| d.kind.playable()).cloned() {
                self.play(PlayItem::drive(&d));
            }
            return;
        }
        if let Some(p) = &self.player {
            let _ = p.command(&["cycle", "pause"]);
        }
    }

    pub fn seek_abs(&self, t: f64) {
        if let Some(n) = &self.nav {
            if self.duration > 0.0 {
                n.input(crate::nav::NavInput::SeekFraction(t / self.duration));
            }
            return;
        }
        if let Some(p) = &self.player {
            let _ = p.command(&["seek", &format!("{t:.3}"), "absolute"]);
        }
    }

    pub fn seek_rel(&self, dt: f64) {
        if let Some(n) = &self.nav {
            n.input(crate::nav::NavInput::Skip(dt));
            return;
        }
        if let Some(p) = &self.player {
            let _ = p.command(&["seek", &format!("{dt:.1}"), "relative"]);
        }
    }

    pub fn chapter_step(&mut self, d: i64) {
        if let Some(n) = &self.nav {
            n.input(crate::nav::NavInput::Chapter(d as i32));
            return;
        }
        if self.chapters.len() > 1 {
            if let Some(p) = &self.player {
                let _ = p.command(&["add", "chapter", &d.to_string()]);
            }
        } else if d > 0 {
            self.next_item(true);
        } else {
            self.prev_item();
        }
    }

    pub fn set_volume(&mut self, v: f64) {
        self.settings.volume = v.clamp(0.0, 150.0);
        if let Some(p) = &self.player {
            p.set("volume", &format!("{:.0}", self.settings.volume));
            if self.muted {
                p.set("mute", "no");
            }
        }
    }

    pub fn select_track(&mut self, kind: &str, id: Option<i64>) {
        let prop = match kind {
            "audio" => "aid",
            "sub" => "sid",
            _ => "vid",
        };
        if let Some(p) = &self.player {
            p.set(prop, &id.map(|i| i.to_string()).unwrap_or_else(|| "no".into()));
        }
        self.last_track_poll = Instant::now() - Duration::from_secs(5);
    }

    pub fn snapshot(&mut self) {
        if !self.has_video {
            return;
        }
        let folder = self.settings.snapshot_folder();
        let _ = std::fs::create_dir_all(&folder);
        if let Some(p) = &self.player {
            match p.command(&["screenshot", "video"]) {
                Ok(()) => self.notify(format!("Snapshot saved to {}", folder.display()), false),
                Err(e) => self.notify(format!("Snapshot failed: {e}"), true),
            }
        }
    }

    pub fn ab_repeat(&mut self) {
        let Some(p) = &self.player else { return };
        match self.ab {
            (None, _) => {
                self.ab = (Some(self.time), None);
                p.set("ab-loop-a", &format!("{:.3}", self.time));
                self.notify("Repeat start set (A). Press again to set the end (B).", false);
            }
            (Some(a), None) => {
                if self.time > a + 0.5 {
                    self.ab = (Some(a), Some(self.time));
                    p.set("ab-loop-b", &format!("{:.3}", self.time));
                    self.notify("Repeating the section from A to B.", false);
                }
            }
            _ => {
                self.ab = (None, None);
                p.set("ab-loop-a", "no");
                p.set("ab-loop-b", "no");
                self.notify("Section repeat off.", false);
            }
        }
    }

    pub fn set_fullscreen(&mut self, ctx: &egui::Context, on: bool) {
        self.fullscreen = on;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(on));
        if on {
            self.page = Page::NowPlaying;
        }
    }

    pub fn current_drive(&self) -> Option<Drive> {
        let cur = self.current.as_ref()?;
        self.drives.iter().find(|d| cur.device.trim_end_matches('\\') == d.device).cloned()
    }

    // ---------- open dialogs ----------

    pub fn open_files(&mut self) {
        let mut exts: Vec<&str> = library::VIDEO_EXT.to_vec();
        exts.extend_from_slice(library::AUDIO_EXT);
        exts.extend_from_slice(library::PHOTO_EXT);
        exts.push("iso");
        if let Some(files) = rfd::FileDialog::new()
            .set_title("Open media files")
            .add_filter("All media", &exts)
            .add_filter("All files", &["*"])
            .pick_files()
        {
            let items: Vec<PlayItem> = files.iter().filter_map(|f| PlayItem::file(f)).collect();
            self.play_list(items, 0);
        }
    }

    pub fn open_disc_folder(&mut self) {
        if let Some(dir) = rfd::FileDialog::new().set_title("Choose a DVD or Blu-ray folder (the one holding VIDEO_TS or BDMV)").pick_folder() {
            match PlayItem::disc_source(&dir) {
                Some(item) => self.play_list(vec![item], 0),
                None => self.notify("That folder doesn't hold a VIDEO_TS (DVD) or BDMV (Blu-ray) folder.", true),
            }
        }
    }

    pub fn open_iso(&mut self) {
        if let Some(f) = rfd::FileDialog::new().set_title("Open a disc image").add_filter("Disc image", &["iso"]).pick_file() {
            if let Some(item) = PlayItem::disc_source(&f) {
                self.play_list(vec![item], 0);
            }
        }
    }

    pub fn load_subtitle(&mut self) {
        if let Some(f) = rfd::FileDialog::new()
            .set_title("Load a subtitle file")
            .add_filter("Subtitles", &["srt", "ass", "ssa", "vtt", "sub", "idx", "sup", "smi", "txt"])
            .pick_file()
        {
            if let Some(p) = &self.player {
                let r = p.command(&["sub-add", &f.display().to_string(), "select"]);
                match r {
                    Ok(()) => self.notify("Subtitle file loaded.", false),
                    Err(e) => self.notify(format!("Couldn't load subtitles: {e}"), true),
                }
                self.last_track_poll = Instant::now() - Duration::from_secs(5);
            }
        }
    }

    /// Hand a copy-protected disc to a licensed Blu-ray player the person already owns.
    /// Asks for the program the first time.
    pub fn open_in_licensed_player(&mut self, drive: &Drive) {
        let exe = match self.settings.licensed_player.clone().filter(|p| p.exists()) {
            Some(p) => p,
            None => {
                let mut dlg = rfd::FileDialog::new().set_title("Choose the licensed Blu-ray player program you own");
                if cfg!(windows) {
                    dlg = dlg.add_filter("Programs", &["exe"]);
                }
                let Some(p) = dlg.pick_file() else { return };
                self.settings.licensed_player = Some(p.clone());
                self.settings.save();
                p
            }
        };
        let target = drive.mount.as_ref().map(|m| m.display().to_string()).unwrap_or_else(|| drive.device.clone());
        if self.current_drive().is_some_and(|c| c.device == drive.device) {
            self.stop();
        }
        let result = if cfg!(target_os = "macos") && exe.extension().is_some_and(|e| e == "app") {
            std::process::Command::new("open").arg("-a").arg(&exe).arg(&target).spawn()
        } else {
            std::process::Command::new(&exe).arg(&target).spawn()
        };
        match result {
            Ok(_) => self.notify(format!("Opened {} in your licensed player.", drive.name()), false),
            Err(e) => self.notify(format!("Couldn't start that player: {e}"), true),
        }
    }

    pub fn open_url(&mut self, url: &str) {
        let url = url.trim();
        if url.is_empty() {
            return;
        }
        let title = url.rsplit('/').find(|s| !s.is_empty()).unwrap_or(url).to_string();
        self.play_list(
            vec![PlayItem { title, url: url.to_string(), kind: MediaKind::Video, device_opt: String::new(), device: String::new(), disc: None }],
            0,
        );
    }

    /// Open a folder in the My Computer browser.
    pub fn browse_to(&mut self, path: Option<PathBuf>) {
        self.browse = path;
        self.browse_loaded = None;
        self.page = Page::Computer;
    }

    pub fn play_test_pattern(&mut self) {
        self.play_list(
            vec![PlayItem {
                title: "Engine test picture".into(),
                url: "av://lavfi:testsrc2=size=1280x720:rate=30".into(),
                kind: MediaKind::Video,
                device_opt: String::new(),
                device: String::new(),
                disc: None,
            }],
            0,
        );
    }

    // ---------- per-frame updates ----------

    pub fn pump(&mut self, ctx: &egui::Context) {
        while let Ok((d, vols)) = self.drive_rx.try_recv() {
            self.volumes = vols;
            let now: Vec<String> = d.iter().filter(|x| x.kind.playable()).map(|x| format!("{}{}", x.device, x.label)).collect();
            for (i, drive) in d.iter().enumerate() {
                let key = format!("{}{}", drive.device, drive.label);
                if drive.kind.playable() && !self.known_discs.contains(&key) && self.frame_no > 30 {
                    self.notify(format!("{} inserted: {}", drive.kind.label(), drive.name()), false);
                    if self.settings.auto_play_disc && self.current.is_none() {
                        let item = PlayItem::drive(drive);
                        self.play_list(vec![item], 0);
                    } else if self.current.is_none() && self.screenshot.is_none() {
                        self.page = Page::Disc(i);
                    }
                }
            }
            self.known_discs = now;
            self.drives = d;
        }
        if let Some(rx) = &self.scan_rx {
            if let Ok(items) = rx.try_recv() {
                self.library = items;
                self.scan_rx = None;
            }
        }
        if let Some(rx) = &self.aacs_rx {
            let mut done = None;
            while let Ok(m) = rx.try_recv() {
                match m {
                    InstallMsg::Progress(s) => self.aacs_status = s,
                    InstallMsg::Done(r) => done = Some(r),
                }
            }
            if let Some(r) = done {
                self.aacs_rx = None;
                self.aacs_status = match r {
                    Ok(s) => s,
                    Err(e) => e,
                };
            }
        }
        if let Some(rx) = &self.install_rx {
            let mut done = None;
            while let Ok(m) = rx.try_recv() {
                match m {
                    InstallMsg::Progress(s) => self.install_status = s,
                    InstallMsg::Done(r) => done = Some(r),
                }
            }
            if let Some(r) = done {
                self.install_rx = None;
                match r {
                    Ok(_) => {
                        self.engine = Engine::locate();
                        self.player = None;
                        self.start_player(ctx);
                        if self.player.is_some() {
                            self.install_status = "Engine installed. Everything is ready to play.".into();
                            self.notify("Codec engine installed.", false);
                        } else {
                            self.install_status = "The engine was downloaded but could not start.".into();
                        }
                    }
                    Err(e) => {
                        self.install_status = format!("Install failed: {e}");
                        self.engine = Engine::locate();
                        self.start_player(ctx);
                    }
                }
            }
        }

        if !self.startup_files.is_empty() && self.player.is_some() {
            let files = std::mem::take(&mut self.startup_files);
            let items: Vec<PlayItem> = files.iter().filter_map(|f| PlayItem::file(f)).collect();
            self.play_list(items, 0);
        }
        if self.demo && self.player.is_some() {
            self.demo = false;
            self.play_test_pattern();
        }

        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if !dropped.is_empty() {
            let items: Vec<PlayItem> = dropped.iter().filter_map(|f| PlayItem::file(f)).collect();
            self.play_list(items, 0);
        }

        // Disc menus: fall back to plain playback if the navigator can't handle the disc,
        // and restart the engine's stream after menu jumps so they show at once.
        if let Some(nav) = self.nav.clone() {
            let err = nav.shared.error.lock().unwrap().clone();
            let stalled = nav.shared.bytes.load(std::sync::atomic::Ordering::Relaxed) == 0 && nav.started.elapsed() > Duration::from_secs(20);
            if err.is_some() || stalled {
                let why = err.unwrap_or_else(|| "the disc's menus didn't start".into());
                eprintln!("[menus] falling back: {why}");
                if let Some(item) = self.nav_fallback.take() {
                    self.notify(format!("Disc menus aren't available for this disc ({why}). Playing the main movie instead."), true);
                    self.skip_menus_next = true;
                    self.play(item);
                    return;
                }
            } else if nav.shared.restart.swap(false, std::sync::atomic::Ordering::AcqRel) {
                if let Some(p) = &self.player {
                    let _ = p.command(&["loadfile", "discnav://disc", "replace"]);
                    p.set("pause", "no");
                }
            }
        }

        let Some(p) = &self.player else { return };
        let events = p.poll_events();
        let mut loaded = false;
        let mut ended = None;
        for e in events {
            match e {
                PlayerEvent::FileLoaded => loaded = true,
                PlayerEvent::EndFile { reason, error } => ended = Some((reason, error)),
                PlayerEvent::Log(l) => {
                    self.log.push_back(l);
                    if self.log.len() > 300 {
                        self.log.pop_front();
                    }
                }
            }
        }
        self.time = p.get_f64("time-pos").unwrap_or(0.0);
        self.duration = p.get_f64("duration").unwrap_or(0.0);
        self.paused = p.get_flag("pause").unwrap_or(false);
        self.muted = p.get_flag("mute").unwrap_or(false);
        self.speed = p.get_f64("speed").unwrap_or(1.0);
        self.chapter = p.get_i64("chapter").unwrap_or(-1);
        self.has_video = self.current.is_some() && p.get_flag("vo-configured").unwrap_or(false);
        if let Some(nav) = &self.nav {
            // The engine sees one endless stream; position comes from the navigator.
            let (pos, len) = nav.progress();
            match nav.kind {
                crate::nav::NavKind::Bluray => {
                    self.time = pos;
                    self.duration = len;
                }
                crate::nav::NavKind::Dvd => {
                    self.duration = 0.0;
                    self.nav_fraction = pos;
                }
            }
            self.chapter = -1;
        }

        if loaded || self.last_track_poll.elapsed() > Duration::from_millis(1500) {
            self.last_track_poll = Instant::now();
            if self.current.is_some() {
                self.tracks = p.tracks();
                self.chapters = p.chapters();
                self.disc_titles = p.get_i64("disc-titles").unwrap_or(0);
                self.disc_title = p.get_i64("disc-title").unwrap_or(-1);
                let w = p.get_i64("video-params/w").unwrap_or(0);
                let h = p.get_i64("video-params/h").unwrap_or(0);
                let fps = p.get_f64("container-fps").unwrap_or(0.0);
                let vc = p.get_string("video-codec").unwrap_or_default();
                self.info.video = if w > 0 { format!("{w}×{h} · {fps:.3} fps · {vc}") } else { String::new() };
                let ac = p.get_string("audio-codec-name").map(|c| mpv::codec_name(&c)).unwrap_or_default();
                let ch = p.get_i64("audio-params/channel-count").map(mpv::channel_layout).unwrap_or_default();
                let sr = p.get_i64("audio-params/samplerate").unwrap_or(0);
                self.info.audio = if ac.is_empty() { String::new() } else { format!("{ac} · {ch} · {} kHz", sr / 1000) };
                self.info.hw = p.get_string("hwdec-current").filter(|s| !s.is_empty() && s != "no").unwrap_or_else(|| "software".into());
                self.info.format = p.get_string("file-format").unwrap_or_default();
            }
        }
        if loaded {
            if let Some(t) = self.pending_resume.take() {
                self.seek_abs(t);
                self.notify(format!("Resumed from {}. Press Home to start from the beginning.", fmt_time(t)), false);
            }
        }
        if let Some((reason, error)) = ended {
            match reason {
                0 => {
                    if let Some(cur) = &self.current {
                        self.settings.resume.remove(&cur.key());
                    }
                    if self.repeat == Repeat::One {
                        if let Some(c) = self.current.clone() {
                            self.play(c);
                        }
                    } else {
                        self.next_item(false);
                    }
                }
                4 => {
                    let title = self.current.as_ref().map(|c| c.title.clone()).unwrap_or_default();
                    let disc = self.current.as_ref().and_then(|c| c.disc);
                    let hint = match disc {
                        Some(DiscKind::BluRay | DiscKind::UhdBluRay) => {
                            " This looks like a copy-protected Blu-ray: open its disc page to hand it to a licensed player you own, or import your own key file (Settings › Discs & Blu-ray)."
                        }
                        Some(DiscKind::DvdVideo) => " If this is a protected DVD, the engine needs the free libdvdcss library.",
                        _ => "",
                    };
                    let detail = self.log.back().cloned().unwrap_or(error);
                    self.notify(format!("Couldn't play \"{title}\". {detail}.{hint}"), true);
                    self.current = None;
                    self.has_video = false;
                }
                _ => {}
            }
        }
    }

    /// Draw the disc's menu buttons, highlights and subtitles over the picture.
    fn paint_nav_overlay(&mut self, ui: &mut Ui, rect: Rect) {
        self.nav_video_rect = None;
        let Some(nav) = self.nav.clone() else { return };
        let Some(p) = &self.player else { return };
        // Where the picture sits inside `rect` (the engine keeps its shape).
        let dw = p.get_i64("video-params/dw").unwrap_or(0) as f32;
        let dh = p.get_i64("video-params/dh").unwrap_or(0) as f32;
        let vh = p.get_i64("video-params/h").unwrap_or(0) as f32;
        if dw <= 0.0 || dh <= 0.0 {
            return;
        }
        let scale = (rect.width() / dw).min(rect.height() / dh);
        let vrect = Rect::from_center_size(rect.center(), vec2(dw * scale, dh * scale));
        self.nav_video_rect = Some(vrect);
        let mut ov = nav.shared.overlay.lock().unwrap();
        if ov.expires.is_some_and(|e| Instant::now() >= e) {
            ov.rgba.fill(0);
            ov.expires = None;
            ov.generation += 1;
        }
        let fw = if ov.frame_w > 0 { ov.frame_w as f32 } else { dw };
        let fh = if ov.frame_h > 0 { ov.frame_h as f32 } else if vh > 0.0 { vh } else { ov.height as f32 };
        self.nav_frame = (fw, fh);
        if ov.width == 0 || ov.height == 0 {
            return;
        }
        if ov.generation != self.nav_gen || self.nav_tex.is_none() {
            self.nav_gen = ov.generation;
            let img = egui::ColorImage::from_rgba_unmultiplied([ov.width, ov.height], &ov.rgba);
            match &mut self.nav_tex {
                Some(t) => t.set(img, TextureOptions::LINEAR),
                None => self.nav_tex = Some(ui.ctx().load_texture("disc-menu", img, TextureOptions::LINEAR)),
            }
        }
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2((fw / ov.width as f32).min(1.0), (fh / ov.height as f32).min(1.0)));
        drop(ov);
        if let Some(t) = &self.nav_tex {
            ui.painter().image(t.id(), vrect, uv, Color32::WHITE);
        }
        if nav.shared.in_menu.load(std::sync::atomic::Ordering::Acquire) {
            ui.ctx().request_repaint_after(Duration::from_millis(50));
        }
    }

    /// Screen position → disc picture coordinates (for menu buttons).
    pub fn nav_point(&self, pos: egui::Pos2) -> Option<(f32, f32)> {
        let r = self.nav_video_rect?;
        if !r.contains(pos) {
            return None;
        }
        let (fw, fh) = self.nav_frame;
        Some(((pos.x - r.left()) / r.width() * fw, (pos.y - r.top()) / r.height() * fh))
    }

    pub fn nav_in_menu(&self) -> bool {
        self.nav.as_ref().is_some_and(|n| n.shared.in_menu.load(std::sync::atomic::Ordering::Acquire))
    }

    pub fn nav_input(&self, i: crate::nav::NavInput) {
        if let Some(n) = &self.nav {
            n.input(i);
        }
    }

    /// Draw the video (or the audio artwork) into `rect`.
    pub fn paint_video(&mut self, ui: &mut Ui, rect: Rect) {
        ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
        let ppp = ui.ctx().pixels_per_point();
        let mut w = (rect.width() * ppp) as usize;
        let mut h = (rect.height() * ppp) as usize;
        let max_w = 2560usize;
        if w > max_w {
            h = h * max_w / w;
            w = max_w;
        }
        if let Some(p) = &mut self.player {
            if self.current.is_some() {
                if let Some(img) = p.render(w, h, false) {
                    match &mut self.video_tex {
                        Some(t) => t.set(img, TextureOptions::LINEAR),
                        None => self.video_tex = Some(ui.ctx().load_texture("video", img, TextureOptions::LINEAR)),
                    }
                }
            }
        }
        if self.has_video {
            if let Some(t) = &self.video_tex {
                let tex_w = t.size()[0] as f32;
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2((w as f32 / tex_w).min(1.0), 1.0));
                ui.painter().image(t.id(), rect, uv, Color32::WHITE);
            }
            self.paint_nav_overlay(ui, rect);
        } else if let Some(cur) = &self.current {
            let t = ui.ctx().input(|i| i.time) as f32;
            let s = rect.height().min(rect.width()) * 0.45;
            let art = Rect::from_center_size(rect.center() - vec2(0.0, s * 0.12), vec2(s, s));
            crate::ui::paint_disc_art(ui.painter(), art, if self.paused { 0.0 } else { t * 1.5 });
            if rect.height() > 200.0 {
                ui.painter().text(
                    pos2(rect.center().x, art.bottom() + 24.0),
                    egui::Align2::CENTER_CENTER,
                    &cur.title,
                    egui::FontId::proportional(20.0),
                    TEXT,
                );
            }
            if !self.paused {
                ui.ctx().request_repaint_after(Duration::from_millis(33));
            }
        }
    }

    pub fn screenshot_tick(&mut self, ctx: &egui::Context) {
        let Some(job) = &mut self.screenshot else { return };
        ctx.request_repaint();
        if job.done.load(Ordering::Acquire) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        job.frames += 1;
        // Test hook for checking disc menus without a person: PLAYCRAFT_TEST_ENTER_AT=<frame>.
        let enter_at = std::env::var("PLAYCRAFT_TEST_ENTER_AT").ok().and_then(|v| v.parse::<u32>().ok());
        let frames = job.frames;
        if enter_at == Some(frames) {
            if let Some(n) = &self.nav {
                eprintln!("[menus] test: pressing Enter (in menu: {})", n.shared.in_menu.load(Ordering::Acquire));
                n.input(crate::nav::NavInput::Enter);
            }
        }
        // PLAYCRAFT_TEST_MOUSE="frame:x:y;frame:x:y" moves the pointer over menu buttons
        // (disc picture coordinates); PLAYCRAFT_TEST_REMOTE=1 opens the remote.
        if let Ok(seq) = std::env::var("PLAYCRAFT_TEST_MOUSE") {
            for step in seq.split(';') {
                let v: Vec<f32> = step.split(':').filter_map(|s| s.parse().ok()).collect();
                if v.len() == 3 && v[0] as u32 == frames {
                    if let Some(n) = &self.nav {
                        eprintln!("[menus] test: pointer at {},{}", v[1], v[2]);
                        n.input(crate::nav::NavInput::MouseMove(v[1], v[2]));
                    }
                }
            }
        }
        if frames == 5 && std::env::var("PLAYCRAFT_TEST_REMOTE").is_ok() {
            self.remote_open = true;
        }
        let Some(job) = &mut self.screenshot else { return };
        let target = SCREENSHOT_FRAMES.load(Ordering::Relaxed);
        if job.frames >= target && !job.requested {
            job.requested = true;
            let path = job.path.clone();
            let done = job.done.clone();
            let c2 = ctx.clone();
            ctx.request_screenshot(move |img| {
                let [w, h] = img.size;
                let mut bytes = Vec::with_capacity(w * h * 4);
                for p in &img.pixels {
                    bytes.extend_from_slice(&p.to_srgba_unmultiplied());
                }
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                    Some(im) => {
                        if let Err(e) = im.save(&path) {
                            eprintln!("could not save screenshot: {e}");
                        }
                    }
                    None => eprintln!("could not build screenshot image"),
                }
                done.store(true, Ordering::Release);
                c2.request_repaint();
            });
        }
        if job.frames > target + 900 {
            eprintln!("screenshot timed out");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        crate::ui::draw(self, ui);
    }

    fn on_exit(&mut self) {
        self.remember_position();
        self.settings.save();
    }
}

/// Engine settings for streams coming from the disc navigator (low latency, timestamps
/// that may jump), and the normal settings for everything else.
fn set_nav_mode(p: &Player, nav: Option<crate::nav::NavKind>, subs_on: bool) {
    match nav {
        Some(kind) => {
            p.set("demuxer-lavf-format", if kind == crate::nav::NavKind::Dvd { "mpeg" } else { "mpegts" });
            p.set("demuxer-lavf-linearize-timestamps", "yes");
            p.set("demuxer-lavf-probesize", "262144");
            p.set("demuxer-lavf-analyzeduration", "0.8");
            p.set("cache", "no");
            p.set("demuxer-readahead-secs", "0.3");
            p.set("video-latency-hacks", "yes");
            p.set("sid", "no"); // menus and subtitles are drawn by PlayCraft
        }
        None => {
            p.set("demuxer-lavf-format", "");
            p.set("demuxer-lavf-linearize-timestamps", "auto");
            p.set("demuxer-lavf-probesize", "0");
            p.set("demuxer-lavf-analyzeduration", "0");
            p.set("cache", "auto");
            p.set("demuxer-readahead-secs", "1");
            p.set("video-latency-hacks", "no");
            p.set("sid", if subs_on { "auto" } else { "no" });
        }
    }
}

fn spawn_drive_watcher(ctx: egui::Context) -> Receiver<(Vec<Drive>, Vec<discs::Volume>)> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || loop {
        let d = discs::detect_drives();
        let v = discs::list_volumes();
        if tx.send((d, v)).is_err() {
            break;
        }
        ctx.request_repaint();
        std::thread::sleep(Duration::from_secs(3));
    });
    rx
}
