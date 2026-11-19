//! Activity log: a plain-text record of what PlayCraft does (start-up, discs found,
//! what was played, disc menus, errors). Saved next to the settings as `playcraft.log`;
//! the previous log is kept as `playcraft.old.log` once it grows past 2 MB.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);
const MAX_BYTES: u64 = 2 * 1024 * 1024;

pub fn path() -> PathBuf {
    crate::library::data_dir().join("playcraft.log")
}

/// Open the log for this run.
pub fn init() {
    let dir = crate::library::data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let p = path();
    if std::fs::metadata(&p).map(|m| m.len() > MAX_BYTES).unwrap_or(false) {
        let _ = std::fs::rename(&p, dir.join("playcraft.old.log"));
    }
    if let Ok(f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        *FILE.lock().unwrap() = Some(f);
    }
    info(format!(
        "PlayCraft {} started on {} {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
}

pub fn info(msg: impl AsRef<str>) {
    write("INFO", msg.as_ref());
}

pub fn warn(msg: impl AsRef<str>) {
    write("WARN", msg.as_ref());
}

pub fn error(msg: impl AsRef<str>) {
    write("ERROR", msg.as_ref());
}

fn write(level: &str, msg: &str) {
    let line = format!("{} {level:5} {msg}\n", timestamp());
    if cfg!(debug_assertions) {
        eprint!("{line}");
    }
    if let Some(f) = FILE.lock().unwrap().as_mut() {
        let _ = f.write_all(line.as_bytes());
        let _ = f.flush();
    }
}

/// "2026-11-20 08:15:02Z" (UTC) without extra libraries.
fn timestamp() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}Z", rem / 3600, (rem / 60) % 60, rem % 60)
}
