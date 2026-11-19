//! Runtime binding to libmpv, the open-source playback engine (FFmpeg + libdvdnav +
//! libbluray + libcdio). The library is loaded at runtime so the player starts even
//! when the engine has not been installed yet; the UI then offers to install it.

use libloading::Library;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[repr(C)]
struct MpvEvent {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}

#[repr(C)]
struct MpvEventLogMessage {
    prefix: *const c_char,
    level: *const c_char,
    text: *const c_char,
    log_level: c_int,
}

#[repr(C)]
struct MpvEventEndFile {
    reason: c_int,
    error: c_int,
}

#[repr(C)]
struct RenderParam {
    kind: c_int,
    data: *mut c_void,
}

const FORMAT_FLAG: c_int = 3;
const FORMAT_INT64: c_int = 4;
const FORMAT_DOUBLE: c_int = 5;

const RP_INVALID: c_int = 0;
const RP_API_TYPE: c_int = 1;
const RP_SW_SIZE: c_int = 17;
const RP_SW_FORMAT: c_int = 18;
const RP_SW_STRIDE: c_int = 19;
const RP_SW_POINTER: c_int = 20;

type FnCreate = unsafe extern "C" fn() -> *mut c_void;
type FnHandleInt = unsafe extern "C" fn(*mut c_void) -> c_int;
type FnHandleVoid = unsafe extern "C" fn(*mut c_void);
type FnSetStr = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int;
type FnCommand = unsafe extern "C" fn(*mut c_void, *mut *const c_char) -> c_int;
type FnGetStr = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_char;
type FnGetProp = unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_void) -> c_int;
type FnFree = unsafe extern "C" fn(*mut c_void);
type FnWaitEvent = unsafe extern "C" fn(*mut c_void, f64) -> *mut MpvEvent;
type FnReqLog = unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int;
type FnErrStr = unsafe extern "C" fn(c_int) -> *const c_char;
type FnApiVer = unsafe extern "C" fn() -> u64;
type FnRcCreate = unsafe extern "C" fn(*mut *mut c_void, *mut c_void, *mut RenderParam) -> c_int;
type FnRcSetCb =
    unsafe extern "C" fn(*mut c_void, Option<unsafe extern "C" fn(*mut c_void)>, *mut c_void);
type FnRcUpdate = unsafe extern "C" fn(*mut c_void) -> u64;
type FnRcRender = unsafe extern "C" fn(*mut c_void, *mut RenderParam) -> c_int;

struct Api {
    _lib: Library,
    create: FnCreate,
    initialize: FnHandleInt,
    terminate_destroy: FnHandleVoid,
    set_option_string: FnSetStr,
    set_property_string: FnSetStr,
    command: FnCommand,
    get_property_string: FnGetStr,
    get_property: FnGetProp,
    free: FnFree,
    wait_event: FnWaitEvent,
    request_log_messages: FnReqLog,
    error_string: FnErrStr,
    client_api_version: FnApiVer,
    rc_create: FnRcCreate,
    rc_set_update_callback: FnRcSetCb,
    rc_update: FnRcUpdate,
    rc_render: FnRcRender,
    rc_free: FnHandleVoid,
    stream_cb_add_ro: Option<FnStreamCbAdd>,
}

#[repr(C)]
struct StreamCbInfo {
    cookie: *mut c_void,
    read_fn: Option<unsafe extern "C" fn(*mut c_void, *mut c_char, u64) -> i64>,
    seek_fn: Option<unsafe extern "C" fn(*mut c_void, i64) -> i64>,
    size_fn: Option<unsafe extern "C" fn(*mut c_void) -> i64>,
    close_fn: Option<unsafe extern "C" fn(*mut c_void)>,
    cancel_fn: Option<unsafe extern "C" fn(*mut c_void)>,
}

type FnStreamOpen = unsafe extern "C" fn(*mut c_void, *mut c_char, *mut StreamCbInfo) -> c_int;
type FnStreamCbAdd = unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void, FnStreamOpen) -> c_int;

unsafe extern "C" fn nav_read(cookie: *mut c_void, buf: *mut c_char, n: u64) -> i64 {
    let slice = std::slice::from_raw_parts_mut(buf as *mut u8, n.min(1 << 20) as usize);
    crate::nav::stream_read(cookie as *mut crate::nav::StreamCookie, slice)
}
unsafe extern "C" fn nav_close(cookie: *mut c_void) {
    crate::nav::stream_close(cookie as *mut crate::nav::StreamCookie)
}
unsafe extern "C" fn nav_cancel(cookie: *mut c_void) {
    crate::nav::stream_cancel(cookie as *mut crate::nav::StreamCookie)
}
unsafe extern "C" fn nav_open(_ud: *mut c_void, _uri: *mut c_char, info: *mut StreamCbInfo) -> c_int {
    match crate::nav::open_stream() {
        Some(cookie) => {
            *info = StreamCbInfo {
                cookie: cookie as *mut c_void,
                read_fn: Some(nav_read),
                seek_fn: None,
                size_fn: None,
                close_fn: Some(nav_close),
                cancel_fn: Some(nav_cancel),
            };
            0
        }
        None => -13, // MPV_ERROR_LOADING_FAILED
    }
}

macro_rules! sym {
    ($lib:expr, $name:literal) => {
        *$lib
            .get(concat!($name, "\0").as_bytes())
            .map_err(|e| format!("{} missing: {e}", $name))?
    };
}

impl Api {
    fn load(path: &Path) -> Result<Api, String> {
        unsafe {
            let lib = Library::new(path).map_err(|e| e.to_string())?;
            Ok(Api {
                create: sym!(lib, "mpv_create"),
                initialize: sym!(lib, "mpv_initialize"),
                terminate_destroy: sym!(lib, "mpv_terminate_destroy"),
                set_option_string: sym!(lib, "mpv_set_option_string"),
                set_property_string: sym!(lib, "mpv_set_property_string"),
                command: sym!(lib, "mpv_command"),
                get_property_string: sym!(lib, "mpv_get_property_string"),
                get_property: sym!(lib, "mpv_get_property"),
                free: sym!(lib, "mpv_free"),
                wait_event: sym!(lib, "mpv_wait_event"),
                request_log_messages: sym!(lib, "mpv_request_log_messages"),
                error_string: sym!(lib, "mpv_error_string"),
                client_api_version: sym!(lib, "mpv_client_api_version"),
                rc_create: sym!(lib, "mpv_render_context_create"),
                rc_set_update_callback: sym!(lib, "mpv_render_context_set_update_callback"),
                rc_update: sym!(lib, "mpv_render_context_update"),
                rc_render: sym!(lib, "mpv_render_context_render"),
                rc_free: sym!(lib, "mpv_render_context_free"),
                stream_cb_add_ro: lib.get::<FnStreamCbAdd>(b"mpv_stream_cb_add_ro\0").ok().map(|s| *s),
                _lib: lib,
            })
        }
    }
}

/// Library file names the engine can have on each system.
pub fn library_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["libmpv-2.dll", "mpv-2.dll", "mpv-1.dll", "libmpv.dll"]
    } else if cfg!(target_os = "macos") {
        &["libmpv.2.dylib", "libmpv.dylib", "libmpv.1.dylib"]
    } else {
        &["libmpv.so.2", "libmpv.so", "libmpv.so.1"]
    }
}

/// Folders searched for the engine, most specific first.
pub fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            dirs.push(d.to_path_buf());
            dirs.push(d.join("engine"));
        }
    }
    dirs.push(crate::library::engine_dir());
    if cfg!(target_os = "macos") {
        dirs.push(PathBuf::from("/opt/homebrew/lib"));
        dirs.push(PathBuf::from("/usr/local/lib"));
        dirs.push(PathBuf::from("/opt/local/lib"));
    }
    dirs
}

pub struct Engine {
    api: Arc<Api>,
    pub path: String,
}

impl Engine {
    /// Find and load the engine. Returns a human-readable reason on failure.
    pub fn locate() -> Result<Engine, String> {
        let mut last_err = String::from("not found");
        for dir in search_dirs() {
            for name in library_names() {
                let p = dir.join(name);
                if p.exists() {
                    match Api::load(&p) {
                        Ok(api) => {
                            return Ok(Engine {
                                api: Arc::new(api),
                                path: p.display().to_string(),
                            })
                        }
                        Err(e) => last_err = format!("{}: {e}", p.display()),
                    }
                }
            }
        }
        // Finally let the system loader search its usual places.
        for name in library_names() {
            match Api::load(Path::new(name)) {
                Ok(api) => {
                    return Ok(Engine {
                        api: Arc::new(api),
                        path: format!("{name} (system)"),
                    })
                }
                Err(e) => {
                    if !last_err.contains(':') {
                        last_err = e
                    }
                }
            }
        }
        Err(last_err)
    }

    pub fn api_version(&self) -> String {
        let v = unsafe { (self.api.client_api_version)() };
        format!("{}.{}", v >> 16, v & 0xffff)
    }
}

struct CallbackData {
    flag: Arc<AtomicBool>,
    ctx: eframe::egui::Context,
}

unsafe extern "C" fn on_render_update(data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let d = &*(data as *const CallbackData);
    d.flag.store(true, Ordering::Release);
    d.ctx.request_repaint();
}

#[repr(C, align(64))]
#[derive(Clone, Copy)]
struct Chunk([u8; 64]);

pub enum PlayerEvent {
    FileLoaded,
    EndFile { reason: i32, error: String },
    Log(String),
}

#[derive(Clone, Debug, Default)]
pub struct Track {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub lang: String,
    pub codec: String,
    pub channels: i64,
    pub selected: bool,
    pub external: bool,
}

impl Track {
    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        if !self.lang.is_empty() {
            parts.push(language_name(&self.lang));
        }
        if !self.title.is_empty() {
            parts.push(self.title.clone());
        }
        if !self.codec.is_empty() {
            parts.push(codec_name(&self.codec));
        }
        if self.channels > 0 {
            parts.push(channel_layout(self.channels));
        }
        if self.external {
            parts.push("external file".into());
        }
        if parts.is_empty() {
            format!("Track {}", self.id)
        } else {
            format!("{}: {}", self.id, parts.join(" · "))
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Chapter {
    pub title: String,
    pub time: f64,
}

pub struct Player {
    api: Arc<Api>,
    handle: *mut c_void,
    render: *mut c_void,
    redraw: Arc<AtomicBool>,
    cb: *mut CallbackData,
    buf: Vec<Chunk>,
    last_size: [usize; 2],
}

// libmpv handles are thread-safe; the render context is only used from the UI thread.
unsafe impl Send for Player {}

fn cstr(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap_or_default()
}

impl Player {
    pub fn new(engine: &Engine, ctx: &eframe::egui::Context, opts: &[(&str, String)]) -> Result<Player, String> {
        let api = engine.api.clone();
        unsafe {
            let handle = (api.create)();
            if handle.is_null() {
                return Err("the engine could not start".into());
            }
            let base: [(&str, &str); 14] = [
                ("vo", "libmpv"),
                ("idle", "yes"),
                ("keep-open", "no"),
                ("terminal", "no"),
                ("config", "no"),
                ("osc", "no"),
                ("input-default-bindings", "no"),
                ("input-vo-keyboard", "no"),
                ("ytdl", "no"),
                ("sub-auto", "fuzzy"),
                ("volume-max", "150"),
                ("image-display-duration", "5"),
                ("screenshot-format", "png"),
                ("audio-client-name", "PlayCraft"),
            ];
            for (k, v) in base {
                (api.set_option_string)(handle, cstr(k).as_ptr(), cstr(v).as_ptr());
            }
            for (k, v) in opts {
                (api.set_option_string)(handle, cstr(k).as_ptr(), cstr(v).as_ptr());
            }
            let err = (api.initialize)(handle);
            if err < 0 {
                let msg = CStr::from_ptr((api.error_string)(err)).to_string_lossy().into_owned();
                (api.terminate_destroy)(handle);
                return Err(format!("the engine failed to initialise: {msg}"));
            }
            (api.request_log_messages)(handle, cstr("warn").as_ptr());
            // Disc menus: the navigator feeds the engine through "discnav://".
            if let Some(add) = api.stream_cb_add_ro {
                add(handle, cstr("discnav").as_ptr(), std::ptr::null_mut(), nav_open);
            }

            let sw = cstr("sw");
            let mut params = [
                RenderParam { kind: RP_API_TYPE, data: sw.as_ptr() as *mut c_void },
                RenderParam { kind: RP_INVALID, data: std::ptr::null_mut() },
            ];
            let mut render: *mut c_void = std::ptr::null_mut();
            let err = (api.rc_create)(&mut render, handle, params.as_mut_ptr());
            if err < 0 || render.is_null() {
                let msg = CStr::from_ptr((api.error_string)(err)).to_string_lossy().into_owned();
                (api.terminate_destroy)(handle);
                return Err(format!("the engine could not create a video surface: {msg}"));
            }
            let redraw = Arc::new(AtomicBool::new(true));
            let cb = Box::into_raw(Box::new(CallbackData { flag: redraw.clone(), ctx: ctx.clone() }));
            (api.rc_set_update_callback)(render, Some(on_render_update), cb as *mut c_void);
            Ok(Player { api, handle, render, redraw, cb, buf: Vec::new(), last_size: [0, 0] })
        }
    }

    /// Whether this engine can take a stream from PlayCraft (needed for disc menus).
    pub fn supports_nav(&self) -> bool {
        self.api.stream_cb_add_ro.is_some()
    }

    pub fn command(&self, args: &[&str]) -> Result<(), String> {
        let owned: Vec<CString> = args.iter().map(|a| cstr(a)).collect();
        let mut ptrs: Vec<*const c_char> = owned.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        let err = unsafe { (self.api.command)(self.handle, ptrs.as_mut_ptr()) };
        if err < 0 {
            Err(self.error_text(err))
        } else {
            Ok(())
        }
    }

    fn error_text(&self, err: c_int) -> String {
        unsafe { CStr::from_ptr((self.api.error_string)(err)).to_string_lossy().into_owned() }
    }

    pub fn set(&self, name: &str, value: &str) {
        unsafe {
            (self.api.set_property_string)(self.handle, cstr(name).as_ptr(), cstr(value).as_ptr());
        }
    }

    pub fn get_string(&self, name: &str) -> Option<String> {
        unsafe {
            let p = (self.api.get_property_string)(self.handle, cstr(name).as_ptr());
            if p.is_null() {
                return None;
            }
            let s = CStr::from_ptr(p).to_string_lossy().into_owned();
            (self.api.free)(p as *mut c_void);
            Some(s)
        }
    }

    pub fn get_f64(&self, name: &str) -> Option<f64> {
        let mut v: f64 = 0.0;
        let err = unsafe {
            (self.api.get_property)(self.handle, cstr(name).as_ptr(), FORMAT_DOUBLE, &mut v as *mut f64 as *mut c_void)
        };
        (err >= 0).then_some(v)
    }

    pub fn get_i64(&self, name: &str) -> Option<i64> {
        let mut v: i64 = 0;
        let err = unsafe {
            (self.api.get_property)(self.handle, cstr(name).as_ptr(), FORMAT_INT64, &mut v as *mut i64 as *mut c_void)
        };
        (err >= 0).then_some(v)
    }

    pub fn get_flag(&self, name: &str) -> Option<bool> {
        let mut v: c_int = 0;
        let err = unsafe {
            (self.api.get_property)(self.handle, cstr(name).as_ptr(), FORMAT_FLAG, &mut v as *mut c_int as *mut c_void)
        };
        (err >= 0).then_some(v != 0)
    }

    pub fn poll_events(&self) -> Vec<PlayerEvent> {
        let mut out = Vec::new();
        for _ in 0..256 {
            let ev = unsafe { &*(self.api.wait_event)(self.handle, 0.0) };
            match ev.event_id {
                0 => break,
                2 if !ev.data.is_null() => {
                    let m = unsafe { &*(ev.data as *const MpvEventLogMessage) };
                    let text = unsafe { CStr::from_ptr(m.text).to_string_lossy().trim().to_string() };
                    let prefix = unsafe { CStr::from_ptr(m.prefix).to_string_lossy().into_owned() };
                    if !text.is_empty() {
                        out.push(PlayerEvent::Log(format!("[{prefix}] {text}")));
                    }
                }
                7 if !ev.data.is_null() => {
                    let e = unsafe { &*(ev.data as *const MpvEventEndFile) };
                    let error = if e.error < 0 { self.error_text(e.error) } else { String::new() };
                    out.push(PlayerEvent::EndFile { reason: e.reason, error });
                }
                8 => out.push(PlayerEvent::FileLoaded),
                _ => {}
            }
        }
        out
    }

    /// Draw the current video frame at the given pixel size if a new one is ready.
    /// The engine keeps the picture's shape and fills the rest with black.
    pub fn render(&mut self, w: usize, h: usize, force: bool) -> Option<eframe::egui::ColorImage> {
        let w = (w.max(16) + 15) / 16 * 16;
        let h = h.max(16);
        let size_changed = self.last_size != [w, h];
        let flagged = self.redraw.swap(false, Ordering::AcqRel);
        if !(flagged || size_changed || force) {
            return None;
        }
        let flags = unsafe { (self.api.rc_update)(self.render) };
        if flags & 1 == 0 && !size_changed && !force {
            return None;
        }
        let stride = w * 4;
        let bytes = stride * h;
        let chunks = bytes.div_ceil(64);
        if self.buf.len() != chunks {
            self.buf = vec![Chunk([0; 64]); chunks];
        }
        let mut size: [c_int; 2] = [w as c_int, h as c_int];
        let fmt = cstr("rgb0");
        let mut stride_v: usize = stride;
        let mut params = [
            RenderParam { kind: RP_SW_SIZE, data: size.as_mut_ptr() as *mut c_void },
            RenderParam { kind: RP_SW_FORMAT, data: fmt.as_ptr() as *mut c_void },
            RenderParam { kind: RP_SW_STRIDE, data: &mut stride_v as *mut usize as *mut c_void },
            RenderParam { kind: RP_SW_POINTER, data: self.buf.as_mut_ptr() as *mut c_void },
            RenderParam { kind: RP_INVALID, data: std::ptr::null_mut() },
        ];
        let err = unsafe { (self.api.rc_render)(self.render, params.as_mut_ptr()) };
        if err < 0 {
            return None;
        }
        self.last_size = [w, h];
        let raw = unsafe { std::slice::from_raw_parts(self.buf.as_ptr() as *const u8, bytes) };
        let pixels = raw
            .chunks_exact(4)
            .map(|p| eframe::egui::Color32::from_rgb(p[0], p[1], p[2]))
            .collect();
        Some(eframe::egui::ColorImage::new([w, h], pixels))
    }

    pub fn tracks(&self) -> Vec<Track> {
        let n = self.get_i64("track-list/count").unwrap_or(0).min(200);
        (0..n)
            .map(|i| {
                let g = |k: &str| self.get_string(&format!("track-list/{i}/{k}")).unwrap_or_default();
                Track {
                    id: self.get_i64(&format!("track-list/{i}/id")).unwrap_or(0),
                    kind: g("type"),
                    title: g("title"),
                    lang: g("lang"),
                    codec: g("codec"),
                    channels: self.get_i64(&format!("track-list/{i}/demux-channel-count")).unwrap_or(0),
                    selected: self.get_flag(&format!("track-list/{i}/selected")).unwrap_or(false),
                    external: self.get_flag(&format!("track-list/{i}/external")).unwrap_or(false),
                }
            })
            .collect()
    }

    pub fn chapters(&self) -> Vec<Chapter> {
        let n = self.get_i64("chapter-list/count").unwrap_or(0).min(999);
        (0..n)
            .map(|i| Chapter {
                title: self.get_string(&format!("chapter-list/{i}/title")).unwrap_or_default(),
                time: self.get_f64(&format!("chapter-list/{i}/time")).unwrap_or(0.0),
            })
            .collect()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        unsafe {
            (self.api.rc_set_update_callback)(self.render, None, std::ptr::null_mut());
            (self.api.rc_free)(self.render);
            (self.api.terminate_destroy)(self.handle);
            drop(Box::from_raw(self.cb));
        }
    }
}

pub fn codec_name(c: &str) -> String {
    let n = match c {
        "h264" => "H.264/AVC",
        "hevc" => "H.265/HEVC",
        "av1" => "AV1",
        "vp9" => "VP9",
        "vp8" => "VP8",
        "mpeg2video" => "MPEG-2",
        "mpeg1video" => "MPEG-1",
        "mpeg4" => "MPEG-4",
        "vc1" => "VC-1",
        "truehd" => "Dolby TrueHD",
        "eac3" => "Dolby Digital Plus",
        "ac3" => "Dolby Digital",
        "dts" => "DTS",
        "mlp" => "MLP",
        "aac" => "AAC",
        "mp3" => "MP3",
        "mp2" => "MPEG Audio",
        "flac" => "FLAC",
        "opus" => "Opus",
        "vorbis" => "Vorbis",
        "pcm_bluray" => "LPCM",
        "pcm_dvd" => "LPCM",
        "pcm_s16le" | "pcm_s24le" | "pcm_s16be" => "PCM",
        "hdmv_pgs_subtitle" => "Blu-ray subtitles",
        "dvd_subtitle" => "DVD subtitles",
        "subrip" => "SRT",
        "ass" => "ASS",
        "webvtt" => "WebVTT",
        other => return other.to_uppercase(),
    };
    n.to_string()
}

pub fn channel_layout(n: i64) -> String {
    match n {
        1 => "Mono".into(),
        2 => "Stereo".into(),
        6 => "5.1".into(),
        8 => "7.1".into(),
        n => format!("{n} ch"),
    }
}

pub fn language_name(code: &str) -> String {
    let n = match code {
        "en" | "eng" => "English",
        "fr" | "fre" | "fra" => "French",
        "de" | "ger" | "deu" => "German",
        "es" | "spa" => "Spanish",
        "it" | "ita" => "Italian",
        "ja" | "jpn" => "Japanese",
        "zh" | "chi" | "zho" => "Chinese",
        "ko" | "kor" => "Korean",
        "pt" | "por" => "Portuguese",
        "ru" | "rus" => "Russian",
        "nl" | "dut" | "nld" => "Dutch",
        "sv" | "swe" => "Swedish",
        "da" | "dan" => "Danish",
        "no" | "nor" => "Norwegian",
        "fi" | "fin" => "Finnish",
        "pl" | "pol" => "Polish",
        "cs" | "cze" | "ces" => "Czech",
        "hu" | "hun" => "Hungarian",
        "tr" | "tur" => "Turkish",
        "ar" | "ara" => "Arabic",
        "he" | "heb" => "Hebrew",
        "hi" | "hin" => "Hindi",
        "th" | "tha" => "Thai",
        "el" | "gre" | "ell" => "Greek",
        other => return other.to_string(),
    };
    n.to_string()
}
