//! Disc menus. The playback engine plays titles but has no menu support, so PlayCraft
//! drives the disc itself with the open-source navigators (libdvdnav for DVD, libbluray
//! for Blu-ray HDMV menus), streams what they read into the engine through a custom
//! "discnav://" address, and draws the menu buttons, highlights and subtitles on top.

use libloading::Library;
use std::ffi::{c_char, c_int, c_void, CString};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NavKind {
    Dvd,
    Bluray,
}

#[derive(Clone, Copy, Debug)]
pub enum NavInput {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Menu,
    Popup,
    MouseMove(f32, f32),
    MouseClick(f32, f32),
    Chapter(i32),
    Skip(f64),
    SeekFraction(f64),
}

/// The picture drawn over the video (menu graphics, highlights, subtitles).
#[derive(Default)]
pub struct Overlay {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    pub generation: u64,
    pub expires: Option<Instant>,
    /// Size of the video frame the overlay is drawn for (0 = use the video's own size).
    pub frame_w: usize,
    pub frame_h: usize,
}

#[derive(Default)]
pub struct NavShared {
    pub overlay: Mutex<Overlay>,
    pub in_menu: AtomicBool,
    pub popup: AtomicBool,
    pub error: Mutex<Option<String>>,
    pub bytes: AtomicU64,
    pub restart: AtomicBool,
    pub show_subs: AtomicBool,
    /// (position, length) as fractions or seconds, see `NavSession::progress`.
    pub progress: Mutex<(f64, f64)>,
}

impl NavShared {
    fn fail(&self, msg: impl Into<String>) {
        let mut e = self.error.lock().unwrap();
        if e.is_none() {
            *e = Some(msg.into());
        }
    }
}

// ---------------------------------------------------------------- library loading

fn engine_dirs() -> Vec<PathBuf> {
    let mut d = Vec::new();
    if let Some(e) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("engine"))) {
        d.push(e);
    }
    d.push(crate::library::engine_dir());
    if cfg!(target_os = "macos") {
        d.push(PathBuf::from("/opt/homebrew/lib"));
        d.push(PathBuf::from("/usr/local/lib"));
    }
    d
}

fn load_any(names: &[&str]) -> Result<Library, String> {
    let mut last = String::from("not found");
    for dir in engine_dirs() {
        for n in names {
            let p = dir.join(n);
            if p.exists() {
                match unsafe { Library::new(&p) } {
                    Ok(l) => return Ok(l),
                    Err(e) => last = format!("{}: {e}", p.display()),
                }
            }
        }
    }
    for n in names {
        if let Ok(l) = unsafe { Library::new(n) } {
            return Ok(l);
        }
    }
    Err(last)
}

macro_rules! sym {
    ($lib:expr, $name:literal) => {
        unsafe { *$lib.get(concat!($name, "\0").as_bytes()).map_err(|e| format!("{} missing: {e}", $name))? }
    };
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
struct HlArea {
    palette: u32,
    sx: u16,
    sy: u16,
    ex: u16,
    ey: u16,
    pts: u32,
    button_n: u32,
}

type P = *mut c_void;

struct DvdLib {
    _lib: Library,
    open: unsafe extern "C" fn(*mut P, *const c_char) -> c_int,
    close: unsafe extern "C" fn(P) -> c_int,
    next_block: unsafe extern "C" fn(P, *mut u8, *mut i32, *mut i32) -> c_int,
    readahead: unsafe extern "C" fn(P, i32) -> c_int,
    pgc_pos: unsafe extern "C" fn(P, i32) -> c_int,
    menu_lang: unsafe extern "C" fn(P, *mut c_char) -> c_int,
    audio_lang: unsafe extern "C" fn(P, *mut c_char) -> c_int,
    spu_lang: unsafe extern "C" fn(P, *mut c_char) -> c_int,
    still_skip: unsafe extern "C" fn(P) -> c_int,
    wait_skip: unsafe extern "C" fn(P) -> c_int,
    is_vts: unsafe extern "C" fn(P) -> i8,
    get_pci: unsafe extern "C" fn(P) -> P,
    up: unsafe extern "C" fn(P, P) -> c_int,
    down: unsafe extern "C" fn(P, P) -> c_int,
    left: unsafe extern "C" fn(P, P) -> c_int,
    right: unsafe extern "C" fn(P, P) -> c_int,
    activate: unsafe extern "C" fn(P, P) -> c_int,
    mouse_select: unsafe extern "C" fn(P, P, i32, i32) -> c_int,
    mouse_activate: unsafe extern "C" fn(P, P, i32, i32) -> c_int,
    menu_call: unsafe extern "C" fn(P, c_int) -> c_int,
    cur_highlight: unsafe extern "C" fn(P, *mut i32) -> c_int,
    hl_area: unsafe extern "C" fn(P, i32, i32, *mut HlArea) -> c_int,
    get_position: unsafe extern "C" fn(P, *mut u32, *mut u32) -> c_int,
    sector_search: unsafe extern "C" fn(P, i64, i32) -> c_int,
    next_pg: unsafe extern "C" fn(P) -> c_int,
    prev_pg: unsafe extern "C" fn(P) -> c_int,
}

impl DvdLib {
    fn load() -> Result<DvdLib, String> {
        let lib = load_any(if cfg!(windows) {
            &["libdvdnav-4.dll", "libdvdnav.dll"]
        } else if cfg!(target_os = "macos") {
            &["libdvdnav.4.dylib", "libdvdnav.5.dylib", "libdvdnav.dylib"]
        } else {
            &["libdvdnav.so.4", "libdvdnav.so"]
        })?;
        Ok(DvdLib {
            open: sym!(lib, "dvdnav_open"),
            close: sym!(lib, "dvdnav_close"),
            next_block: sym!(lib, "dvdnav_get_next_block"),
            readahead: sym!(lib, "dvdnav_set_readahead_flag"),
            pgc_pos: sym!(lib, "dvdnav_set_PGC_positioning_flag"),
            menu_lang: sym!(lib, "dvdnav_menu_language_select"),
            audio_lang: sym!(lib, "dvdnav_audio_language_select"),
            spu_lang: sym!(lib, "dvdnav_spu_language_select"),
            still_skip: sym!(lib, "dvdnav_still_skip"),
            wait_skip: sym!(lib, "dvdnav_wait_skip"),
            is_vts: sym!(lib, "dvdnav_is_domain_vts"),
            get_pci: sym!(lib, "dvdnav_get_current_nav_pci"),
            up: sym!(lib, "dvdnav_upper_button_select"),
            down: sym!(lib, "dvdnav_lower_button_select"),
            left: sym!(lib, "dvdnav_left_button_select"),
            right: sym!(lib, "dvdnav_right_button_select"),
            activate: sym!(lib, "dvdnav_button_activate"),
            mouse_select: sym!(lib, "dvdnav_mouse_select"),
            mouse_activate: sym!(lib, "dvdnav_mouse_activate"),
            menu_call: sym!(lib, "dvdnav_menu_call"),
            cur_highlight: sym!(lib, "dvdnav_get_current_highlight"),
            hl_area: sym!(lib, "dvdnav_get_highlight_area"),
            get_position: sym!(lib, "dvdnav_get_position"),
            sector_search: sym!(lib, "dvdnav_sector_search"),
            next_pg: sym!(lib, "dvdnav_next_pg_search"),
            prev_pg: sym!(lib, "dvdnav_prev_pg_search"),
            _lib: lib,
        })
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct BdEvent {
    event: u32,
    param: u32,
}

#[repr(C)]
struct BdOverlay {
    pts: i64,
    plane: u8,
    cmd: u8,
    palette_update_flag: u8,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    palette: *const [u8; 4],
    img: *const BdRle,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct BdRle {
    len: u16,
    color: u16,
}

#[repr(C)]
struct BdTitleInfo {
    idx: u32,
    playlist: u32,
    duration: u64,
}

type BdOverlayProc = unsafe extern "C" fn(P, *const BdOverlay);

struct BdLib {
    _lib: Library,
    open: unsafe extern "C" fn(*const c_char, *const c_char) -> P,
    close: unsafe extern "C" fn(P),
    play: unsafe extern "C" fn(P) -> c_int,
    read_ext: unsafe extern "C" fn(P, *mut u8, c_int, *mut BdEvent) -> c_int,
    user_input: unsafe extern "C" fn(P, i64, u32) -> c_int,
    mouse_select: unsafe extern "C" fn(P, i64, u16, u16) -> c_int,
    menu_call: unsafe extern "C" fn(P, i64) -> c_int,
    register_overlay: unsafe extern "C" fn(P, P, Option<BdOverlayProc>),
    set_setting: unsafe extern "C" fn(P, u32, u32) -> c_int,
    set_setting_str: unsafe extern "C" fn(P, u32, *const c_char) -> c_int,
    skip_still: unsafe extern "C" fn(P) -> c_int,
    tell_time: unsafe extern "C" fn(P) -> u64,
    seek_time: unsafe extern "C" fn(P, u64) -> i64,
    seek_chapter: unsafe extern "C" fn(P, u32) -> i64,
    current_chapter: unsafe extern "C" fn(P) -> u32,
    playlist_info: unsafe extern "C" fn(P, u32, u32) -> *mut BdTitleInfo,
    free_title_info: unsafe extern "C" fn(*mut BdTitleInfo),
}

impl BdLib {
    fn load() -> Result<BdLib, String> {
        let lib = load_any(if cfg!(windows) {
            &["libbluray-4.dll", "libbluray-3.dll", "libbluray-2.dll", "libbluray.dll"]
        } else if cfg!(target_os = "macos") {
            &["libbluray.3.dylib", "libbluray.2.dylib", "libbluray.dylib"]
        } else {
            &["libbluray.so.3", "libbluray.so.2", "libbluray.so"]
        })?;
        Ok(BdLib {
            open: sym!(lib, "bd_open"),
            close: sym!(lib, "bd_close"),
            play: sym!(lib, "bd_play"),
            read_ext: sym!(lib, "bd_read_ext"),
            user_input: sym!(lib, "bd_user_input"),
            mouse_select: sym!(lib, "bd_mouse_select"),
            menu_call: sym!(lib, "bd_menu_call"),
            register_overlay: sym!(lib, "bd_register_overlay_proc"),
            set_setting: sym!(lib, "bd_set_player_setting"),
            set_setting_str: sym!(lib, "bd_set_player_setting_str"),
            skip_still: sym!(lib, "bd_read_skip_still"),
            tell_time: sym!(lib, "bd_tell_time"),
            seek_time: sym!(lib, "bd_seek_time"),
            seek_chapter: sym!(lib, "bd_seek_chapter"),
            current_chapter: sym!(lib, "bd_get_current_chapter"),
            playlist_info: sym!(lib, "bd_get_playlist_info"),
            free_title_info: sym!(lib, "bd_free_title_info"),
            _lib: lib,
        })
    }
}

fn dvd_lib() -> Result<&'static DvdLib, String> {
    static L: OnceLock<Result<DvdLib, String>> = OnceLock::new();
    L.get_or_init(DvdLib::load).as_ref().map_err(|e| e.clone())
}

fn bd_lib() -> Result<&'static BdLib, String> {
    static L: OnceLock<Result<BdLib, String>> = OnceLock::new();
    L.get_or_init(BdLib::load).as_ref().map_err(|e| e.clone())
}

/// Whether menus can be used for this kind of disc on this computer.
#[allow(dead_code)]
pub fn available(kind: NavKind) -> Result<(), String> {
    match kind {
        NavKind::Dvd => dvd_lib().map(|_| ()),
        NavKind::Bluray => bd_lib().map(|_| ()),
    }
}

// ---------------------------------------------------------------- colour helpers

fn ycrcb_to_rgb(y: u8, cr: u8, cb: u8, hd: bool) -> [u8; 3] {
    let y = (y as f32 - 16.0) * 1.164;
    let cr = cr as f32 - 128.0;
    let cb = cb as f32 - 128.0;
    let (r, g, b) = if hd {
        (y + 1.793 * cr, y - 0.213 * cb - 0.533 * cr, y + 2.112 * cb)
    } else {
        (y + 1.596 * cr, y - 0.392 * cb - 0.813 * cr, y + 2.017 * cb)
    };
    [r.clamp(0.0, 255.0) as u8, g.clamp(0.0, 255.0) as u8, b.clamp(0.0, 255.0) as u8]
}

// ---------------------------------------------------------------- DVD sub-pictures

#[derive(Clone, Default)]
struct SpuPic {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    idx: Vec<u8>,
    colors: [u8; 4],
    alpha: [u8; 4],
    forced: bool,
    shown_at: Option<Instant>,
    stop_after: Option<Duration>,
}

struct NibbleReader<'a> {
    data: &'a [u8],
    pos: usize, // in nibbles
}

impl NibbleReader<'_> {
    fn next(&mut self) -> u32 {
        let b = self.data.get(self.pos / 2).copied().unwrap_or(0);
        let v = if self.pos % 2 == 0 { b >> 4 } else { b & 0x0f };
        self.pos += 1;
        v as u32
    }
    fn align(&mut self) {
        if self.pos % 2 == 1 {
            self.pos += 1;
        }
    }
}

fn decode_spu(pkt: &[u8]) -> Option<SpuPic> {
    if pkt.len() < 4 {
        return None;
    }
    let size = u16::from_be_bytes([pkt[0], pkt[1]]) as usize;
    let mut ctrl = u16::from_be_bytes([pkt[2], pkt[3]]) as usize;
    if size > pkt.len() || ctrl >= size {
        return None;
    }
    let mut pic = SpuPic { alpha: [0, 15, 15, 15], colors: [0, 1, 2, 3], ..Default::default() };
    let (mut x1, mut x2, mut y1, mut y2) = (0usize, 0usize, 0usize, 0usize);
    let (mut off_top, mut off_bot) = (0usize, 0usize);
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 64 || ctrl + 4 > size {
            break;
        }
        let date = u16::from_be_bytes([pkt[ctrl], pkt[ctrl + 1]]) as u64;
        let next = u16::from_be_bytes([pkt[ctrl + 2], pkt[ctrl + 3]]) as usize;
        let mut i = ctrl + 4;
        while i < size {
            let cmd = pkt[i];
            i += 1;
            match cmd {
                0x00 => pic.forced = true,
                0x01 => {}
                0x02 => pic.stop_after = Some(Duration::from_micros(date * 1024 * 1_000_000 / 90_000)),
                0x03 if i + 2 <= size => {
                    pic.colors = [pkt[i + 1] & 0x0f, pkt[i + 1] >> 4, pkt[i] & 0x0f, pkt[i] >> 4];
                    i += 2;
                }
                0x04 if i + 2 <= size => {
                    pic.alpha = [pkt[i + 1] & 0x0f, pkt[i + 1] >> 4, pkt[i] & 0x0f, pkt[i] >> 4];
                    i += 2;
                }
                0x05 if i + 6 <= size => {
                    let b = &pkt[i..i + 6];
                    x1 = ((b[0] as usize) << 4) | (b[1] as usize >> 4);
                    x2 = (((b[1] & 0x0f) as usize) << 8) | b[2] as usize;
                    y1 = ((b[3] as usize) << 4) | (b[4] as usize >> 4);
                    y2 = (((b[4] & 0x0f) as usize) << 8) | b[5] as usize;
                    i += 6;
                }
                0x06 if i + 4 <= size => {
                    off_top = u16::from_be_bytes([pkt[i], pkt[i + 1]]) as usize;
                    off_bot = u16::from_be_bytes([pkt[i + 2], pkt[i + 3]]) as usize;
                    i += 4;
                }
                0x07 if i + 2 <= size => {
                    let l = u16::from_be_bytes([pkt[i], pkt[i + 1]]) as usize;
                    i += l.max(2);
                }
                0xff => break,
                _ => break,
            }
        }
        if next == ctrl {
            break;
        }
        ctrl = next;
    }
    if x2 < x1 || y2 < y1 || off_top == 0 {
        return None;
    }
    let w = x2 - x1 + 1;
    let h = y2 - y1 + 1;
    if w > 720 || h > 576 {
        return None;
    }
    let mut idx = vec![0u8; w * h];
    for (field, start) in [(0usize, off_top), (1usize, off_bot)] {
        let mut rd = NibbleReader { data: &pkt[..size.min(pkt.len())], pos: start * 2 };
        let mut y = field;
        while y < h {
            let mut x = 0;
            while x < w {
                let mut v = rd.next();
                if v < 0x4 {
                    v = (v << 4) | rd.next();
                    if v < 0x10 {
                        v = (v << 4) | rd.next();
                        if v < 0x40 {
                            v = (v << 4) | rd.next();
                        }
                    }
                }
                let color = (v & 3) as u8;
                let mut run = (v >> 2) as usize;
                if run == 0 {
                    run = w - x;
                }
                let end = (x + run).min(w);
                idx[y * w + x..y * w + end].fill(color);
                x = end;
                if rd.pos / 2 >= size {
                    break;
                }
            }
            rd.align();
            y += 2;
        }
    }
    pic.x = x1;
    pic.y = y1;
    pic.w = w;
    pic.h = h;
    pic.idx = idx;
    Some(pic)
}

// ---------------------------------------------------------------- DVD backend

const DVD_W: usize = 720;
const DVD_H: usize = 576;

struct Dvd {
    lib: &'static DvdLib,
    nav: P,
    clut: [u32; 16],
    spu_id: i32,
    spu_buf: Vec<u8>,
    spu: Option<SpuPic>,
    highlight: Option<HlArea>,
    in_vts: bool,
    still: Option<(Option<Instant>, bool)>,
    last_pad: Instant,
}

enum Step {
    Data,
    Again,
    Wait,
    Eof,
}

impl Dvd {
    fn open(device: &str, lang: &str) -> Result<Dvd, String> {
        let lib = dvd_lib()?;
        let mut nav: P = std::ptr::null_mut();
        let c = CString::new(device).map_err(|e| e.to_string())?;
        if unsafe { (lib.open)(&mut nav, c.as_ptr()) } != 1 || nav.is_null() {
            return Err("the DVD navigator could not open this disc".into());
        }
        unsafe {
            (lib.readahead)(nav, 1);
            (lib.pgc_pos)(nav, 1);
            let l = CString::new(lang).unwrap_or_default();
            (lib.menu_lang)(nav, l.as_ptr() as *mut c_char);
            (lib.audio_lang)(nav, l.as_ptr() as *mut c_char);
            (lib.spu_lang)(nav, l.as_ptr() as *mut c_char);
        }
        Ok(Dvd {
            lib,
            nav,
            clut: [0x108080; 16],
            spu_id: 0x20,
            spu_buf: Vec::new(),
            spu: None,
            highlight: None,
            in_vts: false,
            still: None,
            last_pad: Instant::now(),
        })
    }

    fn step(&mut self, out: &mut Vec<u8>, shared: &NavShared) -> Result<Step, String> {
        if let Some((deadline, injected)) = self.still.as_mut() {
            if !*injected {
                // A video "sequence end" makes the decoder show the still picture now.
                out.extend_from_slice(&[0, 0, 1, 0xE0, 0, 7, 0x81, 0, 0, 0, 0, 1, 0xB7]);
                *injected = true;
                return Ok(Step::Data);
            }
            if let Some(d) = deadline {
                if Instant::now() >= *d {
                    unsafe { (self.lib.still_skip)(self.nav) };
                    self.still = None;
                    return Ok(Step::Again);
                }
            }
            return Ok(self.pad(out));
        }
        let mut block = [0u8; 2048];
        let (mut ev, mut len) = (0i32, 0i32);
        if unsafe { (self.lib.next_block)(self.nav, block.as_mut_ptr(), &mut ev, &mut len) } != 1 {
            return Err("the DVD could not be read".into());
        }
        let len = (len.max(0) as usize).min(2048);
        match ev {
            0 => {
                self.parse_spu(&block[..len], shared);
                out.extend_from_slice(&block[..len]);
                Ok(Step::Data)
            }
            2 => {
                let secs = i32::from_ne_bytes([block[0], block[1], block[2], block[3]]);
                let deadline = if secs == 0xff { None } else { Some(Instant::now() + Duration::from_secs(secs.max(0) as u64)) };
                self.still = Some((deadline, false));
                Ok(Step::Again)
            }
            3 => {
                let phys = i32::from_ne_bytes([block[0], block[1], block[2], block[3]]);
                self.spu_id = if phys < 0 || phys & 0x80 != 0 { -1 } else { 0x20 | (phys & 0x1f) };
                Ok(Step::Again)
            }
            5 => {
                self.spu = None;
                self.render(shared);
                Ok(Step::Again)
            }
            6 => {
                self.in_vts = unsafe { (self.lib.is_vts)(self.nav) } == 1;
                self.update_highlight(shared);
                Ok(Step::Again)
            }
            7 => {
                out.extend_from_slice(&block[..len]);
                self.update_highlight(shared);
                Ok(Step::Data)
            }
            8 => Ok(Step::Eof),
            9 => {
                self.update_highlight(shared);
                Ok(Step::Again)
            }
            10 => {
                for i in 0..16 {
                    let b = &block[i * 4..i * 4 + 4];
                    self.clut[i] = u32::from_ne_bytes([b[0], b[1], b[2], b[3]]);
                }
                self.render(shared);
                Ok(Step::Again)
            }
            12 => {
                self.spu = None;
                self.spu_buf.clear();
                self.render(shared);
                Ok(Step::Again)
            }
            13 => {
                unsafe { (self.lib.wait_skip)(self.nav) };
                Ok(Step::Again)
            }
            _ => Ok(Step::Again),
        }
    }

    /// While a still picture waits for the viewer, send padding so the engine keeps going.
    fn pad(&mut self, out: &mut Vec<u8>) -> Step {
        if self.last_pad.elapsed() < Duration::from_millis(120) {
            return Step::Wait;
        }
        self.last_pad = Instant::now();
        let mut p = vec![0xFFu8; 2048];
        p[..14].copy_from_slice(&[0, 0, 1, 0xBA, 0x44, 0, 4, 0, 4, 1, 0x01, 0x89, 0xC3, 0xF8]);
        p[14..20].copy_from_slice(&[0, 0, 1, 0xBE, ((2048 - 20) >> 8) as u8, ((2048 - 20) & 0xff) as u8]);
        out.extend_from_slice(&p);
        Step::Data
    }

    fn parse_spu(&mut self, block: &[u8], shared: &NavShared) {
        if block.len() < 14 || block[..4] != [0, 0, 1, 0xBA] {
            return;
        }
        let mut pos = 14 + (block[13] & 7) as usize;
        while pos + 6 <= block.len() {
            if block[pos..pos + 3] != [0, 0, 1] {
                break;
            }
            let id = block[pos + 3];
            let plen = u16::from_be_bytes([block[pos + 4], block[pos + 5]]) as usize;
            let end = (pos + 6 + plen).min(block.len());
            if id == 0xBD && pos + 9 < end {
                let hdr = block[pos + 8] as usize;
                let data = pos + 9 + hdr;
                if data < end {
                    let sub = block[data] as i32;
                    if (0x20..0x40).contains(&sub) && sub == self.spu_id {
                        self.spu_buf.extend_from_slice(&block[data + 1..end]);
                        if self.spu_buf.len() >= 2 {
                            let need = u16::from_be_bytes([self.spu_buf[0], self.spu_buf[1]]) as usize;
                            if need > 0 && self.spu_buf.len() >= need {
                                let pkt = std::mem::take(&mut self.spu_buf);
                                if let Some(mut pic) = decode_spu(&pkt[..need]) {
                                    pic.shown_at = Some(Instant::now());
                                    self.spu = Some(pic);
                                    self.render(shared);
                                }
                            } else if need == 0 {
                                self.spu_buf.clear();
                            }
                        }
                    }
                }
            }
            if id == 0xB9 {
                break;
            }
            pos = end;
        }
    }

    fn update_highlight(&mut self, shared: &NavShared) {
        let mut btn = 0i32;
        let pci = unsafe { (self.lib.get_pci)(self.nav) };
        let mut hl = HlArea::default();
        self.highlight = if !pci.is_null()
            && unsafe { (self.lib.cur_highlight)(self.nav, &mut btn) } == 1
            && btn > 0
            && unsafe { (self.lib.hl_area)(pci, btn, 0, &mut hl) } == 1
            && hl.ex > hl.sx
        {
            Some(hl)
        } else {
            None
        };
        shared.in_menu.store(self.highlight.is_some(), Ordering::Release);
        self.render(shared);
    }

    fn color(&self, ci: u8, a: u8) -> [u8; 4] {
        let c = self.clut[(ci & 15) as usize];
        let rgb = ycrcb_to_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8, false);
        [rgb[0], rgb[1], rgb[2], (a & 15) * 17]
    }

    fn render(&mut self, shared: &NavShared) {
        let mut ov = shared.overlay.lock().unwrap();
        ov.width = DVD_W;
        ov.height = DVD_H;
        ov.frame_w = DVD_W;
        ov.frame_h = 0;
        ov.rgba.clear();
        ov.rgba.resize(DVD_W * DVD_H * 4, 0);
        ov.expires = None;
        let show_subs = shared.show_subs.load(Ordering::Acquire);
        if let Some(pic) = &self.spu {
            let visible = !self.in_vts || self.highlight.is_some() || pic.forced || show_subs;
            if visible {
                if let (Some(at), Some(stop)) = (pic.shown_at, pic.stop_after) {
                    ov.expires = Some(at + stop);
                }
                let hl = self.highlight;
                for yy in 0..pic.h {
                    let y = pic.y + yy;
                    if y >= DVD_H {
                        break;
                    }
                    for xx in 0..pic.w {
                        let x = pic.x + xx;
                        if x >= DVD_W {
                            break;
                        }
                        let i = pic.idx[yy * pic.w + xx] as usize;
                        let (ci, a) = match hl {
                            Some(h) if x >= h.sx as usize && x <= h.ex as usize && y >= h.sy as usize && y <= h.ey as usize => {
                                (((h.palette >> (16 + i * 4)) & 0x0f) as u8, ((h.palette >> (i * 4)) & 0x0f) as u8)
                            }
                            _ => (pic.colors[i], pic.alpha[i]),
                        };
                        if a == 0 {
                            continue;
                        }
                        let px = self.color(ci, a);
                        let o = (y * DVD_W + x) * 4;
                        ov.rgba[o..o + 4].copy_from_slice(&px);
                    }
                }
            }
        } else if let Some(h) = self.highlight {
            // No menu picture to recolour: outline the selected button instead.
            let (sx, sy, ex, ey) = (h.sx as usize, h.sy as usize, (h.ex as usize).min(DVD_W - 1), (h.ey as usize).min(DVD_H - 1));
            for y in sy..=ey {
                for x in sx..=ex {
                    let edge = y < sy + 3 || y + 3 > ey || x < sx + 3 || x + 3 > ex;
                    let o = (y * DVD_W + x) * 4;
                    let px = if edge { [255, 255, 255, 230] } else { [255, 255, 255, 40] };
                    ov.rgba[o..o + 4].copy_from_slice(&px);
                }
            }
        }
        ov.generation += 1;
    }

    fn input(&mut self, i: NavInput, shared: &NavShared) {
        let lib = self.lib;
        let pci = unsafe { (lib.get_pci)(self.nav) };
        let mut restart = false;
        unsafe {
            match i {
                NavInput::Up if !pci.is_null() => {
                    (lib.up)(self.nav, pci);
                }
                NavInput::Down if !pci.is_null() => {
                    (lib.down)(self.nav, pci);
                }
                NavInput::Left if !pci.is_null() => {
                    (lib.left)(self.nav, pci);
                }
                NavInput::Right if !pci.is_null() => {
                    (lib.right)(self.nav, pci);
                }
                NavInput::Enter if !pci.is_null() => {
                    (lib.activate)(self.nav, pci);
                    restart = true;
                }
                NavInput::MouseMove(x, y) if !pci.is_null() => {
                    (lib.mouse_select)(self.nav, pci, x as i32, y as i32);
                }
                NavInput::MouseClick(x, y) if !pci.is_null() => {
                    if (lib.mouse_activate)(self.nav, pci, x as i32, y as i32) == 1 {
                        restart = true;
                    }
                }
                NavInput::Menu | NavInput::Popup => {
                    if (lib.menu_call)(self.nav, 3) != 1 {
                        (lib.menu_call)(self.nav, 2);
                    }
                    restart = true;
                }
                NavInput::Chapter(d) => {
                    if d > 0 {
                        (lib.next_pg)(self.nav);
                    } else {
                        (lib.prev_pg)(self.nav);
                    }
                    restart = true;
                }
                NavInput::SeekFraction(f) => {
                    let (mut pos, mut len) = (0u32, 0u32);
                    if (lib.get_position)(self.nav, &mut pos, &mut len) == 1 && len > 0 {
                        (lib.sector_search)(self.nav, (f.clamp(0.0, 0.999) * len as f64) as i64, 0);
                        restart = true;
                    }
                }
                NavInput::Skip(secs) => {
                    // Roughly 0.5 MB per second of DVD video.
                    let (mut pos, mut len) = (0u32, 0u32);
                    if (lib.get_position)(self.nav, &mut pos, &mut len) == 1 && len > 0 {
                        let target = (pos as f64 + secs * 250.0).clamp(0.0, len as f64 - 1.0);
                        (lib.sector_search)(self.nav, target as i64, 0);
                        restart = true;
                    }
                }
                _ => {}
            }
        }
        if restart {
            self.still = None;
            self.spu_buf.clear();
            shared.restart.store(true, Ordering::Release);
        }
        self.update_highlight(shared);
    }

    fn progress(&self) -> (f64, f64) {
        let (mut pos, mut len) = (0u32, 0u32);
        if unsafe { (self.lib.get_position)(self.nav, &mut pos, &mut len) } == 1 && len > 0 {
            (pos as f64 / len as f64, 1.0)
        } else {
            (0.0, 0.0)
        }
    }
}

impl Drop for Dvd {
    fn drop(&mut self) {
        unsafe { (self.lib.close)(self.nav) };
    }
}

// ---------------------------------------------------------------- Blu-ray backend

struct Plane {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    rgba: Vec<u8>,
}

struct BdOverlayCtx {
    planes: Mutex<[Option<Plane>; 2]>,
    shared: Arc<NavShared>,
}

unsafe extern "C" fn bd_overlay_cb(handle: P, ov: *const BdOverlay) {
    if handle.is_null() {
        return;
    }
    let ctx = &*(handle as *const BdOverlayCtx);
    let mut planes = ctx.planes.lock().unwrap();
    if ov.is_null() {
        *planes = [None, None];
        publish(&planes, &ctx.shared);
        return;
    }
    let ov = &*ov;
    let pi = (ov.plane as usize).min(1);
    match ov.cmd {
        0 => {
            let (w, h) = (ov.w as usize, ov.h as usize);
            planes[pi] = Some(Plane { x: ov.x as usize, y: ov.y as usize, w, h, rgba: vec![0; w * h * 4] });
        }
        1 => planes[pi] = None,
        2 | 5 => {
            if let Some(p) = planes[pi].as_mut() {
                p.rgba.fill(0);
            }
        }
        3 => {
            if let (Some(p), false, false) = (planes[pi].as_mut(), ov.img.is_null(), ov.palette.is_null()) {
                let pal = std::slice::from_raw_parts(ov.palette, 256);
                let (ox, oy, w, h) = (ov.x as usize, ov.y as usize, ov.w as usize, ov.h as usize);
                let mut rle = ov.img;
                let (mut x, mut y) = (0usize, 0usize);
                let mut wrapped = false;
                let mut guard = 0usize;
                while y < h && guard < 8_000_000 {
                    guard += 1;
                    let e = *rle;
                    rle = rle.add(1);
                    if e.len == 0 {
                        if !wrapped {
                            x = 0;
                            y += 1;
                        }
                        wrapped = false;
                        continue;
                    }
                    if wrapped {
                        wrapped = false;
                    }
                    let entry = pal[(e.color & 0xff) as usize];
                    let (yy, cr, cb, t) = (entry[0], entry[1], entry[2], entry[3]);
                    let rgb = ycrcb_to_rgb(yy, cr, cb, true);
                    let end = (x + e.len as usize).min(w);
                    let py = oy + y;
                    if py < p.h && t > 0 {
                        for xx in x..end {
                            let px = ox + xx;
                            if px < p.w {
                                let o = (py * p.w + px) * 4;
                                p.rgba[o..o + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], t]);
                            }
                        }
                    }
                    x = end;
                    if x >= w {
                        x = 0;
                        y += 1;
                        wrapped = true;
                    }
                }
            }
        }
        4 => {
            if let Some(p) = planes[pi].as_mut() {
                for yy in ov.y as usize..(ov.y as usize + ov.h as usize).min(p.h) {
                    for xx in ov.x as usize..(ov.x as usize + ov.w as usize).min(p.w) {
                        let o = (yy * p.w + xx) * 4;
                        p.rgba[o..o + 4].fill(0);
                    }
                }
            }
        }
        6 => publish(&planes, &ctx.shared),
        _ => {}
    }
}

/// Combine subtitles (plane 0) and menus (plane 1) into one picture for the screen.
fn publish(planes: &[Option<Plane>; 2], shared: &NavShared) {
    let (fw, fh) = planes.iter().flatten().map(|p| (p.x + p.w, p.y + p.h)).fold((0, 0), |a, b| (a.0.max(b.0), a.1.max(b.1)));
    let mut ov = shared.overlay.lock().unwrap();
    ov.width = fw;
    ov.height = fh;
    ov.frame_w = fw;
    ov.frame_h = fh;
    ov.expires = None;
    ov.rgba.clear();
    ov.rgba.resize(fw * fh * 4, 0);
    for p in planes.iter().flatten() {
        for y in 0..p.h {
            for x in 0..p.w {
                let s = (y * p.w + x) * 4;
                let a = p.rgba[s + 3];
                if a == 0 {
                    continue;
                }
                let d = ((p.y + y) * fw + p.x + x) * 4;
                ov.rgba[d..d + 4].copy_from_slice(&p.rgba[s..s + 4]);
            }
        }
    }
    ov.generation += 1;
}

struct Bd {
    lib: &'static BdLib,
    bd: P,
    ctx: *mut BdOverlayCtx,
    still: bool,
    still_deadline: Option<Instant>,
    last_pad: Instant,
    duration: f64,
}

impl Bd {
    fn open(device: &str, lang: &str, shared: Arc<NavShared>) -> Result<Bd, String> {
        let lib = bd_lib()?;
        let c = CString::new(device).map_err(|e| e.to_string())?;
        let bd = unsafe { (lib.open)(c.as_ptr(), std::ptr::null()) };
        if bd.is_null() {
            return Err("the Blu-ray navigator could not open this disc".into());
        }
        let ctx = Box::into_raw(Box::new(BdOverlayCtx { planes: Mutex::new([None, None]), shared }));
        unsafe {
            (lib.set_setting)(bd, 0x100, 1); // decode subtitles into the overlay
            let l = CString::new(lang).unwrap_or_default();
            for idx in [16u32, 17, 18] {
                (lib.set_setting_str)(bd, idx, l.as_ptr());
            }
            (lib.register_overlay)(bd, ctx as P, Some(bd_overlay_cb));
            if (lib.play)(bd) == 0 {
                (lib.register_overlay)(bd, std::ptr::null_mut(), None);
                (lib.close)(bd);
                drop(Box::from_raw(ctx));
                return Err("this disc's menus could not be started".into());
            }
        }
        Ok(Bd { lib, bd, ctx, still: false, still_deadline: None, last_pad: Instant::now(), duration: 0.0 })
    }

    fn step(&mut self, out: &mut Vec<u8>, shared: &NavShared) -> Result<Step, String> {
        if self.still {
            if let Some(d) = self.still_deadline {
                if Instant::now() >= d {
                    unsafe { (self.lib.skip_still)(self.bd) };
                    self.still = false;
                    self.still_deadline = None;
                }
            }
        }
        let mut buf = vec![0u8; 6144 * 8];
        let mut ev = BdEvent::default();
        let n = unsafe { (self.lib.read_ext)(self.bd, buf.as_mut_ptr(), buf.len() as c_int, &mut ev) };
        if n < 0 {
            return Err("the Blu-ray could not be read".into());
        }
        match ev.event {
            1 => {
                return Err(if ev.param == 2 {
                    "this disc's menus need Java (BD-J), which isn't supported".into()
                } else {
                    "the disc's menu program stopped with an error".into()
                })
            }
            2 => return Err("the Blu-ray could not be read".into()),
            3 => return Err("this Blu-ray is copy-protected and couldn't be unlocked".into()),
            6 => {
                let info = unsafe { (self.lib.playlist_info)(self.bd, ev.param, 0) };
                if !info.is_null() {
                    self.duration = unsafe { (*info).duration } as f64 / 90_000.0;
                    unsafe { (self.lib.free_title_info)(info) };
                }
            }
            25 => {
                self.still = ev.param != 0;
                if !self.still {
                    self.still_deadline = None;
                }
            }
            26 => {
                self.still = true;
                self.still_deadline = if ev.param == 0 { None } else { Some(Instant::now() + Duration::from_secs(ev.param as u64)) };
            }
            29 => shared.popup.store(ev.param != 0, Ordering::Release),
            30 => shared.in_menu.store(ev.param != 0, Ordering::Release),
            _ => {}
        }
        if n > 0 {
            out.extend_from_slice(&buf[..n as usize]);
            return Ok(Step::Data);
        }
        if ev.event != 0 {
            return Ok(Step::Again);
        }
        // Nothing to read right now (still picture, menu waiting or idle): keep the engine fed.
        if self.last_pad.elapsed() < Duration::from_millis(120) {
            return Ok(Step::Wait);
        }
        self.last_pad = Instant::now();
        for _ in 0..32 {
            let mut pkt = [0xFFu8; 192];
            pkt[..8].copy_from_slice(&[0, 0, 0, 0, 0x47, 0x1F, 0xFF, 0x10]);
            out.extend_from_slice(&pkt);
        }
        Ok(Step::Data)
    }

    fn input(&mut self, i: NavInput, shared: &NavShared) {
        let lib = self.lib;
        let mut restart = false;
        unsafe {
            match i {
                NavInput::Up => {
                    (lib.user_input)(self.bd, -1, 12);
                }
                NavInput::Down => {
                    (lib.user_input)(self.bd, -1, 13);
                }
                NavInput::Left => {
                    (lib.user_input)(self.bd, -1, 14);
                }
                NavInput::Right => {
                    (lib.user_input)(self.bd, -1, 15);
                }
                NavInput::Enter => {
                    (lib.user_input)(self.bd, -1, 16);
                    restart = true;
                }
                NavInput::Menu => {
                    (lib.menu_call)(self.bd, -1);
                    restart = true;
                }
                NavInput::Popup => {
                    (lib.user_input)(self.bd, -1, 11);
                }
                NavInput::MouseMove(x, y) => {
                    (lib.mouse_select)(self.bd, -1, x.max(0.0) as u16, y.max(0.0) as u16);
                }
                NavInput::MouseClick(x, y) => {
                    (lib.mouse_select)(self.bd, -1, x.max(0.0) as u16, y.max(0.0) as u16);
                    (lib.user_input)(self.bd, -1, 17);
                    restart = true;
                }
                NavInput::Chapter(d) => {
                    let cur = (lib.current_chapter)(self.bd) as i64;
                    let target = (cur + d as i64).max(1) as u32;
                    (lib.seek_chapter)(self.bd, target - 1);
                    restart = true;
                }
                NavInput::Skip(secs) => {
                    let now = (lib.tell_time)(self.bd) as f64 / 90_000.0;
                    let t = (now + secs).max(0.0);
                    (lib.seek_time)(self.bd, (t * 90_000.0) as u64);
                    restart = true;
                }
                NavInput::SeekFraction(f) => {
                    if self.duration > 0.0 {
                        (lib.seek_time)(self.bd, (f.clamp(0.0, 0.999) * self.duration * 90_000.0) as u64);
                        restart = true;
                    }
                }
            }
        }
        if restart {
            self.still = false;
            self.still_deadline = None;
            shared.restart.store(true, Ordering::Release);
        }
    }

    fn progress(&self) -> (f64, f64) {
        let now = unsafe { (self.lib.tell_time)(self.bd) } as f64 / 90_000.0;
        (now, self.duration)
    }
}

impl Drop for Bd {
    fn drop(&mut self) {
        unsafe {
            (self.lib.register_overlay)(self.bd, std::ptr::null_mut(), None);
            (self.lib.close)(self.bd);
            drop(Box::from_raw(self.ctx));
        }
    }
}

// ---------------------------------------------------------------- session

enum Backend {
    Dvd(Dvd),
    Bd(Bd),
}

// The navigator handles are only used while holding the session lock.
unsafe impl Send for Backend {}

pub struct NavSession {
    pub kind: NavKind,
    backend: Mutex<Backend>,
    pending: Mutex<Vec<u8>>,
    pub shared: Arc<NavShared>,
    pub started: Instant,
}

impl NavSession {
    pub fn open(kind: NavKind, device: &str, lang: &str) -> Result<Arc<NavSession>, String> {
        let shared = Arc::new(NavShared::default());
        let backend = match kind {
            NavKind::Dvd => Backend::Dvd(Dvd::open(device, lang)?),
            NavKind::Bluray => Backend::Bd(Bd::open(device, &iso639_2(lang), shared.clone())?),
        };
        Ok(Arc::new(NavSession { kind, backend: Mutex::new(backend), pending: Mutex::new(Vec::new()), shared, started: Instant::now() }))
    }

    /// Fill `buf` for the engine. Blocks while the disc waits (menus, stills).
    pub fn read(&self, buf: &mut [u8], cancel: &AtomicBool) -> i64 {
        let mut spins = 0u32;
        loop {
            if cancel.load(Ordering::Acquire) {
                return -1;
            }
            {
                let mut pending = self.pending.lock().unwrap();
                if !pending.is_empty() {
                    let n = buf.len().min(pending.len());
                    buf[..n].copy_from_slice(&pending[..n]);
                    pending.drain(..n);
                    self.shared.bytes.fetch_add(n as u64, Ordering::Relaxed);
                    return n as i64;
                }
                let mut be = self.backend.lock().unwrap();
                let r = match &mut *be {
                    Backend::Dvd(d) => d.step(&mut pending, &self.shared),
                    Backend::Bd(b) => b.step(&mut pending, &self.shared),
                };
                match r {
                    Ok(Step::Data) | Ok(Step::Again) => {
                        spins += 1;
                        if spins < 10_000 {
                            continue;
                        }
                        spins = 0;
                    }
                    Ok(Step::Wait) => {}
                    Ok(Step::Eof) => return 0,
                    Err(e) => {
                        self.shared.fail(e);
                        return -1;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(15));
        }
    }

    pub fn input(&self, i: NavInput) {
        let mut be = self.backend.lock().unwrap();
        match &mut *be {
            Backend::Dvd(d) => d.input(i, &self.shared),
            Backend::Bd(b) => b.input(i, &self.shared),
        }
        if self.shared.restart.load(Ordering::Acquire) {
            self.pending.lock().unwrap().clear();
        }
    }

    pub fn set_show_subs(&self, on: bool) {
        self.shared.show_subs.store(on, Ordering::Release);
        if let Backend::Dvd(d) = &mut *self.backend.lock().unwrap() {
            d.render(&self.shared);
        }
    }

    /// DVD: (fraction, 1.0). Blu-ray: (seconds, total seconds).
    pub fn progress(&self) -> (f64, f64) {
        match self.backend.try_lock() {
            Ok(be) => {
                let p = match &*be {
                    Backend::Dvd(d) => d.progress(),
                    Backend::Bd(b) => b.progress(),
                };
                *self.shared.progress.lock().unwrap() = p;
                p
            }
            Err(_) => *self.shared.progress.lock().unwrap(),
        }
    }
}

fn iso639_2(two: &str) -> String {
    match two {
        "en" => "eng",
        "fr" => "fre",
        "de" => "ger",
        "es" => "spa",
        "it" => "ita",
        "ja" => "jpn",
        "zh" => "chi",
        "ko" => "kor",
        "pt" => "por",
        "ru" => "rus",
        "nl" => "dut",
        "sv" => "swe",
        "da" => "dan",
        "no" => "nor",
        "fi" => "fin",
        "pl" => "pol",
        other if other.len() == 3 => other,
        _ => "eng",
    }
    .to_string()
}

// ---------------------------------------------------------------- engine stream hookup

static CURRENT: Mutex<Option<Arc<NavSession>>> = Mutex::new(None);

pub fn set_current(s: Option<Arc<NavSession>>) {
    *CURRENT.lock().unwrap() = s;
}

pub struct StreamCookie {
    session: Arc<NavSession>,
    cancel: AtomicBool,
}

/// Called by the engine when it opens "discnav://…".
pub fn open_stream() -> Option<*mut StreamCookie> {
    let s = CURRENT.lock().unwrap().clone()?;
    Some(Box::into_raw(Box::new(StreamCookie { session: s, cancel: AtomicBool::new(false) })))
}

pub unsafe fn stream_read(cookie: *mut StreamCookie, buf: &mut [u8]) -> i64 {
    let c = &*cookie;
    c.session.read(buf, &c.cancel)
}

pub unsafe fn stream_cancel(cookie: *mut StreamCookie) {
    (*cookie).cancel.store(true, Ordering::Release);
}

pub unsafe fn stream_close(cookie: *mut StreamCookie) {
    drop(Box::from_raw(cookie));
}
