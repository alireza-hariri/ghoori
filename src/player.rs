//! mpv core wrapper: option setup, observed-property state and event pumping.

use crate::util;
use libmpv2::{Format, Mpv};
use libmpv2_sys as sys;
use serde_json::Value;
use std::ffi::{CStr, c_char, c_int};

pub fn mpv_err_str(code: c_int) -> String {
    unsafe { CStr::from_ptr(sys::mpv_error_string(code)) }.to_string_lossy().into_owned()
}

fn lib_err(e: libmpv2::Error) -> String {
    match e {
        libmpv2::Error::Raw(code) => mpv_err_str(code),
        other => format!("{other:?}"),
    }
}

#[derive(Debug, Clone, Default)]
pub struct Track {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub lang: String,
    pub codec: String,
    pub selected: bool,
    pub external: bool,
    pub albumart: bool,
    pub detail: String,
}

impl Track {
    pub fn label(&self) -> String {
        let mut s = format!("#{}", self.id);
        if !self.title.is_empty() {
            s += &format!("  {}", self.title);
        }
        if !self.lang.is_empty() {
            s += &format!("  [{}]", self.lang);
        }
        let mut extra = Vec::new();
        if !self.codec.is_empty() {
            extra.push(self.codec.clone());
        }
        if !self.detail.is_empty() {
            extra.push(self.detail.clone());
        }
        if self.external {
            extra.push("external".into());
        }
        if !extra.is_empty() {
            s += &format!("  ({})", extra.join(", "));
        }
        s
    }
}

#[derive(Debug, Clone, Default)]
pub struct PlaylistItem {
    pub filename: String,
    pub title: Option<String>,
    pub current: bool,
}

impl PlaylistItem {
    pub fn name(&self) -> String {
        self.title.clone().unwrap_or_else(|| util::display_name(&self.filename))
    }
}

#[derive(Debug, Clone, Default)]
pub struct Chapter {
    pub title: String,
    pub time: f64,
}

#[derive(Debug, Default)]
pub struct State {
    pub time_pos: f64,
    pub duration: f64,
    pub paused: bool,
    pub volume: f64,
    pub volume_max: f64,
    pub muted: bool,
    pub speed: f64,
    pub title: String,
    pub path: String,
    pub idle: bool,
    pub eof: bool,
    pub buffering: bool,
    pub cache_percent: i64,
    pub cache_end: f64,
    pub seekable: bool,
    pub playlist: Vec<PlaylistItem>,
    pub playlist_pos: i64,
    pub tracks: Vec<Track>,
    pub chapters: Vec<Chapter>,
    pub chapter: i64,
    pub ab_a: Option<f64>,
    pub ab_b: Option<f64>,
    pub loop_file: bool,
    pub loop_playlist: bool,
    pub sub_delay: f64,
    pub sub_visible: bool,
    pub width: i64,
    pub height: i64,
    pub hwdec: String,
    pub video_codec: String,
    pub audio_codec: String,
    pub file_format: String,
    pub fps: f64,
    pub dropped: i64,
    pub video_bitrate: f64,
    pub audio_bitrate: f64,
    pub samplerate: i64,
    pub channels: String,
    pub file_size: i64,
    pub metadata: Vec<(String, String)>,
}

impl State {
    pub fn video_track(&self) -> Option<&Track> {
        self.tracks.iter().find(|t| t.kind == "video" && t.selected)
    }
    /// A real moving-picture track (not cover art) is playing.
    pub fn has_motion_video(&self) -> bool {
        self.video_track().is_some_and(|t| !t.albumart)
    }
    pub fn meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.as_str())
    }
    pub fn current_chapter(&self) -> Option<&Chapter> {
        usize::try_from(self.chapter).ok().and_then(|i| self.chapters.get(i))
    }
}

#[derive(Debug)]
pub enum PlayerEvent {
    FileLoaded,
    VideoReconfig,
    PlaybackRestart,
    LoadFailed { error: String },
    Shutdown,
}

const OBSERVED: &[(&str, Format)] = &[
    ("time-pos", Format::Double),
    ("duration", Format::Double),
    ("pause", Format::Flag),
    ("volume", Format::Double),
    ("volume-max", Format::Double),
    ("mute", Format::Flag),
    ("speed", Format::Double),
    ("media-title", Format::String),
    ("path", Format::String),
    ("idle-active", Format::Flag),
    ("eof-reached", Format::Flag),
    ("paused-for-cache", Format::Flag),
    ("cache-buffering-state", Format::Int64),
    ("demuxer-cache-time", Format::Double),
    ("seekable", Format::Flag),
    ("playlist", Format::String),
    ("playlist-pos", Format::Int64),
    ("track-list", Format::String),
    ("chapter-list", Format::String),
    ("chapter", Format::Int64),
    ("ab-loop-a", Format::String),
    ("ab-loop-b", Format::String),
    ("loop-file", Format::String),
    ("loop-playlist", Format::String),
    ("sub-delay", Format::Double),
    ("sub-visibility", Format::Flag),
    ("dwidth", Format::Int64),
    ("dheight", Format::Int64),
    ("hwdec-current", Format::String),
    ("video-codec", Format::String),
    ("audio-codec-name", Format::String),
    ("file-format", Format::String),
    ("estimated-vf-fps", Format::Double),
    ("frame-drop-count", Format::Int64),
    ("video-bitrate", Format::Double),
    ("audio-bitrate", Format::Double),
    ("audio-params/samplerate", Format::Int64),
    ("audio-params/hr-channels", Format::String),
    ("file-size", Format::Int64),
    ("metadata", Format::String),
];

enum Val {
    None,
    F(f64),
    B(bool),
    I(i64),
    S(String),
}

impl Val {
    fn f(&self) -> f64 {
        match *self {
            Val::F(v) => v,
            Val::I(v) => v as f64,
            _ => 0.0,
        }
    }
    fn i(&self) -> i64 {
        match *self {
            Val::I(v) => v,
            Val::F(v) => v as i64,
            _ => 0,
        }
    }
    fn b(&self) -> bool {
        matches!(*self, Val::B(true))
    }
    fn s(&self) -> String {
        match self {
            Val::S(s) => s.clone(),
            _ => String::new(),
        }
    }
    fn json(&self) -> Value {
        match self {
            Val::S(s) => serde_json::from_str(s).unwrap_or(Value::Null),
            _ => Value::Null,
        }
    }
}

fn jstr(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn parse_tracks(v: &Value) -> Vec<Track> {
    let Some(arr) = v.as_array() else { return Vec::new() };
    arr.iter()
        .map(|t| {
            let kind = jstr(t, "type");
            let detail = match kind.as_str() {
                "video" => match (t.get("demux-w").and_then(Value::as_i64), t.get("demux-h").and_then(Value::as_i64)) {
                    (Some(w), Some(h)) => format!("{w}x{h}"),
                    _ => String::new(),
                },
                "audio" => {
                    let ch = t.get("demux-channel-count").and_then(Value::as_i64);
                    let sr = t.get("demux-samplerate").and_then(Value::as_i64);
                    match (ch, sr) {
                        (Some(c), Some(s)) => format!("{c}ch {} kHz", s as f64 / 1000.0),
                        (Some(c), None) => format!("{c}ch"),
                        _ => String::new(),
                    }
                }
                _ => String::new(),
            };
            Track {
                id: t.get("id").and_then(Value::as_i64).unwrap_or(0),
                kind,
                title: jstr(t, "title"),
                lang: jstr(t, "lang"),
                codec: jstr(t, "codec"),
                selected: t.get("selected").and_then(Value::as_bool).unwrap_or(false),
                external: t.get("external").and_then(Value::as_bool).unwrap_or(false),
                albumart: t.get("albumart").and_then(Value::as_bool).unwrap_or(false)
                    || t.get("image").and_then(Value::as_bool).unwrap_or(false),
                detail,
            }
        })
        .collect()
}

fn parse_playlist(v: &Value) -> Vec<PlaylistItem> {
    let Some(arr) = v.as_array() else { return Vec::new() };
    arr.iter()
        .map(|e| PlaylistItem {
            filename: jstr(e, "filename"),
            title: e.get("title").and_then(Value::as_str).map(str::to_string),
            current: e.get("current").and_then(Value::as_bool).unwrap_or(false),
        })
        .collect()
}

fn parse_chapters(v: &Value) -> Vec<Chapter> {
    let Some(arr) = v.as_array() else { return Vec::new() };
    arr.iter()
        .map(|c| Chapter { title: jstr(c, "title"), time: c.get("time").and_then(Value::as_f64).unwrap_or(0.0) })
        .collect()
}

pub struct Player {
    pub mpv: Mpv,
    pub st: State,
}

pub struct PlayerOptions<'a> {
    pub volume: f64,
    pub muted: bool,
    pub verbose: bool,
    pub extra: &'a [(String, String)],
}

impl Player {
    pub fn new(opts: PlayerOptions) -> Result<Self, String> {
        let config_dir = util::config_dir();
        let watch_later = util::state_dir().join("watch_later");
        let _ = std::fs::create_dir_all(&config_dir);
        let _ = std::fs::create_dir_all(&watch_later);
        let shots = util::user_dir("PICTURES", "Pictures").join("Ghoori");
        let volume = format!("{}", opts.volume.round());
        // Probing CUDA/Vulkan on machines without such devices can stall for
        // seconds, so only try backends whose device nodes exist.
        let nvidia = util::nvidia_gpu_active();
        let dri = std::path::Path::new("/dev/dri").exists();
        let mut hwdec: Vec<&str> = Vec::new();
        let mut interop: Vec<&str> = Vec::new();
        if nvidia {
            hwdec.extend(["nvdec", "nvdec-copy"]);
            interop.push("cuda");
        }
        if dri {
            hwdec.extend(["vaapi", "vaapi-copy"]);
            interop.push("vaapi");
        }
        hwdec.push("no");
        if interop.is_empty() {
            interop.push("no");
        }

        let defaults: Vec<(&str, String)> = vec![
            ("vo", "libmpv".into()),
            ("hwdec", hwdec.join(",")),
            ("gpu-hwdec-interop", interop.join(",")),
            ("idle", "yes".into()),
            ("keep-open", "yes".into()),
            ("input-default-bindings", "no".into()),
            ("input-vo-keyboard", "no".into()),
            ("osd-level", "1".into()),
            ("terminal", if opts.verbose { "yes" } else { "no" }.into()),
            ("msg-level", if opts.verbose { "all=v" } else { "all=warn" }.into()),
            ("config-dir", config_dir.to_string_lossy().into_owned()),
            ("config", "yes".into()),
            ("save-position-on-quit", "yes".into()),
            ("resume-playback", "yes".into()),
            ("watch-later-dir", watch_later.to_string_lossy().into_owned()),
            ("sub-auto", "fuzzy".into()),
            ("audio-file-auto", "no".into()),
            ("volume", volume),
            ("volume-max", "150".into()),
            ("mute", if opts.muted { "yes" } else { "no" }.into()),
            ("screenshot-directory", shots.to_string_lossy().into_owned()),
            ("screenshot-template", "%F-%P".into()),
            ("ytdl", "yes".into()),
            ("image-display-duration", "inf".into()),
            ("demuxer-max-bytes", "150MiB".into()),
            ("demuxer-max-back-bytes", "75MiB".into()),
        ];

        let mpv = Mpv::with_initializer(|init| {
            for (k, v) in &defaults {
                if let Err(e) = init.set_option(k, v.as_str()) {
                    eprintln!("ghoori: mpv option {k}={v} rejected: {}", lib_err(e));
                }
            }
            for (k, v) in opts.extra {
                if let Err(e) = init.set_option(k, v.as_str()) {
                    eprintln!("ghoori: mpv option --{k}={v} rejected: {}", lib_err(e));
                }
            }
            Ok(())
        })
        .map_err(lib_err)?;

        for (name, fmt) in OBSERVED {
            mpv.observe_property(name, *fmt, 0).map_err(lib_err)?;
        }

        Ok(Self {
            mpv,
            st: State {
                volume: opts.volume,
                volume_max: 150.0,
                speed: 1.0,
                idle: true,
                sub_visible: true,
                ..Default::default()
            },
        })
    }

    pub fn command(&self, args: &[&str]) -> Result<(), String> {
        let Some((name, rest)) = args.split_first() else { return Ok(()) };
        self.mpv.command(name, rest).map_err(lib_err)
    }

    /// Run a command, ignoring failure (used for user actions whose failure is harmless).
    pub fn cmd(&self, args: &[&str]) {
        if let Err(e) = self.command(args) {
            eprintln!("ghoori: command {args:?} failed: {e}");
        }
    }

    pub fn set_str(&self, name: &str, v: &str) {
        if let Err(e) = self.mpv.set_property(name, v) {
            eprintln!("ghoori: set {name}={v} failed: {}", lib_err(e));
        }
    }
    pub fn set_f64(&self, name: &str, v: f64) {
        let _ = self.mpv.set_property(name, v);
    }
    pub fn set_bool(&self, name: &str, v: bool) {
        let _ = self.mpv.set_property(name, v);
    }
    pub fn get_string(&self, name: &str) -> Option<String> {
        self.mpv.get_property::<String>(name).ok()
    }

    pub fn has_file(&self) -> bool {
        !self.st.idle && !self.st.path.is_empty()
    }

    /// Load files; `append` adds them to the playlist instead of replacing it.
    pub fn open(&mut self, items: &[String], append: bool) {
        if items.is_empty() {
            return;
        }
        if !append {
            self.save_position();
        }
        for (i, item) in items.iter().enumerate() {
            let mode = if append || i > 0 { "append-play" } else { "replace" };
            self.cmd(&["loadfile", item, mode]);
        }
    }

    pub fn save_position(&self) {
        if self.has_file() && util::is_local_file(&self.st.path) {
            let _ = self.command(&["write-watch-later-config"]);
        }
    }

    pub fn toggle_pause(&self) {
        if self.st.eof && self.st.paused {
            // At the end of a kept-open file: restart rather than no-op.
            self.cmd(&["seek", "0", "absolute"]);
        }
        self.set_bool("pause", !self.st.paused);
    }

    pub fn seek_relative(&self, secs: f64, exact: bool) {
        let s = format!("{secs}");
        self.cmd(&["seek", &s, if exact { "relative+exact" } else { "relative" }]);
    }

    pub fn seek_absolute(&self, secs: f64, exact: bool) {
        let s = format!("{:.3}", secs.max(0.0));
        self.cmd(&["seek", &s, if exact { "absolute+exact" } else { "absolute+keyframes" }]);
    }

    pub fn set_volume(&self, v: f64) {
        self.set_f64("volume", v.clamp(0.0, self.st.volume_max.max(100.0)));
    }

    pub fn select_track(&self, kind: &str, id: Option<i64>) {
        let prop = match kind {
            "video" => "vid",
            "audio" => "aid",
            _ => "sid",
        };
        match id {
            Some(id) => self.set_str(prop, &id.to_string()),
            None => self.set_str(prop, "no"),
        }
    }

    /// Drain all pending mpv events, updating `st`.
    pub fn poll(&mut self) -> Vec<PlayerEvent> {
        let mut out = Vec::new();
        loop {
            let ev = unsafe { &*sys::mpv_wait_event(self.mpv.ctx.as_ptr(), 0.0) };
            match ev.event_id {
                sys::mpv_event_id_MPV_EVENT_NONE => break,
                sys::mpv_event_id_MPV_EVENT_SHUTDOWN => out.push(PlayerEvent::Shutdown),
                sys::mpv_event_id_MPV_EVENT_FILE_LOADED => out.push(PlayerEvent::FileLoaded),
                sys::mpv_event_id_MPV_EVENT_VIDEO_RECONFIG => out.push(PlayerEvent::VideoReconfig),
                sys::mpv_event_id_MPV_EVENT_PLAYBACK_RESTART => out.push(PlayerEvent::PlaybackRestart),
                sys::mpv_event_id_MPV_EVENT_END_FILE => {
                    let ef = unsafe { &*(ev.data as *const sys::mpv_event_end_file) };
                    if ef.reason == sys::mpv_end_file_reason_MPV_END_FILE_REASON_ERROR {
                        out.push(PlayerEvent::LoadFailed { error: mpv_err_str(ef.error) });
                    }
                }
                sys::mpv_event_id_MPV_EVENT_PROPERTY_CHANGE => {
                    let p = unsafe { &*(ev.data as *const sys::mpv_event_property) };
                    let name = unsafe { CStr::from_ptr(p.name) }.to_string_lossy();
                    let val = unsafe { read_value(p.format, p.data) };
                    self.apply(&name, val);
                }
                _ => {}
            }
        }
        out
    }

    fn apply(&mut self, name: &str, v: Val) {
        let st = &mut self.st;
        match name {
            "time-pos" => st.time_pos = v.f(),
            "duration" => st.duration = v.f(),
            "pause" => st.paused = v.b(),
            "volume" => st.volume = v.f(),
            "volume-max" => st.volume_max = if matches!(v, Val::None) { 100.0 } else { v.f() },
            "mute" => st.muted = v.b(),
            "speed" => st.speed = if matches!(v, Val::None) { 1.0 } else { v.f() },
            "media-title" => st.title = v.s(),
            "path" => st.path = v.s(),
            "idle-active" => st.idle = v.b(),
            "eof-reached" => st.eof = v.b(),
            "paused-for-cache" => st.buffering = v.b(),
            "cache-buffering-state" => st.cache_percent = v.i(),
            "demuxer-cache-time" => st.cache_end = v.f(),
            "seekable" => st.seekable = v.b(),
            "playlist" => st.playlist = parse_playlist(&v.json()),
            "playlist-pos" => st.playlist_pos = if matches!(v, Val::None) { -1 } else { v.i() },
            "track-list" => st.tracks = parse_tracks(&v.json()),
            "chapter-list" => st.chapters = parse_chapters(&v.json()),
            "chapter" => st.chapter = if matches!(v, Val::None) { -1 } else { v.i() },
            "ab-loop-a" => st.ab_a = v.s().parse().ok(),
            "ab-loop-b" => st.ab_b = v.s().parse().ok(),
            "loop-file" => st.loop_file = !matches!(v.s().as_str(), "" | "no"),
            "loop-playlist" => st.loop_playlist = !matches!(v.s().as_str(), "" | "no"),
            "sub-delay" => st.sub_delay = v.f(),
            "sub-visibility" => st.sub_visible = v.b(),
            "dwidth" => st.width = v.i(),
            "dheight" => st.height = v.i(),
            "hwdec-current" => st.hwdec = v.s(),
            "video-codec" => st.video_codec = v.s(),
            "audio-codec-name" => st.audio_codec = v.s(),
            "file-format" => st.file_format = v.s(),
            "estimated-vf-fps" => st.fps = v.f(),
            "frame-drop-count" => st.dropped = v.i(),
            "video-bitrate" => st.video_bitrate = v.f(),
            "audio-bitrate" => st.audio_bitrate = v.f(),
            "audio-params/samplerate" => st.samplerate = v.i(),
            "audio-params/hr-channels" => st.channels = v.s(),
            "file-size" => st.file_size = v.i(),
            "metadata" => {
                st.metadata = match v.json() {
                    Value::Object(map) => {
                        map.into_iter().filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string()))).collect()
                    }
                    _ => Vec::new(),
                }
            }
            _ => {}
        }
    }
}

unsafe fn read_value(format: sys::mpv_format, data: *mut std::ffi::c_void) -> Val {
    if data.is_null() {
        return Val::None;
    }
    unsafe {
        match format {
            sys::mpv_format_MPV_FORMAT_DOUBLE => Val::F(*(data as *const f64)),
            sys::mpv_format_MPV_FORMAT_FLAG => Val::B(*(data as *const c_int) != 0),
            sys::mpv_format_MPV_FORMAT_INT64 => Val::I(*(data as *const i64)),
            sys::mpv_format_MPV_FORMAT_STRING => {
                let p = *(data as *const *const c_char);
                if p.is_null() { Val::None } else { Val::S(CStr::from_ptr(p).to_string_lossy().into_owned()) }
            }
            _ => Val::None,
        }
    }
}
