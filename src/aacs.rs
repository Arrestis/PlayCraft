//! Finds the open-source libaacs library (VideoLAN) that libbluray uses for protected
//! Blu-ray discs, and reads its version. libaacs ships no keys or certificates.

use std::ffi::c_int;
use std::path::PathBuf;
use std::sync::OnceLock;

pub const PROJECT_URL: &str = "https://www.videolan.org/developers/libaacs.html";
pub const SOURCE_URL: &str = "https://code.videolan.org/videolan/libaacs";

#[derive(Clone, Debug)]
pub struct AacsInfo {
    pub version: String,
    pub path: String,
}

fn candidates() -> Vec<PathBuf> {
    let mut c = Vec::new();
    let ext = if cfg!(windows) { "dll" } else if cfg!(target_os = "macos") { "dylib" } else { "so" };
    if let Some(p) = std::env::var_os("LIBAACS_PATH") {
        let p = PathBuf::from(p);
        c.push(p.with_extension(ext));
        c.push(p);
    }
    if let Some(d) = crate::embed::dir() {
        c.push(d.join(format!("libaacs.{ext}")));
    }
    if let Some(d) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("engine"))) {
        c.push(d.join(format!("libaacs.{ext}")));
    }
    c.push(crate::library::engine_dir().join(format!("libaacs.{ext}")));
    let names: &[&str] = if cfg!(windows) {
        &["libaacs.dll", "libaacs-0.dll"]
    } else if cfg!(target_os = "macos") {
        &["libaacs.0.dylib", "libaacs.dylib", "/opt/homebrew/lib/libaacs.dylib", "/usr/local/lib/libaacs.dylib"]
    } else {
        &["libaacs.so.0", "libaacs.so"]
    };
    c.extend(names.iter().map(PathBuf::from));
    c
}

/// The libaacs the player will use, if any. Looked up once.
pub fn info() -> Option<AacsInfo> {
    static INFO: OnceLock<Option<AacsInfo>> = OnceLock::new();
    INFO.get_or_init(|| {
        for p in candidates() {
            let is_bare_name = p.components().count() == 1;
            if !is_bare_name && !p.exists() {
                continue;
            }
            // Loading only reads the library's version; nothing is decrypted here.
            let Ok(lib) = (unsafe { libloading::Library::new(&p) }) else { continue };
            let version = unsafe {
                lib.get::<unsafe extern "C" fn(*mut c_int, *mut c_int, *mut c_int)>(b"aacs_get_version\0")
                    .ok()
                    .map(|f| {
                        let (mut a, mut b, mut c) = (0, 0, 0);
                        f(&mut a, &mut b, &mut c);
                        format!("{a}.{b}.{c}")
                    })
            };
            // Keep it loaded for the life of the program (libbluray uses it too).
            std::mem::forget(lib);
            return Some(AacsInfo { version: version.unwrap_or_else(|| "unknown".into()), path: p.display().to_string() });
        }
        None
    })
    .clone()
}
