//! Settings that are remembered between runs, the media library scan, and the
//! codec engine installer.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

pub const APP_NAME: &str = "PlayCraft";

/// The program's earlier name; its settings folder is moved over on first start.
const OLD_APP_NAME: &str = "OpenDisc Player";

pub fn data_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join(APP_NAME)
}

/// Carry settings, resume points and any downloaded engine over from the old name.
pub fn migrate_old_data() {
    let Some(base) = dirs::data_local_dir() else { return };
    let old = base.join(OLD_APP_NAME);
    let new = base.join(APP_NAME);
    if old.is_dir() && !new.exists() {
        let _ = std::fs::rename(&old, &new);
    }
}

pub fn engine_dir() -> PathBuf {
    data_dir().join("engine")
}

fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RecentItem {
    pub title: String,
    pub url: String,
    pub kind: String,
    #[serde(default)]
    pub device_opt: String,
    #[serde(default)]
    pub device: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub folders: Vec<PathBuf>,
    pub recent: Vec<RecentItem>,
    pub resume: HashMap<String, f64>,
    pub volume: f64,
    pub resume_playback: bool,
    pub hardware_decoding: bool,
    pub deinterlace: bool,
    pub audio_lang: String,
    pub sub_lang: String,
    pub subtitles_on: bool,
    pub sub_scale: f64,
    pub audio_output: String,
    pub hd_passthrough: bool,
    pub night_mode: bool,
    pub dialogue_boost: bool,
    pub brightness: i32,
    pub contrast: i32,
    pub saturation: i32,
    pub gamma: i32,
    pub sharpen: bool,
    pub denoise: bool,
    pub snapshot_dir: Option<PathBuf>,
    pub auto_play_disc: bool,
    /// A licensed Blu-ray player the person already owns, used for copy-protected discs.
    pub licensed_player: Option<PathBuf>,
    /// Start DVDs and Blu-rays at their own menus.
    pub disc_menus: bool,
    /// Remote size (1.0 = normal), mini layout, and whether its volume keys
    /// control the computer (true) or only PlayCraft (false).
    pub remote_scale: f32,
    pub remote_mini: bool,
    pub remote_pc_volume: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            folders: default_folders(),
            recent: Vec::new(),
            resume: HashMap::new(),
            volume: 80.0,
            resume_playback: true,
            hardware_decoding: true,
            deinterlace: false,
            audio_lang: "en,eng".into(),
            sub_lang: "en,eng".into(),
            subtitles_on: false,
            sub_scale: 1.0,
            audio_output: "auto-safe".into(),
            hd_passthrough: false,
            night_mode: false,
            dialogue_boost: false,
            brightness: 0,
            contrast: 0,
            saturation: 0,
            gamma: 0,
            sharpen: false,
            denoise: false,
            snapshot_dir: None,
            auto_play_disc: false,
            licensed_player: None,
            disc_menus: true,
            remote_scale: 1.0,
            remote_mini: false,
            remote_pc_volume: true,
        }
    }
}

fn default_folders() -> Vec<PathBuf> {
    [dirs::video_dir(), dirs::audio_dir(), dirs::picture_dir()].into_iter().flatten().collect()
}

impl Settings {
    pub fn load() -> Settings {
        std::fs::read_to_string(settings_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(data_dir());
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(settings_path(), s);
        }
    }

    pub fn snapshot_folder(&self) -> PathBuf {
        self.snapshot_dir
            .clone()
            .or_else(|| dirs::picture_dir().map(|p| p.join("PlayCraft Snapshots")))
            .unwrap_or_else(|| data_dir().join("snapshots"))
    }

    pub fn add_recent(&mut self, item: RecentItem) {
        self.recent.retain(|r| r.url != item.url || r.device != item.device);
        self.recent.insert(0, item);
        self.recent.truncate(12);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Video,
    Music,
    Photo,
    DiscImage,
    DiscFolder,
}

#[derive(Clone, Debug)]
pub struct MediaItem {
    pub path: PathBuf,
    pub name: String,
    pub folder: String,
    pub ext: String,
    pub size: u64,
    pub kind: MediaKind,
}

pub const VIDEO_EXT: &[&str] = &[
    "mkv", "mp4", "m4v", "avi", "mov", "wmv", "flv", "webm", "mpg", "mpeg", "m2ts", "mts", "ts", "vob", "ogv",
    "3gp", "rmvb", "rm", "divx", "asf", "f4v", "wtv", "dvr-ms", "m2v", "hevc", "264", "265", "y4m", "mxf",
];
pub const AUDIO_EXT: &[&str] = &[
    "mp3", "flac", "wav", "aac", "m4a", "ogg", "oga", "opus", "wma", "ape", "aiff", "aif", "dsf", "dff", "mka",
    "ac3", "eac3", "dts", "thd", "wv", "mpc", "tta", "alac", "mid", "midi", "cda",
];
pub const PHOTO_EXT: &[&str] = &["jpg", "jpeg", "png", "gif", "bmp", "webp", "tif", "tiff", "heic", "avif", "jxl"];

pub fn kind_of(path: &Path) -> Option<MediaKind> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    if ext == "iso" {
        Some(MediaKind::DiscImage)
    } else if VIDEO_EXT.contains(&ext.as_str()) {
        Some(MediaKind::Video)
    } else if AUDIO_EXT.contains(&ext.as_str()) {
        Some(MediaKind::Music)
    } else if PHOTO_EXT.contains(&ext.as_str()) {
        Some(MediaKind::Photo)
    } else {
        None
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VideoGroup {
    Movie,
    TvShow,
    Other,
}

/// Sort a video into Movies, TV Shows or Videos from its name and size.
pub fn video_group(m: &MediaItem) -> VideoGroup {
    if matches!(m.kind, MediaKind::DiscFolder | MediaKind::DiscImage) {
        return VideoGroup::Movie;
    }
    let n = m.name.to_lowercase();
    let b = n.as_bytes();
    let mut episode = n.contains("season") || n.contains("episode");
    for i in 0..b.len() {
        // S01E02 or 1x02 patterns
        if b[i] == b's' && i + 3 < b.len() && b[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j + 1 < b.len() && b[j] == b'e' && b[j + 1].is_ascii_digit() && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) {
                episode = true;
            }
        }
        if b[i] == b'x' && i > 0 && i + 2 < b.len() && b[i - 1].is_ascii_digit() && b[i + 1].is_ascii_digit() && b[i + 2].is_ascii_digit() {
            episode = true;
        }
    }
    if episode {
        VideoGroup::TvShow
    } else if m.size >= 700_000_000 {
        VideoGroup::Movie
    } else {
        VideoGroup::Other
    }
}

pub fn display_name(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|s| s.to_string_lossy().replace(['_', '.'], " "))
        .unwrap_or_else(|| path.display().to_string())
}

/// Scan the library folders on a background thread.
pub fn scan(folders: Vec<PathBuf>) -> Receiver<Vec<MediaItem>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let mut items = Vec::new();
        for root in folders {
            let mut it = walkdir::WalkDir::new(&root).max_depth(8).follow_links(false).into_iter();
            while let Some(Ok(e)) = it.next() {
                if items.len() > 20000 {
                    break;
                }
                let p = e.path();
                if e.file_type().is_dir() {
                    let n = e.file_name().to_string_lossy().to_uppercase();
                    if n == "VIDEO_TS" || n == "BDMV" {
                        let disc = p.parent().unwrap_or(p).to_path_buf();
                        items.push(MediaItem {
                            name: display_name(&disc),
                            folder: disc.parent().map(|f| f.display().to_string()).unwrap_or_default(),
                            ext: if n == "BDMV" { "Blu-ray".into() } else { "DVD".into() },
                            size: 0,
                            kind: MediaKind::DiscFolder,
                            path: disc,
                        });
                        it.skip_current_dir();
                    } else if n.starts_with('.') || n == "$RECYCLE.BIN" || n == "NODE_MODULES" {
                        it.skip_current_dir();
                    }
                    continue;
                }
                if let Some(kind) = kind_of(p) {
                    let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                    items.push(MediaItem {
                        name: display_name(p),
                        folder: p.parent().map(|f| f.display().to_string()).unwrap_or_default(),
                        ext: p.extension().map(|x| x.to_string_lossy().to_uppercase()).unwrap_or_default(),
                        size,
                        kind,
                        path: p.to_path_buf(),
                    });
                }
            }
        }
        items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        let _ = tx.send(items);
    });
    rx
}

/// Where the Blu-ray decryption library looks for its key file.
pub fn keydb_path() -> Option<PathBuf> {
    if cfg!(windows) {
        dirs::config_dir().map(|d| d.join("aacs").join("KEYDB.cfg"))
    } else if cfg!(target_os = "macos") {
        dirs::home_dir().map(|d| d.join("Library/Preferences/aacs/KEYDB.cfg"))
    } else {
        dirs::config_dir().map(|d| d.join("aacs").join("KEYDB.cfg"))
    }
}

pub fn engine_install_help() -> &'static str {
    if cfg!(windows) {
        "The playback engine (libmpv with FFmpeg, libdvdnav, libbluray, libcdio, libaacs and libbdplus) is bundled in the \"engine\" folder next to the program when it is built. If that folder is missing, press \"Install codec engine\" to download it (about 30 MB)."
    } else if cfg!(target_os = "macos") {
        "Install the free engine with Homebrew, then restart the player:\n    brew install mpv libbluray libaacs\nThis brings FFmpeg with every common codec, DVD and Blu-ray support."
    } else {
        "Install the free engine from your system's package manager, then restart the player:\n    Debian/Ubuntu:  sudo apt install libmpv2 libdvdnav4 libdvdcss2 libbluray2 libaacs0\n    Fedora:         sudo dnf install mpv-libs libdvdnav libbluray libaacs\n    Arch:           sudo pacman -S mpv libdvdnav libbluray libaacs\nThis brings FFmpeg with every common codec, DVD, CD and Blu-ray support."
    }
}

pub enum InstallMsg {
    Progress(String),
    Done(Result<String, String>),
}

/// Download the open-source engine (Windows only; other systems use their package manager).
pub fn install_engine(ctx: eframe::egui::Context) -> Receiver<InstallMsg> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let dest = engine_dir();
        let _ = std::fs::create_dir_all(&dest);
        if !cfg!(windows) {
            let _ = tx.send(InstallMsg::Done(Err(engine_install_help().to_string())));
            ctx.request_repaint();
            return;
        }
        let say = |m: String| {
            let _ = tx.send(InstallMsg::Progress(m));
            ctx.request_repaint();
        };
        let res = fetch_engine_windows(&dest, &say);
        if let Err(e) = &res {
            crate::log::error(format!("Codec engine install failed: {e}"));
        }
        let _ = tx.send(InstallMsg::Done(res));
        ctx.request_repaint();
    });
    rx
}

/// Run a Windows tool without a console window; return its output or a readable error.
fn run_tool(exe: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let o = cmd.output().map_err(|e| format!("{exe} could not start ({e})"))?;
    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&o.stderr);
        Err(format!("{exe}: {}", err.lines().find(|l| !l.trim().is_empty()).unwrap_or("failed")))
    }
}

/// Download the playback engine for this PlayCraft's processor using Windows' own curl.exe
/// and tar.exe (no PowerShell scripts, which security software often blocks).
pub fn fetch_engine_windows(dest: &Path, say: &dyn Fn(String)) -> Result<String, String> {
    const REPO: &str = "https://github.com/shinchiro/mpv-winbuild-cmake";
    let arch = msys_arch().0;
    let curl = |args: &[&str]| -> Result<String, String> {
        let mut a = vec!["-sSfL", "--retry", "4", "--retry-delay", "2", "-A", "playcraft"];
        a.extend_from_slice(args);
        run_tool("curl.exe", &a)
    };
    say("Finding the newest engine build…".into());
    // The "latest" page redirects to …/releases/tag/<tag>.
    let headers = curl(&["-I", &format!("{REPO}/releases/latest")])
        .map_err(|e| format!("Couldn't reach GitHub ({e}). Check the internet connection, or a firewall or proxy blocking it."))?;
    let tag = headers
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            let lower = l.to_ascii_lowercase();
            lower.starts_with("location:").then(|| l[9..].trim().to_string())
        })
        .filter_map(|l| l.split("/tag/").nth(1).map(str::to_string))
        .last()
        .ok_or("GitHub didn't say which engine build is the newest")?;
    let page = curl(&[&format!("{REPO}/releases/expanded_assets/{tag}")])?;
    let want = format!("mpv-dev-{arch}-2");
    let href = page
        .split("href=\"")
        .skip(1)
        .filter_map(|s| s.split('"').next())
        .find(|h| {
            let name = h.rsplit('/').next().unwrap_or(h);
            name.starts_with(&want) && name.ends_with(".7z")
        })
        .ok_or_else(|| format!("No engine build for {} was found in release {tag}", crate::mpv::Engine::processor_name()))?
        .to_string();
    let url = if href.starts_with("http") { href.clone() } else { format!("https://github.com{href}") };
    let name = href.rsplit('/').next().unwrap_or("engine.7z").to_string();
    say(format!("Downloading {name} (about 30 MB)…"));
    let archive = std::env::temp_dir().join(format!("playcraft-{name}"));
    let archive_s = archive.display().to_string();
    curl(&["-o", &archive_s, &url]).map_err(|e| format!("The download failed ({e})"))?;

    say("Unpacking…".into());
    let _ = std::fs::create_dir_all(dest);
    let dest_s = dest.display().to_string();
    let target = dest.join("libmpv-2.dll");
    let _ = std::fs::remove_file(&target);
    let tar = run_tool("tar.exe", &["-xf", &archive_s, "-C", &dest_s, "libmpv-2.dll"]);
    if !target.exists() {
        // Older Windows tar can't open .7z files: try 7-Zip if it is installed.
        let seven = ["7z.exe", r"C:\Program Files\7-Zip\7z.exe", r"C:\Program Files (x86)\7-Zip\7z.exe"];
        let mut done = false;
        for s in seven {
            if run_tool(s, &["e", &archive_s, &format!("-o{dest_s}"), "libmpv-2.dll", "-y"]).is_ok() && target.exists() {
                done = true;
                break;
            }
        }
        if !done {
            let _ = std::fs::remove_file(&archive);
            return Err(format!(
                "The engine was downloaded but couldn't be unpacked ({}). Installing 7-Zip (7-zip.org) and trying again will fix this.",
                tar.err().unwrap_or_else(|| "tar.exe found no engine file".into())
            ));
        }
    }
    let _ = std::fs::remove_file(&archive);
    crate::log::info(format!("Codec engine installed to {}", target.display()));
    Ok(target.display().to_string())
}

/// Download libaacs and libbdplus (built by MSYS2 from VideoLAN's official source releases)
/// plus their helper libraries into the user's engine folder. Windows only; other systems
/// get them from their package manager. No keys are downloaded.
pub fn install_aacs(ctx: eframe::egui::Context) -> Receiver<InstallMsg> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        if !cfg!(windows) {
            let msg = if cfg!(target_os = "macos") {
                "Install it with Homebrew, then restart the player:\n    brew install libaacs libbdplus"
            } else {
                "Install it from your package manager, then restart the player:\n    sudo apt install libaacs0 libbdplus0   (Debian/Ubuntu)\n    sudo dnf install libaacs libbdplus    (Fedora)"
            };
            let _ = tx.send(InstallMsg::Done(Err(msg.to_string())));
            ctx.request_repaint();
            return;
        }
        let dest = engine_dir();
        let _ = std::fs::create_dir_all(&dest);
        let _ = tx.send(InstallMsg::Progress("Downloading libaacs from the MSYS2 package server…".into()));
        ctx.request_repaint();
        let q = |p: &Path| p.display().to_string().replace('\'', "''");
        let work = dest.join("aacs-download");
        let script = format!(
            r#"$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$mirror = 'https://repo.msys2.org/mingw/{env}/'
$index = (Invoke-WebRequest -UseBasicParsing $mirror).Content
New-Item -ItemType Directory -Force '{work}' | Out-Null
foreach ($p in 'libaacs','libbdplus','libgcrypt','libgpg-error','libwinpthread','gettext-runtime','libiconv','gcc-libs','libc%2B%2B','libunwind') {{
  $m = [regex]::Matches($index, 'href="(mingw-w64-{cpu}-' + [regex]::Escape($p) + '-[0-9][^"]*?\.pkg\.tar\.zst)"')
  if ($m.Count -eq 0) {{ if ($p -in 'libaacs','libgcrypt','libgpg-error') {{ throw "$p was not found" }} else {{ continue }} }}
  $name = $m[$m.Count - 1].Groups[1].Value
  Write-Output ("GOT " + $name)
  Invoke-WebRequest -UseBasicParsing ($mirror + $name) -OutFile ('{work}\' + $name)
  & tar.exe -xf ('{work}\' + $name) -C '{work}'
}}
Copy-Item '{work}\{env}\bin\*.dll' '{dest}' -Force
Copy-Item '{dest}\libaacs-0.dll' '{dest}\libaacs.dll' -Force
if (Test-Path '{dest}\libbdplus-0.dll') {{ Copy-Item '{dest}\libbdplus-0.dll' '{dest}\libbdplus.dll' -Force }}
Remove-Item -Recurse -Force '{work}'
Write-Output 'OK'
"#,
            work = q(&work),
            dest = q(&dest),
            env = msys_arch().1,
            cpu = msys_arch().2
        );
        let mut cmd = std::process::Command::new("powershell.exe");
        cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &script]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        let res = match cmd.output() {
            Ok(o) if o.status.success() => {
                let got: Vec<String> = String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .filter_map(|l| l.strip_prefix(&format!("GOT mingw-w64-{}-", msys_arch().2)[..]).map(|s| s.trim_end_matches("-any.pkg.tar.zst").to_string()))
                    .collect();
                Ok(format!("Installed {}. Restart the player to use it.", got.join(", ")))
            }
            Ok(o) => Err(String::from_utf8_lossy(&o.stderr).lines().find(|l| !l.trim().is_empty()).unwrap_or("download failed").to_string()),
            Err(e) => Err(e.to_string()),
        };
        let _ = tx.send(InstallMsg::Done(res));
        ctx.request_repaint();
    });
    rx
}

pub fn fmt_time(t: f64) -> String {
    if !t.is_finite() || t < 0.0 {
        return "--:--".into();
    }
    let s = t as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

pub fn fmt_size(b: u64) -> String {
    let f = b as f64;
    if f >= 1e9 {
        format!("{:.1} GB", f / 1e9)
    } else if f >= 1e6 {
        format!("{:.0} MB", f / 1e6)
    } else if f > 0.0 {
        format!("{:.0} KB", f / 1e3)
    } else {
        String::new()
    }
}

/// (engine build, MSYS2 environment, MSYS2 package processor) matching this PlayCraft.
pub fn msys_arch() -> (&'static str, &'static str, &'static str) {
    if cfg!(target_arch = "aarch64") {
        ("aarch64", "clangarm64", "clang-aarch64")
    } else if cfg!(target_arch = "x86") {
        ("i686", "mingw32", "i686")
    } else {
        ("x86_64", "mingw64", "x86_64")
    }
}
