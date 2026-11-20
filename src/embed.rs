//! The playback engine built into playcraft.exe (Windows release builds).
//!
//! build.rs packs the engine libraries (libmpv with FFmpeg, libbluray, libdvdnav, libaacs…)
//! into the program itself, so Windows users get a single file. On first start they are
//! unpacked to `%LOCALAPPDATA%\PlayCraft\runtime\<processor>-<id>\`. The folder name changes
//! whenever the engine does, so a copy for another version or processor is never used.

use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

static PACK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/engine.pack"));
static PACK_ID: &str = include_str!(concat!(env!("OUT_DIR"), "/engine.id"));

static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Folder holding the unpacked built-in engine, if this build has one.
pub fn dir() -> Option<PathBuf> {
    DIR.get().cloned().flatten()
}

fn processor() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "x64"
    }
}

/// Unpack the built-in engine (only the first time) and remember where it is.
pub fn prepare() {
    let result = unpack();
    match &result {
        Ok(Some(d)) => crate::log::info(format!("Built-in engine ready in {}", d.display())),
        Ok(None) => {}
        Err(e) => crate::log::error(format!("Could not unpack the built-in engine: {e}")),
    }
    let _ = DIR.set(result.ok().flatten());
}

fn unpack() -> Result<Option<PathBuf>, String> {
    if PACK.len() < 10 || &PACK[..6] != b"PCENG1" {
        return Ok(None);
    }
    let root = crate::library::data_dir().join("runtime");
    let name = format!("{}-{}", processor(), PACK_ID.trim());
    let dir = root.join(&name);
    if dir.join(".complete").exists() {
        return Ok(Some(dir));
    }
    // Unpack into a temporary folder, then rename, so a half-finished copy is never used.
    let tmp = root.join(format!("{name}.partial"));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let count = u32::from_le_bytes(PACK[6..10].try_into().unwrap()) as usize;
    let mut pos = 10;
    let take = |pos: &mut usize, n: usize| -> Result<&'static [u8], String> {
        let s = PACK.get(*pos..*pos + n).ok_or("the built-in engine is damaged")?;
        *pos += n;
        Ok(s)
    };
    for _ in 0..count {
        let nlen = u16::from_le_bytes(take(&mut pos, 2)?.try_into().unwrap()) as usize;
        let fname = String::from_utf8_lossy(take(&mut pos, nlen)?).to_string();
        let size = u64::from_le_bytes(take(&mut pos, 8)?.try_into().unwrap()) as usize;
        let plen = u64::from_le_bytes(take(&mut pos, 8)?.try_into().unwrap()) as usize;
        let packed = take(&mut pos, plen)?;
        if fname.contains(['/', '\\']) || fname.contains("..") {
            return Err(format!("unexpected file name {fname}"));
        }
        let data = miniz_oxide::inflate::decompress_to_vec_with_limit(packed, size.max(1))
            .map_err(|e| format!("{fname}: {e:?}"))?;
        let mut f = std::fs::File::create(tmp.join(&fname)).map_err(|e| e.to_string())?;
        f.write_all(&data).map_err(|e| e.to_string())?;
    }
    std::fs::write(tmp.join(".complete"), PACK_ID.trim()).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::rename(&tmp, &dir).map_err(|e| e.to_string())?;
    // Tidy away engines unpacked by older versions.
    if let Ok(rd) = std::fs::read_dir(&root) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            // Only this processor's older copies (a 32-bit and 64-bit PlayCraft can coexist).
            if n != name && n.starts_with(&format!("{}-", processor())) {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    Ok(Some(dir))
}
