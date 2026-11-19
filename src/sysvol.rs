//! The computer's own (system) volume and mute, used by the remote.

/// Raise or lower the system volume by roughly 4%.
pub fn step(up: bool) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{VK_VOLUME_DOWN, VK_VOLUME_UP};
        // Each volume key press moves Windows' volume by 2%.
        let vk = if up { VK_VOLUME_UP } else { VK_VOLUME_DOWN };
        press(vk);
        press(vk);
    }
    #[cfg(target_os = "macos")]
    {
        let delta = if up { "+ 6" } else { "- 6" };
        let script = format!("set volume output volume ((output volume of (get volume settings)) {delta})");
        let _ = std::process::Command::new("osascript").args(["-e", &script]).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let amount = if up { "+5%" } else { "-5%" };
        let ok = std::process::Command::new("pactl")
            .args(["set-sink-volume", "@DEFAULT_SINK@", amount])
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            let a = if up { "5%+" } else { "5%-" };
            let _ = std::process::Command::new("amixer").args(["-q", "sset", "Master", a]).spawn();
        }
    }
}

/// Mute or unmute the system sound.
pub fn toggle_mute() {
    #[cfg(windows)]
    {
        press(windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_VOLUME_MUTE);
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("osascript")
            .args(["-e", "set volume output muted (not (output muted of (get volume settings)))"])
            .spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let ok = std::process::Command::new("pactl")
            .args(["set-sink-mute", "@DEFAULT_SINK@", "toggle"])
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            let _ = std::process::Command::new("amixer").args(["-q", "sset", "Master", "toggle"]).spawn();
        }
    }
}

#[cfg(windows)]
fn press(vk: windows_sys::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP};
    unsafe {
        keybd_event(vk as u8, 0, KEYEVENTF_EXTENDEDKEY, 0);
        keybd_event(vk as u8, 0, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP, 0);
    }
}
