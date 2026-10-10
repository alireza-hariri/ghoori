use std::path::{Path, PathBuf};

pub const VIDEO_EXTS: &[&str] = &[
    "3g2", "3gp", "asf", "avi", "divx", "f4v", "flv", "h264", "h265", "hevc", "ifo", "m2t", "m2ts", "m2v", "m4v",
    "mkv", "mk3d", "mov", "mp4", "mpeg", "mpg", "mts", "mxf", "ogm", "ogv", "rm", "rmvb", "ts", "vob", "webm", "wmv",
    "y4m",
];
pub const AUDIO_EXTS: &[&str] = &[
    "aac", "ac3", "aiff", "alac", "ape", "au", "dts", "eac3", "flac", "m4a", "m4b", "mka", "mp2", "mp3", "mpc", "oga",
    "ogg", "opus", "ra", "tak", "tta", "wav", "weba", "wma", "wv",
];
pub const IMAGE_EXTS: &[&str] = &["avif", "bmp", "gif", "jpeg", "jpg", "jxl", "png", "tif", "tiff", "webp"];
pub const PLAYLIST_EXTS: &[&str] = &["cue", "m3u", "m3u8", "pls"];
pub const SUB_EXTS: &[&str] = &["ass", "idx", "lrc", "smi", "srt", "ssa", "sub", "sup", "vtt"];

fn ext_of(path: &str) -> String {
    Path::new(path).extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default()
}

pub fn is_media(path: &str) -> bool {
    let e = ext_of(path);
    [VIDEO_EXTS, AUDIO_EXTS, IMAGE_EXTS, PLAYLIST_EXTS].iter().any(|list| list.contains(&e.as_str()))
}

pub fn is_subtitle(path: &str) -> bool {
    SUB_EXTS.contains(&ext_of(path).as_str())
}

pub fn is_url(s: &str) -> bool {
    match s.find("://") {
        Some(i) => i > 1 && s[..i].chars().all(|c| c.is_ascii_alphanumeric() || c == '+'),
        None => false,
    }
}

pub fn is_local_file(s: &str) -> bool {
    !is_url(s) || s.starts_with("file://")
}

/// Human readable name for a path or URL.
pub fn display_name(s: &str) -> String {
    if is_url(s) && !s.starts_with("file://") {
        return s.to_string();
    }
    let s = s.strip_prefix("file://").unwrap_or(s);
    Path::new(s).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| s.to_string())
}

/// Format seconds as `m:ss` or `h:mm:ss`.
pub fn fmt_time(secs: f64) -> String {
    if !secs.is_finite() {
        return "--:--".into();
    }
    let neg = secs < 0.0;
    let t = secs.abs().floor() as u64;
    let (h, m, s) = (t / 3600, (t / 60) % 60, t % 60);
    let body = if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") };
    if neg { format!("-{body}") } else { body }
}

pub fn fmt_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 { format!("{bytes} B") } else { format!("{v:.1} {}", UNITS[u]) }
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    match std::env::var_os(var) {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => home_dir().join(fallback),
    }
}

pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("ghoori")
}

pub fn state_dir() -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state").join("ghoori")
}

/// Resolve a well-known user directory (Videos, Music, ...) via user-dirs.dirs.
pub fn user_dir(key: &str, fallback: &str) -> PathBuf {
    let home = home_dir();
    let conf = xdg("XDG_CONFIG_HOME", ".config").join("user-dirs.dirs");
    if let Ok(text) = std::fs::read_to_string(conf) {
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix(&format!("XDG_{key}_DIR=")) {
                let v = rest.trim().trim_matches('"').replace("$HOME", &home.to_string_lossy());
                return PathBuf::from(v);
            }
        }
    }
    home.join(fallback)
}

/// True when an NVIDIA GPU is present, powered on and driven by the nvidia driver.
///
/// Hybrid laptops often keep the dGPU suspended or removed from the bus; probing
/// CUDA then either wakes it up or stalls for seconds, so we only use it when the
/// kernel already reports it as active.
pub fn nvidia_gpu_active() -> bool {
    if !Path::new("/dev/nvidiactl").exists() {
        return false;
    }
    let Ok(devices) = std::fs::read_dir("/sys/bus/pci/devices") else { return false };
    let read = |p: PathBuf| std::fs::read_to_string(p).map(|s| s.trim().to_string()).unwrap_or_default();
    devices.flatten().any(|dev| {
        let d = dev.path();
        let is_nvidia_display = read(d.join("vendor")) == "0x10de" && read(d.join("class")).starts_with("0x03");
        if !is_nvidia_display {
            return false;
        }
        let driver = std::fs::read_link(d.join("driver"))
            .ok()
            .and_then(|l| l.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        // Reading these attributes does not wake the device.
        let runtime = read(d.join("power/runtime_status"));
        let state = read(d.join("power_state"));
        driver == "nvidia" && (runtime.is_empty() || runtime == "active") && (state.is_empty() || state == "D0")
    })
}

/// Make relative local paths absolute so mpv, recents and watch-later agree.
pub fn absolutize(s: &str) -> String {
    if is_url(s) {
        return s.to_string();
    }
    let p = Path::new(s);
    if p.is_absolute() {
        return s.to_string();
    }
    std::env::current_dir().map(|d| d.join(p).to_string_lossy().into_owned()).unwrap_or_else(|_| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_formatting() {
        assert_eq!(fmt_time(0.0), "0:00");
        assert_eq!(fmt_time(65.9), "1:05");
        assert_eq!(fmt_time(3725.0), "1:02:05");
        assert_eq!(fmt_time(-5.0), "-0:05");
        assert_eq!(fmt_time(f64::NAN), "--:--");
    }

    #[test]
    fn url_detection() {
        assert!(is_url("https://example.com/a.mp4"));
        assert!(is_url("file:///tmp/a.mp4"));
        assert!(!is_url("/tmp/a.mp4"));
        assert!(!is_url("C://x"));
        assert!(is_local_file("file:///tmp/a.mp4"));
        assert!(!is_local_file("http://x/y"));
    }

    #[test]
    fn media_detection() {
        assert!(is_media("/a/B.MKV"));
        assert!(is_media("x.flac"));
        assert!(!is_media("x.srt"));
        assert!(is_subtitle("x.SRT"));
        assert_eq!(display_name("/a/b/c.mp4"), "c.mp4");
        assert_eq!(display_name("file:///a/b.mp4"), "b.mp4");
    }
}
