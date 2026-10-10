use crate::util;
use std::path::PathBuf;

const MAX_RECENT: usize = 12;

/// Persistent user preferences stored in `~/.config/ghoori/settings.conf`.
#[derive(Debug, Clone)]
pub struct Settings {
    pub volume: f64,
    pub muted: bool,
    pub show_remaining: bool,
    pub thumbnails: bool,
    pub browser_dir: Option<String>,
    pub recent: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 100.0,
            muted: false,
            show_remaining: false,
            thumbnails: true,
            browser_dir: None,
            recent: Vec::new(),
        }
    }
}

fn path() -> PathBuf {
    util::config_dir().join("settings.conf")
}

fn parse_bool(v: &str) -> bool {
    matches!(v, "1" | "yes" | "true" | "on")
}

impl Settings {
    pub fn load() -> Self {
        let mut s = Settings::default();
        let Ok(text) = std::fs::read_to_string(path()) else { return s };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "volume" => s.volume = v.parse().unwrap_or(s.volume),
                "muted" => s.muted = parse_bool(v),
                "show_remaining" => s.show_remaining = parse_bool(v),
                "thumbnails" => s.thumbnails = parse_bool(v),
                "browser_dir" => s.browser_dir = Some(v.to_string()),
                "recent" if s.recent.len() < MAX_RECENT => s.recent.push(v.to_string()),
                _ => {}
            }
        }
        s.volume = s.volume.clamp(0.0, 150.0);
        s
    }

    pub fn save(&self) {
        let mut out = String::from("# Ghoori settings (rewritten on exit)\n");
        out += &format!("volume={}\n", self.volume.round());
        out += &format!("muted={}\n", self.muted);
        out += &format!("show_remaining={}\n", self.show_remaining);
        out += &format!("thumbnails={}\n", self.thumbnails);
        if let Some(d) = &self.browser_dir {
            out += &format!("browser_dir={d}\n");
        }
        for r in &self.recent {
            out += &format!("recent={r}\n");
        }
        let p = path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&p, out) {
            eprintln!("ghoori: could not save settings to {}: {e}", p.display());
        }
    }

    pub fn push_recent(&mut self, entry: &str) {
        if entry.is_empty() || entry.contains('\n') {
            return;
        }
        self.recent.retain(|r| r != entry);
        self.recent.insert(0, entry.to_string());
        self.recent.truncate(MAX_RECENT);
    }
}
