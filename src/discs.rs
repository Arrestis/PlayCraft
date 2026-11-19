//! Finding optical drives and working out what kind of disc is inside.

use std::path::{Path, PathBuf};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscKind {
    Empty,
    DvdVideo,
    BluRay,
    UhdBluRay,
    AudioCd,
    Data,
    Unknown,
}

impl DiscKind {
    pub fn label(self) -> &'static str {
        match self {
            DiscKind::Empty => "No disc",
            DiscKind::DvdVideo => "DVD-Video",
            DiscKind::BluRay => "Blu-ray",
            DiscKind::UhdBluRay => "Ultra HD Blu-ray",
            DiscKind::AudioCd => "Audio CD",
            DiscKind::Data => "Data disc",
            DiscKind::Unknown => "Disc",
        }
    }
    pub fn playable(self) -> bool {
        !matches!(self, DiscKind::Empty)
    }
}

#[derive(Clone, Debug)]
pub struct Drive {
    /// Device the engine opens: "D:" on Windows, "/dev/sr0" on Linux, the volume on macOS.
    pub device: String,
    /// Where the disc's files can be read, if mounted.
    pub mount: Option<PathBuf>,
    pub label: String,
    pub kind: DiscKind,
}

impl Drive {
    pub fn name(&self) -> String {
        if !self.label.is_empty() {
            self.label.replace('_', " ")
        } else {
            self.kind.label().to_string()
        }
    }
}

/// Work out what kind of video disc a folder (a disc root, a copied disc folder or a
/// VIDEO_TS / BDMV folder itself) holds.
pub fn classify_folder(path: &Path) -> DiscKind {
    let name = path.file_name().map(|n| n.to_string_lossy().to_uppercase()).unwrap_or_default();
    if name == "VIDEO_TS" {
        return DiscKind::DvdVideo;
    }
    if name == "BDMV" {
        return bd_kind(path);
    }
    if path.join("BDMV").is_dir() {
        return bd_kind(&path.join("BDMV"));
    }
    if path.join("VIDEO_TS").is_dir() || path.join("video_ts").is_dir() {
        return DiscKind::DvdVideo;
    }
    if let Ok(rd) = std::fs::read_dir(path) {
        let mut any = false;
        for e in rd.flatten().take(200) {
            any = true;
            let n = e.file_name().to_string_lossy().to_lowercase();
            if n.ends_with(".cda") || n.ends_with(".aiff") {
                return DiscKind::AudioCd;
            }
        }
        if any {
            return DiscKind::Data;
        }
    }
    DiscKind::Unknown
}

/// A friendly name for a disc folder: the volume label for a drive root, else the folder name.
pub fn source_name(path: &Path) -> String {
    if path.file_name().is_none() {
        #[cfg(windows)]
        {
            use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW;
            let s = path.display().to_string();
            let root: Vec<u16> = format!("{}\\", s.trim_end_matches('\\')).encode_utf16().chain(Some(0)).collect();
            let mut name = [0u16; 261];
            let ok = unsafe {
                GetVolumeInformationW(
                    root.as_ptr(),
                    name.as_mut_ptr(),
                    name.len() as u32,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    0,
                )
            };
            if ok != 0 {
                let label = String::from_utf16_lossy(&name[..name.iter().position(|&c| c == 0).unwrap_or(0)]);
                if !label.is_empty() {
                    return label.replace('_', " ");
                }
            }
        }
        return format!("Disc in {}", path.display());
    }
    crate::library::display_name(path)
}

/// Copy-protected Blu-ray discs carry an AACS folder in their root.
pub fn is_protected(root: &Path) -> bool {
    root.join("AACS").is_dir() || root.join("aacs").is_dir()
}

/// Ultra HD discs mark their index file as version 3.
fn bd_kind(bdmv: &Path) -> DiscKind {
    let idx = bdmv.join("index.bdmv");
    if let Ok(bytes) = std::fs::read(&idx) {
        if bytes.len() >= 8 && &bytes[4..8] == b"0300" {
            return DiscKind::UhdBluRay;
        }
    }
    DiscKind::BluRay
}

/// The disc root to hand the engine for a folder the person picked.
pub fn disc_root(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().to_uppercase()).unwrap_or_default();
    if name == "VIDEO_TS" || name == "BDMV" {
        path.parent().map(Path::to_path_buf).unwrap_or_else(|| path.to_path_buf())
    } else {
        path.to_path_buf()
    }
}

#[cfg(windows)]
pub fn detect_drives() -> Vec<Drive> {
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW};
    let mut out = Vec::new();
    let mask = unsafe { GetLogicalDrives() };
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let letter = (b'A' + i as u8) as char;
        let root: Vec<u16> = format!("{letter}:\\").encode_utf16().chain(Some(0)).collect();
        // 5 = DRIVE_CDROM (CD, DVD and Blu-ray drives)
        if unsafe { GetDriveTypeW(root.as_ptr()) } != 5 {
            continue;
        }
        let mut name = [0u16; 261];
        let mut fs = [0u16; 261];
        let ok = unsafe {
            GetVolumeInformationW(
                root.as_ptr(),
                name.as_mut_ptr(),
                name.len() as u32,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                fs.as_mut_ptr(),
                fs.len() as u32,
            )
        };
        let device = format!("{letter}:");
        if ok == 0 {
            out.push(Drive { device, mount: None, label: String::new(), kind: DiscKind::Empty });
            continue;
        }
        let label = String::from_utf16_lossy(&name[..name.iter().position(|&c| c == 0).unwrap_or(0)]);
        let mount = PathBuf::from(format!("{letter}:\\"));
        let kind = classify_folder(&mount);
        out.push(Drive { device, mount: Some(mount), label, kind });
    }
    out
}

#[cfg(target_os = "linux")]
pub fn detect_drives() -> Vec<Drive> {
    let mounts = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir("/sys/block") else { return out };
    let mut names: Vec<String> = rd
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("sr"))
        .collect();
    names.sort();
    for n in names {
        let device = format!("/dev/{n}");
        let size: u64 = std::fs::read_to_string(format!("/sys/block/{n}/size"))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        let mount = mounts.lines().find_map(|l| {
            let mut it = l.split_whitespace();
            let dev = it.next()?;
            let mp = it.next()?;
            (dev == device).then(|| PathBuf::from(mp.replace("\\040", " ")))
        });
        let (kind, label) = match &mount {
            Some(m) => (classify_folder(m), m.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default()),
            // A disc that is present but not mounted is usually an audio CD.
            None if size > 0 => (DiscKind::Unknown, String::new()),
            None => (DiscKind::Empty, String::new()),
        };
        out.push(Drive { device, mount, label, kind });
    }
    out
}

#[cfg(target_os = "macos")]
pub fn detect_drives() -> Vec<Drive> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir("/Volumes") else { return out };
    for e in rd.flatten() {
        let p = e.path();
        let kind = classify_folder(&p);
        if matches!(kind, DiscKind::DvdVideo | DiscKind::BluRay | DiscKind::UhdBluRay | DiscKind::AudioCd) {
            out.push(Drive {
                device: p.display().to_string(),
                label: e.file_name().to_string_lossy().into_owned(),
                mount: Some(p),
                kind,
            });
        }
    }
    out
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub fn detect_drives() -> Vec<Drive> {
    Vec::new()
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolumeKind {
    Fixed,
    Removable,
    Network,
    Optical,
}

#[derive(Clone, Debug)]
pub struct Volume {
    pub name: String,
    pub path: PathBuf,
    pub kind: VolumeKind,
}

/// Disks and drives shown on the My Computer page.
#[cfg(windows)]
pub fn list_volumes() -> Vec<Volume> {
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW};
    let mut out = Vec::new();
    let mask = unsafe { GetLogicalDrives() };
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let letter = (b'A' + i as u8) as char;
        let root: Vec<u16> = format!("{letter}:\\").encode_utf16().chain(Some(0)).collect();
        let kind = match unsafe { GetDriveTypeW(root.as_ptr()) } {
            2 => VolumeKind::Removable,
            3 => VolumeKind::Fixed,
            4 => VolumeKind::Network,
            5 => VolumeKind::Optical,
            _ => continue,
        };
        let mut name = [0u16; 261];
        let ok = unsafe {
            GetVolumeInformationW(
                root.as_ptr(),
                name.as_mut_ptr(),
                name.len() as u32,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            )
        };
        if ok == 0 && kind == VolumeKind::Optical {
            continue; // empty disc drive
        }
        let label = String::from_utf16_lossy(&name[..name.iter().position(|&c| c == 0).unwrap_or(0)]);
        let label = if !label.is_empty() {
            label
        } else {
            match kind {
                VolumeKind::Fixed => "Local Disk".into(),
                VolumeKind::Removable => "USB Drive".into(),
                VolumeKind::Network => "Network Drive".into(),
                VolumeKind::Optical => "Disc".into(),
            }
        };
        out.push(Volume { name: format!("{label} ({letter}:)"), path: PathBuf::from(format!("{letter}:\\")), kind });
    }
    out
}

#[cfg(not(windows))]
pub fn list_volumes() -> Vec<Volume> {
    let mut out = vec![Volume { name: "Computer".into(), path: PathBuf::from("/"), kind: VolumeKind::Fixed }];
    let mut roots = vec![PathBuf::from("/Volumes"), PathBuf::from("/media"), PathBuf::from("/mnt"), PathBuf::from("/run/media")];
    if let Ok(user) = std::env::var("USER") {
        roots.push(PathBuf::from(format!("/media/{user}")));
        roots.push(PathBuf::from(format!("/run/media/{user}")));
    }
    for r in roots {
        if let Ok(rd) = std::fs::read_dir(&r) {
            for e in rd.flatten() {
                let p = e.path();
                if !p.is_dir() || out.iter().any(|v| v.path == p) {
                    continue;
                }
                let k = classify_folder(&p);
                let kind = if matches!(k, DiscKind::DvdVideo | DiscKind::BluRay | DiscKind::UhdBluRay | DiscKind::AudioCd) {
                    VolumeKind::Optical
                } else {
                    VolumeKind::Removable
                };
                out.push(Volume { name: e.file_name().to_string_lossy().into_owned(), path: p, kind });
            }
        }
    }
    out
}

/// Open the drive tray.
pub fn eject(drive: &Drive) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Media::Multimedia::mciSendStringW;
        let w = |s: String| -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() };
        unsafe {
            let open = w(format!("open {} type cdaudio alias odp_drive", drive.device));
            mciSendStringW(open.as_ptr(), std::ptr::null_mut(), 0, std::ptr::null_mut());
            let door = w("set odp_drive door open".into());
            mciSendStringW(door.as_ptr(), std::ptr::null_mut(), 0, std::ptr::null_mut());
            let close = w("close odp_drive".into());
            mciSendStringW(close.as_ptr(), std::ptr::null_mut(), 0, std::ptr::null_mut());
        }
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("eject").arg(&drive.device).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("diskutil").arg("eject").arg(&drive.device).spawn();
    }
}

/// The address the engine opens for a disc of the given kind at `device`.
/// Returns (url, option name, option value).
pub fn engine_url(kind: DiscKind, device: &str, title: Option<i64>) -> (String, &'static str, String) {
    match kind {
        DiscKind::BluRay | DiscKind::UhdBluRay => {
            let t = title.map(|t| t.to_string()).unwrap_or_default();
            (format!("bd://{t}"), "bluray-device", device_root(device))
        }
        DiscKind::AudioCd => {
            if cfg!(target_os = "macos") {
                (device.to_string(), "", String::new())
            } else {
                ("cdda://".to_string(), "cdrom-device", device.to_string())
            }
        }
        _ => {
            let t = title.map(|t| t.to_string()).unwrap_or_default();
            (format!("dvd://{t}"), "dvd-device", device.to_string())
        }
    }
}

fn device_root(device: &str) -> String {
    if cfg!(windows) && device.len() == 2 && device.ends_with(':') {
        format!("{device}\\")
    } else {
        device.to_string()
    }
}
