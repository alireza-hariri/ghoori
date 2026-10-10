//! The player application: input handling, actions and the immediate-mode UI.

use crate::browser::{BrowseMode, Browser};
use crate::icons;
use crate::player::{Player, PlayerEvent, Track};
use crate::render::{self, GlRenderer};
use crate::settings::Settings;
use crate::text::Fonts;
use crate::thumbs::Thumbnailer;
use crate::util::{self, fmt_time};
use raylib::prelude::*;
use std::path::PathBuf;

const ACCENT: Color = Color::new(242, 165, 65, 255);
const FG: Color = Color::new(238, 238, 240, 255);
const DIM: Color = Color::new(160, 160, 168, 255);
const PANEL: Color = Color::new(22, 22, 27, 252);
const ROW_HOVER: Color = Color::new(255, 255, 255, 22);
const ERROR: Color = Color::new(225, 80, 80, 255);

const BAR_H: f32 = 88.0;
const TOP_H: f32 = 52.0;
const PANEL_W: f32 = 400.0;
const PL_ROW: f32 = 40.0;
const MENU_ROW: f32 = 32.0;
const HIDE_AFTER: f64 = 2.2;
const DOUBLE_CLICK: f64 = 0.28;

const SPEEDS: [f64; 9] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 3.0];

fn test_key(name: &str) -> Option<KeyboardKey> {
    use KeyboardKey::*;
    Some(match name {
        "SPACE" => KEY_SPACE,
        "LEFT" => KEY_LEFT,
        "RIGHT" => KEY_RIGHT,
        "UP" => KEY_UP,
        "DOWN" => KEY_DOWN,
        "N" => KEY_N,
        "P" => KEY_P,
        "M" => KEY_M,
        "S" => KEY_S,
        "I" => KEY_I,
        "R" => KEY_R,
        "C" => KEY_C,
        "A" => KEY_A,
        "V" => KEY_V,
        "TAB" => KEY_TAB,
        "F" => KEY_F,
        "ESC" => KEY_ESCAPE,
        "F1" => KEY_F1,
        "5" => KEY_FIVE,
        "]" => KEY_RIGHT_BRACKET,
        "[" => KEY_LEFT_BRACKET,
        _ => return None,
    })
}

fn fade(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, (c.a as f32 * a.clamp(0.0, 1.0)) as u8)
}

fn v2(x: f32, y: f32) -> Vector2 {
    Vector2::new(x, y)
}

fn inside(r: Rectangle, p: Vector2) -> bool {
    p.x >= r.x && p.x < r.x + r.width && p.y >= r.y && p.y < r.y + r.height
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    TogglePause,
    SeekRel(f64),
    SeekPercent(f64),
    FrameStep(bool),
    VolumeBy(f64),
    ToggleMute,
    SpeedBy(f64),
    SetSpeed(f64),
    Next,
    Prev,
    ChapterBy(i64),
    PlayIndex(usize),
    RemoveIndex(usize),
    MoveIndex(usize, usize),
    Shuffle,
    ClearPlaylist,
    ToggleLoopPlaylist,
    ToggleFullscreen,
    SelectTrack(&'static str, Option<i64>),
    Cycle(&'static str),
    ToggleSubs,
    SubDelay(f64),
    ToggleLoopFile,
    AbLoop,
    ClearAbLoop,
    Screenshot,
    TogglePlaylist,
    ToggleStats,
    ToggleHelp,
    ToggleRemaining,
    ToggleThumbs,
    Browse(BrowseMode),
    OpenUrlDialog,
    Popup(PopupKind),
    Open(Vec<String>, bool),
    Stop,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PopupKind {
    Context,
    Subs,
    Audio,
    Speed,
}

struct Popup {
    kind: PopupKind,
    anchor: Vector2,
    upward: bool,
}

struct MenuItem {
    label: String,
    hint: String,
    checked: bool,
    action: Option<Action>,
}

impl MenuItem {
    fn new(label: impl Into<String>, action: Action) -> Self {
        Self { label: label.into(), hint: String::new(), checked: false, action: Some(action) }
    }
    fn hint(mut self, h: &str) -> Self {
        self.hint = h.into();
        self
    }
    fn checked(mut self, c: bool) -> Self {
        self.checked = c;
        self
    }
    fn sep() -> Self {
        Self { label: String::new(), hint: String::new(), checked: false, action: None }
    }
    fn info(label: impl Into<String>) -> Self {
        Self { label: label.into(), hint: String::new(), checked: false, action: None }
    }
}

enum Modal {
    Browser(Browser),
    Url(String),
    Help,
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Layer {
    Video,
    Bar,
    Panel,
    Popup,
    Modal,
}

#[derive(Default)]
struct Input {
    mouse: Vector2,
    moved: bool,
    left_pressed: bool,
    left_down: bool,
    right_pressed: bool,
    middle_pressed: bool,
    wheel: f32,
    keys: Vec<KeyboardKey>,
    chars: Vec<char>,
    shift: bool,
    ctrl: bool,
    dropped: Vec<String>,
    w: f32,
    h: f32,
    now: f64,
    on_screen: bool,
}

const REPEATABLE: &[KeyboardKey] = &[
    KeyboardKey::KEY_LEFT,
    KeyboardKey::KEY_RIGHT,
    KeyboardKey::KEY_UP,
    KeyboardKey::KEY_DOWN,
    KeyboardKey::KEY_PAGE_UP,
    KeyboardKey::KEY_PAGE_DOWN,
    KeyboardKey::KEY_BACKSPACE,
    KeyboardKey::KEY_COMMA,
    KeyboardKey::KEY_PERIOD,
    KeyboardKey::KEY_Z,
    KeyboardKey::KEY_X,
    KeyboardKey::KEY_J,
    KeyboardKey::KEY_L,
    KeyboardKey::KEY_LEFT_BRACKET,
    KeyboardKey::KEY_RIGHT_BRACKET,
];

impl Input {
    fn capture(rl: &mut RaylibHandle, prev_mouse: Vector2) -> Self {
        use KeyboardKey::*;
        let mut keys = Vec::new();
        while let Some(k) = rl.get_key_pressed() {
            keys.push(k);
        }
        for &k in REPEATABLE {
            if rl.is_key_pressed_repeat(k) && !keys.contains(&k) {
                keys.push(k);
            }
        }
        let mut chars = Vec::new();
        while let Some(c) = rl.get_char_pressed() {
            chars.push(c);
        }
        let dropped = if rl.is_file_dropped() {
            rl.load_dropped_files().paths().iter().map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        };
        // raylib's HiDPI mouse scaling assumes cursor positions are in pixels, but on Wayland they are already
        // logical, so under fractional scaling the pointer drifts. Map window coordinates to screen space ourselves.
        if let Some((ww, wh)) = render::window_size() {
            let (sx, sy) = (rl.get_screen_width() as f32 / ww as f32, rl.get_screen_height() as f32 / wh as f32);
            unsafe { raylib::ffi::SetMouseScale(sx, sy) };
        }
        let mouse = rl.get_mouse_position();
        Self {
            moved: (mouse.x - prev_mouse.x).abs() + (mouse.y - prev_mouse.y).abs() > 0.5,
            mouse,
            left_pressed: rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT),
            left_down: rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT),
            right_pressed: rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_RIGHT),
            middle_pressed: rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_MIDDLE),
            wheel: rl.get_mouse_wheel_move(),
            keys,
            chars,
            shift: rl.is_key_down(KEY_LEFT_SHIFT) || rl.is_key_down(KEY_RIGHT_SHIFT),
            ctrl: rl.is_key_down(KEY_LEFT_CONTROL) || rl.is_key_down(KEY_RIGHT_CONTROL),
            dropped,
            w: rl.get_screen_width() as f32,
            h: rl.get_screen_height() as f32,
            now: rl.get_time(),
            on_screen: rl.is_cursor_on_screen(),
        }
    }
}

pub struct StartupOptions {
    pub files: Vec<String>,
    pub fullscreen: bool,
    pub thumbnails: bool,
    pub snapshot: Option<(String, f64)>,
}

pub struct App {
    // Render resources first: they must be released before the mpv cores.
    thumbs: Option<Thumbnailer>,
    video_gl: GlRenderer,
    video_rt: Option<RenderTexture2D>,
    player: Player,
    fonts: Fonts,
    settings: Settings,

    inp: Input,
    layer: Layer,
    consumed: bool,

    ui_alpha: f32,
    last_activity: f64,
    show_playlist: bool,
    pl_scroll: f32,
    pl_press: Option<(usize, f32)>,
    pl_dragging: bool,
    popup: Option<Popup>,
    modal: Option<Modal>,
    show_stats: bool,
    osd: Option<(String, f64)>,
    toast: Option<(String, f64)>,
    seek_drag: bool,
    last_drag_seek: f64,
    vol_drag: bool,
    pending_click: Option<f64>,
    last_click: (f64, Vector2),
    fullscreen: bool,
    autofit_pending: bool,
    title: String,
    cursor_hidden: bool,
    request_fullscreen: bool,
    quit: bool,
    snapshot: Option<(String, f64)>,
    snapshot_start: Option<f64>,
    test_done: u64,
    sub_margin: Option<i64>,
}

impl App {
    pub fn new(
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        player: Player,
        settings: Settings,
        opts: StartupOptions,
    ) -> Result<Self, String> {
        let video_gl = GlRenderer::new(&player.mpv, true)?;
        let thumbs = if opts.thumbnails {
            Thumbnailer::new().map_err(|e| eprintln!("ghoori: thumbnails disabled: {e}")).ok()
        } else {
            None
        };
        let fonts = Fonts::load(rl, thread, std::env::var("GHOORI_FONT").ok().as_deref());
        let mut app = Self {
            thumbs,
            video_gl,
            video_rt: None,
            player,
            fonts,
            settings,
            inp: Input::default(),
            layer: Layer::Video,
            consumed: false,
            ui_alpha: 1.0,
            last_activity: 0.0,
            show_playlist: false,
            pl_scroll: 0.0,
            pl_press: None,
            pl_dragging: false,
            popup: None,
            modal: None,
            show_stats: false,
            osd: None,
            toast: None,
            seek_drag: false,
            last_drag_seek: 0.0,
            vol_drag: false,
            pending_click: None,
            last_click: (-10.0, v2(0.0, 0.0)),
            fullscreen: false,
            autofit_pending: true,
            title: String::new(),
            cursor_hidden: false,
            request_fullscreen: false,
            quit: false,
            snapshot: opts.snapshot,
            snapshot_start: None,
            test_done: 0,
            sub_margin: None,
        };
        if opts.fullscreen {
            app.toggle_fullscreen(rl);
        }
        if !opts.files.is_empty() {
            let files: Vec<String> = opts.files.iter().map(|f| util::absolutize(f)).collect();
            app.player.open(&files, false);
        }
        Ok(app)
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    pub fn shutdown(&mut self) {
        if self.snapshot_start.is_some() {
            return;
        }
        self.remember_position();
        self.settings.volume = self.player.st.volume;
        self.settings.muted = self.player.st.muted;
        self.settings.save();
    }

    fn remember_position(&self) {
        let st = &self.player.st;
        if !self.player.has_file() || !util::is_local_file(&st.path) {
            return;
        }
        if st.eof || (st.duration > 0.0 && st.time_pos > st.duration - 5.0) {
            // Finished: next time start from the beginning.
            let _ = self.player.command(&["delete-watch-later-config"]);
        } else {
            self.player.save_position();
        }
    }

    fn osd(&mut self, msg: impl Into<String>) {
        self.osd = Some((msg.into(), self.inp.now + 1.6));
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), self.inp.now + 6.0));
    }

    fn wake_ui(&mut self) {
        self.last_activity = self.inp.now;
    }

    // ------------------------------------------------------------------ frame

    pub fn frame(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread) {
        self.inp = Input::capture(rl, self.inp.mouse);
        self.consumed = false;
        if self.snapshot.is_some() {
            self.apply_test_script();
        }
        if rl.window_should_close() {
            self.quit = true;
        }
        if self.inp.moved || self.inp.left_pressed || self.inp.right_pressed || self.inp.wheel != 0.0 {
            self.wake_ui();
        }

        for ev in self.player.poll() {
            self.on_player_event(ev);
        }
        self.handle_drop();
        self.handle_keys(rl);
        self.update_title(rl, thread);
        self.layer = self.layer_at(self.inp.mouse);

        // Fade controls.
        let target = if self.ui_wanted() { 1.0 } else { 0.0 };
        let dt = rl.get_frame_time().min(0.1);
        let speed = if target > self.ui_alpha { 10.0 } else { 4.0 };
        self.ui_alpha += (target - self.ui_alpha) * (1.0 - (-speed * dt).exp());
        if (self.ui_alpha - target).abs() < 0.01 {
            self.ui_alpha = target;
        }

        // Keep subtitles clear of the control bar while it is shown.
        let margin = if self.ui_wanted() && self.inp.h > 0.0 { 22 + (BAR_H / self.inp.h * 720.0) as i64 } else { 22 };
        if self.player.has_file() && self.sub_margin != Some(margin) {
            self.player.set_str("sub-margin-y", &margin.to_string());
            self.sub_margin = Some(margin);
        }

        // Video into its offscreen target.
        let (rw, rh) = (rl.get_render_width(), rl.get_render_height());
        let mut force = false;
        if self.video_rt.as_ref().is_none_or(|t| t.texture.width != rw || t.texture.height != rh) {
            self.video_rt = rl.load_render_texture(thread, rw.max(1) as u32, rh.max(1) as u32).ok();
            force = true;
        }
        let wants = self.video_gl.wants_render();
        if (wants || force)
            && let Some(rt) = &self.video_rt
        {
            self.video_gl.render(rt.id, rw, rh, false);
        }

        // Thumbnailer follows the current file.
        let st = &self.player.st;
        let thumb_src = (self.settings.thumbnails
            && self.player.has_file()
            && st.has_motion_video()
            && st.duration > 0.0
            && util::is_local_file(&st.path))
        .then(|| st.path.clone());
        let aspect = if st.height > 0 { st.width as f64 / st.height as f64 } else { 16.0 / 9.0 };
        if let Some(t) = &mut self.thumbs {
            t.set_source(thumb_src.as_deref());
            t.set_aspect(aspect);
            t.update(rl, thread);
        }

        let mut win_fullscreen = false;
        {
            let mut d = rl.begin_drawing(thread);
            d.clear_background(Color::BLACK);
            self.draw_scene(&mut d);
            self.draw_ui(&mut d);
            if self.inp.left_pressed && !self.consumed && self.layer == Layer::Video {
                win_fullscreen = self.video_click();
            }
            self.video_mouse_extras();
            self.take_snapshot();
        }
        self.video_gl.report_swap();

        if win_fullscreen || std::mem::take(&mut self.request_fullscreen) {
            self.toggle_fullscreen(rl);
        }
        if let Some(t) = self.pending_click
            && self.inp.now - t > DOUBLE_CLICK
        {
            self.pending_click = None;
            if self.player.has_file() {
                self.player.toggle_pause();
                self.flash_play_state();
            }
        }
        self.pending_window_ops(rl);
        self.update_cursor(rl);
    }

    /// Capture the back buffer before it is swapped (it is undefined afterwards on EGL).
    fn take_snapshot(&mut self) {
        let Some((path, delay)) = self.snapshot.clone() else { return };
        // Delay counts from the first rendered frame, not from process start.
        let start = *self.snapshot_start.get_or_insert(self.inp.now);
        if self.inp.now - start >= delay {
            unsafe {
                raylib::ffi::rlDrawRenderBatchActive();
                let img = raylib::ffi::LoadImageFromScreen();
                let c = std::ffi::CString::new(path.clone()).unwrap_or_default();
                raylib::ffi::ExportImage(img, c.as_ptr());
                raylib::ffi::UnloadImage(img);
            }
            eprintln!("ghoori: snapshot written to {path}");
            self.snapshot = None;
            self.quit = true;
        }
    }

    /// Test hook: `GHOORI_TEST="mouse=x,y;click@2=x,y;playlist;stats;help;browser;context;menu=subs;key@1.5=RIGHT"`.
    fn apply_test_script(&mut self) {
        let Ok(script) = std::env::var("GHOORI_TEST") else { return };
        let elapsed = self.snapshot_start.map(|s| self.inp.now - s).unwrap_or(0.0);
        for (i, cmd) in script.split(';').map(str::trim).enumerate() {
            if let Some((at, key)) = cmd.strip_prefix("key@").and_then(|r| r.split_once('=')) {
                let at: f64 = at.parse().unwrap_or(0.0);
                if elapsed >= at && self.test_done & (1 << i) == 0 {
                    self.test_done |= 1 << i;
                    if let Some(k) = test_key(key) {
                        self.inp.keys.push(k);
                    }
                }
                continue;
            }
            if let Some((at, pos)) = cmd.strip_prefix("click@").and_then(|r| r.split_once('=')) {
                let at: f64 = at.parse().unwrap_or(0.0);
                if elapsed >= at
                    && self.test_done & (1 << i) == 0
                    && let Some((x, y)) = pos.split_once(',')
                {
                    self.test_done |= 1 << i;
                    self.inp.mouse = v2(x.parse().unwrap_or(0.0), y.parse().unwrap_or(0.0));
                    self.inp.on_screen = true;
                    self.inp.left_pressed = true;
                    self.inp.left_down = true;
                    self.wake_ui();
                }
                continue;
            }
            match cmd {
                "playlist" if !self.show_playlist => self.exec(Action::TogglePlaylist),
                "stats" => self.show_stats = true,
                "help" => self.modal = Some(Modal::Help),
                "browser" if self.modal.is_none() => self.exec(Action::Browse(BrowseMode::Media)),
                "url" => self.modal = Some(Modal::Url("https://example.com/video.mkv".into())),
                "context" => {
                    self.popup = Some(Popup { kind: PopupKind::Context, anchor: v2(300.0, 120.0), upward: false })
                }
                "menu=subs" => {
                    self.popup = Some(Popup {
                        kind: PopupKind::Subs,
                        anchor: self.popup_anchor(PopupKind::Subs).0,
                        upward: true,
                    })
                }
                "menu=speed" => {
                    self.popup = Some(Popup {
                        kind: PopupKind::Speed,
                        anchor: self.popup_anchor(PopupKind::Speed).0,
                        upward: true,
                    })
                }
                _ => {
                    if let Some((x, y)) = cmd.strip_prefix("mouse=").and_then(|v| v.split_once(',')) {
                        self.inp.mouse = v2(x.parse().unwrap_or(0.0), y.parse().unwrap_or(0.0));
                        self.inp.on_screen = true;
                        self.wake_ui();
                    }
                }
            }
        }
    }

    fn ui_wanted(&self) -> bool {
        let st = &self.player.st;
        if !self.player.has_file() || st.paused || self.modal.is_some() || self.popup.is_some() {
            return true;
        }
        if self.seek_drag || self.vol_drag || self.show_playlist {
            return true;
        }
        if !self.inp.on_screen {
            return false;
        }
        let over_bar = self.inp.mouse.y > self.inp.h - BAR_H || self.inp.mouse.y < TOP_H;
        over_bar || self.inp.now - self.last_activity < HIDE_AFTER
    }

    fn on_player_event(&mut self, ev: PlayerEvent) {
        match ev {
            PlayerEvent::FileLoaded => {
                let path = self.player.st.path.clone();
                if !path.is_empty() {
                    let entry = if util::is_url(&path) { path } else { util::absolutize(&path) };
                    self.settings.push_recent(&entry);
                }
                self.toast = None;
            }
            PlayerEvent::VideoReconfig => {}
            PlayerEvent::PlaybackRestart => {}
            PlayerEvent::LoadFailed { error } => {
                let name = self
                    .player
                    .st
                    .playlist
                    .iter()
                    .find(|p| p.current)
                    .map(|p| p.name())
                    .unwrap_or_else(|| "file".into());
                self.toast(format!("Could not play {name}: {error}"));
            }
            PlayerEvent::Shutdown => self.quit = true,
        }
    }

    fn handle_drop(&mut self) {
        if self.inp.dropped.is_empty() {
            return;
        }
        let dropped = std::mem::take(&mut self.inp.dropped);
        let all_subs = dropped.iter().all(|p| util::is_subtitle(p));
        if all_subs && self.player.has_file() {
            for p in &dropped {
                self.player.cmd(&["sub-add", p, "select"]);
            }
            self.osd(format!("Loaded {} subtitle file(s)", dropped.len()));
            return;
        }
        let over_panel = self.show_playlist && inside(self.panel_rect(), self.inp.mouse);
        let append = (self.inp.shift || over_panel) && self.player.has_file();
        let n = dropped.len();
        self.exec(Action::Open(dropped, append));
        if append {
            self.osd(format!("Added {n} item(s) to playlist"));
        }
    }

    fn update_title(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread) {
        let st = &self.player.st;
        let t = if self.player.has_file() && !st.title.is_empty() {
            format!("{} — Ghoori", st.title)
        } else {
            "Ghoori".to_string()
        };
        if t != self.title {
            rl.set_window_title(thread, &t);
            self.title = t;
        }
    }

    fn pending_window_ops(&mut self, rl: &mut RaylibHandle) {
        let st = &self.player.st;
        if self.autofit_pending && st.width > 0 && st.height > 0 && st.has_motion_video() {
            self.autofit_pending = false;
            if !self.fullscreen {
                let mon = get_current_monitor();
                // Monitor modes and video sizes are physical pixels; window sizes are logical.
                let dpi = rl.get_window_scale_dpi();
                let dpi = if dpi.x > 0.0 { dpi.x as f64 } else { 1.0 };
                let (mw, mh) = (get_monitor_width(mon) as f64 / dpi, get_monitor_height(mon) as f64 / dpi);
                let (vw, vh) = (st.width as f64 / dpi, st.height as f64 / dpi);
                let scale = (mw * 0.8 / vw).min(mh * 0.8 / vh).min(1.0);
                let (w, h) = ((vw * scale) as i32, (vh * scale) as i32);
                let (w, h) = (w.max(480), h.max(300));
                rl.set_window_size(w, h);
                // Wayland compositors place windows themselves.
                if !crate::render::is_wayland() {
                    let mpos = unsafe { raylib::ffi::GetMonitorPosition(mon) };
                    rl.set_window_position(mpos.x as i32 + (mw as i32 - w) / 2, mpos.y as i32 + (mh as i32 - h) / 2);
                }
            }
        }
    }

    fn update_cursor(&mut self, rl: &mut RaylibHandle) {
        let hide = self.ui_alpha == 0.0 && self.player.has_file() && self.player.st.video_track().is_some();
        if hide != self.cursor_hidden {
            if hide {
                rl.hide_cursor()
            } else {
                rl.show_cursor()
            }
            self.cursor_hidden = hide;
        }
    }

    fn toggle_fullscreen(&mut self, rl: &mut RaylibHandle) {
        rl.toggle_borderless_windowed();
        self.fullscreen = unsafe { raylib::ffi::IsWindowState(ConfigFlags::FLAG_BORDERLESS_WINDOWED_MODE as u32) };
    }

    fn flash_play_state(&mut self) {
        // OSD reflects the state *after* the toggle request.
        let msg = if self.player.st.paused { "Play" } else { "Pause" };
        self.osd(msg);
    }

    /// Returns true when the click should toggle fullscreen (double click).
    fn video_click(&mut self) -> bool {
        let (t, p) = self.last_click;
        let now = self.inp.now;
        let m = self.inp.mouse;
        if now - t < DOUBLE_CLICK && (p.x - m.x).abs() < 8.0 && (p.y - m.y).abs() < 8.0 {
            self.pending_click = None;
            self.last_click = (-10.0, m);
            return true;
        }
        self.last_click = (now, m);
        if self.player.has_file() {
            self.pending_click = Some(now);
        }
        false
    }

    fn video_mouse_extras(&mut self) {
        if self.layer != Layer::Video || self.consumed {
            return;
        }
        if self.inp.wheel != 0.0 && self.player.has_file() {
            self.exec(Action::VolumeBy(self.inp.wheel as f64 * 5.0));
        }
        if self.inp.right_pressed {
            self.popup = Some(Popup { kind: PopupKind::Context, anchor: self.inp.mouse, upward: false });
        }
        if self.inp.middle_pressed {
            self.exec(Action::ToggleMute);
        }
    }

    // ---------------------------------------------------------------- actions

    pub fn exec(&mut self, a: Action) {
        let p = &self.player;
        let st = &p.st;
        match a {
            Action::TogglePause => {
                if p.has_file() {
                    p.toggle_pause();
                    self.flash_play_state();
                }
            }
            Action::SeekRel(s) => {
                if p.has_file() {
                    p.seek_relative(s, false);
                    let target = (st.time_pos + s).clamp(0.0, st.duration.max(0.0));
                    let msg = if st.duration > 0.0 {
                        format!("{}  {} / {}", if s < 0.0 { "«" } else { "»" }, fmt_time(target), fmt_time(st.duration))
                    } else {
                        format!("{:+} s", s)
                    };
                    self.osd(msg);
                }
            }
            Action::SeekPercent(pc) => {
                if p.has_file() && st.duration > 0.0 {
                    p.seek_absolute(st.duration * pc / 100.0, false);
                    self.osd(format!("{} / {}", fmt_time(st.duration * pc / 100.0), fmt_time(st.duration)));
                }
            }
            Action::FrameStep(fwd) => p.cmd(&[if fwd { "frame-step" } else { "frame-back-step" }]),
            Action::VolumeBy(dv) => {
                let v = (st.volume + dv).clamp(0.0, st.volume_max.max(100.0)).round();
                p.set_volume(v);
                if st.muted && dv > 0.0 {
                    p.set_bool("mute", false);
                }
                self.osd(format!("Volume {v:.0}%"));
            }
            Action::ToggleMute => {
                let m = !st.muted;
                p.set_bool("mute", m);
                self.osd(if m { "Muted" } else { "Unmuted" });
            }
            Action::SpeedBy(ds) => {
                let s = ((st.speed + ds) * 100.0).round() / 100.0;
                self.exec(Action::SetSpeed(s));
            }
            Action::SetSpeed(s) => {
                let s = s.clamp(0.1, 8.0);
                p.set_f64("speed", s);
                self.osd(format!("Speed {s:.2}×"));
            }
            Action::Next | Action::Prev => {
                let next = a == Action::Next;
                let n = st.playlist.len() as i64;
                let can = if next {
                    st.playlist_pos + 1 < n || st.loop_playlist
                } else {
                    st.playlist_pos > 0 || st.loop_playlist
                };
                if can {
                    self.remember_position();
                    self.player.cmd(&[if next { "playlist-next" } else { "playlist-prev" }, "force"]);
                } else {
                    self.osd(if next { "End of playlist" } else { "Start of playlist" });
                }
            }
            Action::ChapterBy(n) => {
                if st.chapters.is_empty() {
                    self.osd("No chapters");
                } else {
                    p.cmd(&["add", "chapter", &n.to_string()]);
                }
            }
            Action::PlayIndex(i) => {
                if i as i64 != st.playlist_pos || st.idle {
                    self.remember_position();
                }
                self.player.cmd(&["playlist-play-index", &i.to_string()]);
                self.player.set_bool("pause", false);
            }
            Action::RemoveIndex(i) => p.cmd(&["playlist-remove", &i.to_string()]),
            Action::MoveIndex(from, to) => p.cmd(&["playlist-move", &from.to_string(), &to.to_string()]),
            Action::Shuffle => {
                p.cmd(&["playlist-shuffle"]);
                self.osd("Playlist shuffled");
            }
            Action::ClearPlaylist => {
                // Keep the playing entry, drop the rest.
                p.cmd(&["playlist-clear"]);
                self.osd("Playlist cleared");
            }
            Action::ToggleLoopPlaylist => {
                let on = !st.loop_playlist;
                p.set_str("loop-playlist", if on { "inf" } else { "no" });
                self.osd(if on { "Repeat playlist: on" } else { "Repeat playlist: off" });
            }
            Action::ToggleFullscreen => {
                // Needs the raylib handle; resolved via the click path.
                self.pending_click = None;
                self.last_click = (-10.0, self.inp.mouse);
                self.request_fullscreen = true;
            }
            Action::SelectTrack(kind, id) => {
                p.select_track(kind, id);
                let label = match id {
                    None => "off".to_string(),
                    Some(id) => {
                        st.tracks.iter().find(|t| t.kind == kind && t.id == id).map(Track::label).unwrap_or_default()
                    }
                };
                let what = if kind == "sub" { "Subtitles" } else { "Audio" };
                self.osd(format!("{what}: {label}"));
            }
            Action::Cycle(kind) => {
                let list: Vec<i64> = st.tracks.iter().filter(|t| t.kind == kind).map(|t| t.id).collect();
                if list.is_empty() {
                    self.osd(if kind == "sub" { "No subtitles" } else { "No audio tracks" });
                    return;
                }
                let cur = st.tracks.iter().find(|t| t.kind == kind && t.selected).map(|t| t.id);
                let next = match cur.and_then(|c| list.iter().position(|&i| i == c)) {
                    Some(i) if i + 1 < list.len() => Some(list[i + 1]),
                    Some(_) if kind == "sub" => None,
                    Some(_) => Some(list[0]),
                    None => Some(list[0]),
                };
                self.exec(Action::SelectTrack(kind, next));
            }
            Action::ToggleSubs => {
                let v = !st.sub_visible;
                p.set_bool("sub-visibility", v);
                self.osd(if v { "Subtitles visible" } else { "Subtitles hidden" });
            }
            Action::SubDelay(d) => {
                let nd = st.sub_delay + d;
                p.set_f64("sub-delay", nd);
                self.osd(format!("Subtitle delay {:+.0} ms", nd * 1000.0));
            }
            Action::ToggleLoopFile => {
                let on = !st.loop_file;
                p.set_str("loop-file", if on { "inf" } else { "no" });
                self.osd(if on { "Repeat file: on" } else { "Repeat file: off" });
            }
            Action::AbLoop => {
                p.cmd(&["ab-loop"]);
                let msg = match (st.ab_a, st.ab_b) {
                    (None, _) => format!("Loop A set at {}", fmt_time(st.time_pos)),
                    (Some(a), None) => format!("Looping {} → {}", fmt_time(a), fmt_time(st.time_pos)),
                    _ => "A-B loop cleared".to_string(),
                };
                self.osd(msg);
            }
            Action::ClearAbLoop => {
                p.set_str("ab-loop-a", "no");
                p.set_str("ab-loop-b", "no");
                self.osd("A-B loop cleared");
            }
            Action::Screenshot => {
                if !p.has_file() {
                    return;
                }
                let dir = p.get_string("screenshot-directory").unwrap_or_default();
                let _ = std::fs::create_dir_all(&dir);
                match p.command(&["screenshot", "subtitles"]) {
                    Ok(()) => self.osd(format!("Screenshot saved to {dir}")),
                    Err(e) => self.toast(format!("Screenshot failed: {e}")),
                }
            }
            Action::TogglePlaylist => {
                self.show_playlist = !self.show_playlist;
                if self.show_playlist {
                    self.scroll_playlist_to_current();
                }
            }
            Action::ToggleStats => self.show_stats = !self.show_stats,
            Action::ToggleHelp => {
                self.modal = if matches!(self.modal, Some(Modal::Help)) { None } else { Some(Modal::Help) };
            }
            Action::ToggleRemaining => self.settings.show_remaining = !self.settings.show_remaining,
            Action::ToggleThumbs => {
                self.settings.thumbnails = !self.settings.thumbnails;
                self.osd(if self.settings.thumbnails { "Seek previews: on" } else { "Seek previews: off" });
            }
            Action::Browse(mode) => {
                let start = match mode {
                    BrowseMode::Subtitle if util::is_local_file(&st.path) => {
                        PathBuf::from(&st.path).parent().map(PathBuf::from).unwrap_or_else(util::home_dir)
                    }
                    _ => self
                        .settings
                        .browser_dir
                        .as_ref()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| util::user_dir("VIDEOS", "Videos")),
                };
                self.popup = None;
                self.modal = Some(Modal::Browser(Browser::new(mode, &start)));
            }
            Action::OpenUrlDialog => {
                self.popup = None;
                self.modal = Some(Modal::Url(String::new()));
            }
            Action::Popup(kind) => {
                let anchor = self.popup_anchor(kind);
                let same = self.popup.as_ref().is_some_and(|p| p.kind == kind);
                self.popup = if same { None } else { Some(Popup { kind, anchor: anchor.0, upward: anchor.1 }) };
            }
            Action::Open(items, append) => {
                let items: Vec<String> =
                    items.iter().map(|i| util::absolutize(i.trim())).filter(|i| !i.is_empty()).collect();
                if !items.is_empty() {
                    self.player.open(&items, append);
                    if !append {
                        self.player.set_bool("pause", false);
                    }
                }
            }
            Action::Stop => {
                self.remember_position();
                self.player.cmd(&["stop"]);
            }
            Action::Quit => self.quit = true,
        }
    }

    fn popup_anchor(&self, kind: PopupKind) -> (Vector2, bool) {
        let (w, h) = (self.inp.w, self.inp.h);
        let y = h - BAR_H + 4.0;
        match kind {
            PopupKind::Context => (self.inp.mouse, false),
            PopupKind::Subs => (v2(w - 118.0, y), true),
            PopupKind::Audio => (v2(w - 158.0, y), true),
            PopupKind::Speed => (v2(w - 208.0, y), true),
        }
    }

    // ----------------------------------------------------------------- keys

    fn handle_keys(&mut self, rl: &mut RaylibHandle) {
        use KeyboardKey::*;
        if self.inp.keys.is_empty() && self.inp.chars.is_empty() {
            return;
        }
        self.wake_ui();
        let inp = std::mem::take(&mut self.inp.keys);
        let chars = std::mem::take(&mut self.inp.chars);
        let key = |k: KeyboardKey| inp.contains(&k);
        let (shift, ctrl) = (self.inp.shift, self.inp.ctrl);

        // Modal dialogs swallow the keyboard.
        if let Some(modal) = &mut self.modal {
            match modal {
                Modal::Help => {
                    if key(KEY_ESCAPE) || key(KEY_F1) || key(KEY_H) || key(KEY_ENTER) || chars.contains(&'?') {
                        self.modal = None;
                    }
                }
                Modal::Url(text) => {
                    if key(KEY_ESCAPE) {
                        self.modal = None;
                    } else if key(KEY_ENTER) || key(KEY_KP_ENTER) {
                        let t = text.trim().to_string();
                        self.modal = None;
                        if !t.is_empty() {
                            self.exec(Action::Open(vec![t], shift && self.player.has_file()));
                        }
                    } else {
                        if ctrl && key(KEY_V) {
                            if let Ok(clip) = rl.get_clipboard_text() {
                                text.push_str(clip.lines().next().unwrap_or("").trim());
                            }
                        } else if ctrl && (key(KEY_U) || key(KEY_BACKSPACE)) {
                            text.clear();
                        } else if key(KEY_BACKSPACE) {
                            text.pop();
                        }
                        if !ctrl {
                            text.extend(chars.iter().filter(|c| !c.is_control()));
                        }
                    }
                }
                Modal::Browser(b) => {
                    let page = 10;
                    if key(KEY_ESCAPE) {
                        self.settings.browser_dir = Some(b.cwd.to_string_lossy().into_owned());
                        self.modal = None;
                        return;
                    }
                    if key(KEY_UP) {
                        b.move_sel(-1);
                    }
                    if key(KEY_DOWN) {
                        b.move_sel(1);
                    }
                    if key(KEY_PAGE_UP) {
                        b.move_sel(-page);
                    }
                    if key(KEY_PAGE_DOWN) {
                        b.move_sel(page);
                    }
                    if key(KEY_HOME) {
                        b.move_sel(-100_000);
                    }
                    if key(KEY_END) {
                        b.move_sel(100_000);
                    }
                    if ctrl && key(KEY_H) {
                        b.show_hidden = !b.show_hidden;
                        b.refresh();
                    }
                    if key(KEY_BACKSPACE) {
                        if b.filter.is_empty() {
                            b.up();
                        } else {
                            let mut f = b.filter.clone();
                            f.pop();
                            b.set_filter(f);
                        }
                    }
                    if !ctrl && !chars.is_empty() {
                        let mut f = b.filter.clone();
                        f.extend(chars.iter().filter(|c| !c.is_control()));
                        b.set_filter(f);
                    }
                    if key(KEY_ENTER) || key(KEY_KP_ENTER) {
                        if ctrl {
                            self.browser_play_folder();
                        } else {
                            self.browser_activate(shift);
                        }
                    }
                    // Keep the selection on screen.
                    self.browser_scroll_to_selection();
                }
            }
            return;
        }

        if self.popup.is_some() && key(KEY_ESCAPE) {
            self.popup = None;
            return;
        }

        let mut acts: Vec<Action> = Vec::new();
        let small = if shift { 1.0 } else { 5.0 };
        if key(KEY_SPACE) || key(KEY_K) {
            acts.push(Action::TogglePause);
        }
        if ctrl {
            if key(KEY_O) {
                acts.push(Action::Browse(BrowseMode::Media));
            }
            if key(KEY_U) || key(KEY_L) {
                acts.push(Action::OpenUrlDialog);
            }
            if key(KEY_V)
                && let Ok(clip) = rl.get_clipboard_text()
            {
                let items: Vec<String> = clip
                    .lines()
                    .map(|l| l.trim().trim_start_matches("file://").to_string())
                    .filter(|l| !l.is_empty())
                    .collect();
                if !items.is_empty() {
                    let append = shift && self.player.has_file();
                    acts.push(Action::Open(items, append));
                }
            }
            if key(KEY_RIGHT) {
                acts.push(Action::Next);
            }
            if key(KEY_LEFT) {
                acts.push(Action::Prev);
            }
            if key(KEY_Q) || key(KEY_W) {
                acts.push(Action::Quit);
            }
            if key(KEY_T) {
                acts.push(Action::ToggleThumbs);
            }
            if key(KEY_S) {
                acts.push(Action::Shuffle);
            }
        } else {
            if key(KEY_LEFT) {
                acts.push(Action::SeekRel(-small));
            }
            if key(KEY_RIGHT) {
                acts.push(Action::SeekRel(small));
            }
            if key(KEY_J) {
                acts.push(Action::SeekRel(-10.0));
            }
            if key(KEY_L) {
                acts.push(Action::SeekRel(10.0));
            }
            if key(KEY_UP) {
                acts.push(if shift { Action::SeekRel(60.0) } else { Action::VolumeBy(5.0) });
            }
            if key(KEY_DOWN) {
                acts.push(if shift { Action::SeekRel(-60.0) } else { Action::VolumeBy(-5.0) });
            }
            if key(KEY_M) {
                acts.push(Action::ToggleMute);
            }
            if key(KEY_F) || key(KEY_F11) {
                acts.push(Action::ToggleFullscreen);
            }
            if key(KEY_ESCAPE) {
                if self.fullscreen {
                    acts.push(Action::ToggleFullscreen);
                } else if self.show_playlist {
                    self.show_playlist = false;
                } else if self.show_stats {
                    self.show_stats = false;
                }
            }
            if key(KEY_PAGE_UP) {
                acts.push(Action::ChapterBy(-1));
            }
            if key(KEY_PAGE_DOWN) {
                acts.push(Action::ChapterBy(1));
            }
            if key(KEY_N) {
                acts.push(if shift { Action::Prev } else { Action::Next });
            }
            if key(KEY_P) {
                acts.push(Action::Prev);
            }
            if key(KEY_HOME) {
                acts.push(Action::SeekPercent(0.0));
            }
            if key(KEY_COMMA) {
                acts.push(Action::FrameStep(false));
            }
            if key(KEY_PERIOD) {
                acts.push(Action::FrameStep(true));
            }
            if key(KEY_LEFT_BRACKET) {
                acts.push(Action::SpeedBy(-0.1));
            }
            if key(KEY_RIGHT_BRACKET) {
                acts.push(Action::SpeedBy(0.1));
            }
            if key(KEY_BACKSPACE) {
                acts.push(Action::SetSpeed(1.0));
            }
            if key(KEY_V) {
                acts.push(Action::ToggleSubs);
            }
            if key(KEY_C) {
                acts.push(Action::Cycle("sub"));
            }
            if key(KEY_A) {
                acts.push(Action::Cycle("audio"));
            }
            if key(KEY_Z) {
                acts.push(Action::SubDelay(-0.1));
            }
            if key(KEY_X) {
                acts.push(Action::SubDelay(0.1));
            }
            if key(KEY_R) {
                acts.push(if shift { Action::AbLoop } else { Action::ToggleLoopFile });
            }
            if key(KEY_S) {
                acts.push(Action::Screenshot);
            }
            if key(KEY_TAB) {
                acts.push(Action::TogglePlaylist);
            }
            if key(KEY_I) {
                acts.push(Action::ToggleStats);
            }
            if key(KEY_F1) || key(KEY_H) || chars.contains(&'?') {
                acts.push(Action::ToggleHelp);
            }
            if key(KEY_Q) {
                acts.push(Action::Quit);
            }
            if key(KEY_DELETE)
                && self.show_playlist
                && let Some(i) = self.hovered_playlist_row()
            {
                acts.push(Action::RemoveIndex(i));
            }
            for (k, pc) in [
                (KEY_ZERO, 0.0),
                (KEY_ONE, 10.0),
                (KEY_TWO, 20.0),
                (KEY_THREE, 30.0),
                (KEY_FOUR, 40.0),
                (KEY_FIVE, 50.0),
                (KEY_SIX, 60.0),
                (KEY_SEVEN, 70.0),
                (KEY_EIGHT, 80.0),
                (KEY_NINE, 90.0),
            ] {
                if key(k) && !shift {
                    acts.push(Action::SeekPercent(pc));
                }
            }
        }
        for a in acts {
            if a == Action::ToggleFullscreen {
                self.toggle_fullscreen(rl);
            } else {
                self.exec(a);
            }
        }
    }

    // --------------------------------------------------------------- layout

    fn panel_rect(&self) -> Rectangle {
        let w = PANEL_W.min(self.inp.w - 40.0);
        Rectangle::new(self.inp.w - w, 0.0, w, (self.inp.h - BAR_H).max(0.0))
    }

    fn modal_rect(&self) -> Rectangle {
        let (w, h) = (self.inp.w, self.inp.h);
        let (mw, mh) = match self.modal {
            Some(Modal::Url(_)) => (720f32.min(w - 40.0), 170.0),
            Some(Modal::Help) => (820f32.min(w - 40.0), 600f32.min(h - 40.0)),
            _ => (940f32.min(w - 40.0), 660f32.min(h - 40.0)),
        };
        Rectangle::new(((w - mw) / 2.0).round(), ((h - mh) / 2.0).round(), mw, mh)
    }

    fn layer_at(&self, m: Vector2) -> Layer {
        if self.modal.is_some() {
            return Layer::Modal;
        }
        if self.popup.is_some() {
            return Layer::Popup;
        }
        if self.show_playlist && inside(self.panel_rect(), m) {
            return Layer::Panel;
        }
        if self.ui_alpha > 0.05 && (m.y > self.inp.h - BAR_H || (m.y < TOP_H && self.player.has_file())) {
            return Layer::Bar;
        }
        Layer::Video
    }

    fn hover(&self, r: Rectangle, layer: Layer) -> bool {
        self.layer == layer && inside(r, self.inp.mouse)
    }

    fn click(&mut self, r: Rectangle, layer: Layer) -> bool {
        if !self.consumed && self.inp.left_pressed && self.hover(r, layer) {
            self.consumed = true;
            return true;
        }
        false
    }

    fn icon_button<D, F>(&mut self, d: &mut D, c: Vector2, size: f32, layer: Layer, active: bool, draw: F) -> bool
    where
        D: RaylibDraw,
        F: FnOnce(&mut D, Vector2, f32, Color),
    {
        let a = if layer == Layer::Bar { self.ui_alpha } else { 1.0 };
        let r = Rectangle::new(c.x - size / 2.0 - 4.0, c.y - size / 2.0 - 4.0, size + 8.0, size + 8.0);
        let hot = self.hover(r, layer);
        if hot {
            d.draw_circle_v(c, size * 0.72, fade(Color::WHITE, 0.12 * a));
        }
        let col = if active {
            ACCENT
        } else if hot {
            Color::WHITE
        } else {
            FG
        };
        draw(d, c, size * 0.62, fade(col, a));
        self.click(r, layer)
    }

    // ---------------------------------------------------------------- scene

    fn draw_scene(&mut self, d: &mut RaylibDrawHandle) {
        let (w, h) = (self.inp.w, self.inp.h);
        if !self.player.has_file() {
            self.draw_idle(d);
            return;
        }
        if self.player.st.video_track().is_some() {
            if let Some(rt) = &self.video_rt {
                let src = Rectangle::new(0.0, 0.0, rt.texture.width as f32, rt.texture.height as f32);
                d.draw_texture_pro(rt.texture(), src, Rectangle::new(0.0, 0.0, w, h), v2(0.0, 0.0), 0.0, Color::WHITE);
            }
        } else if !self.player.st.tracks.is_empty() {
            self.draw_audio_only(d);
        }
        let st = &self.player.st;
        if st.buffering || (st.tracks.is_empty() && !st.idle) {
            self.draw_spinner(d, v2(w / 2.0, h / 2.0));
            if st.buffering {
                let t = format!("Buffering {}%", st.cache_percent);
                let tw = self.fonts.measure(&t, 18.0);
                self.fonts.draw_shadow(d, &t, v2((w - tw) / 2.0, h / 2.0 + 34.0), 18.0, FG);
            }
        }
    }

    fn draw_spinner(&self, d: &mut RaylibDrawHandle, c: Vector2) {
        let a = (self.inp.now * 300.0 % 360.0) as f32;
        d.draw_ring(c, 16.0, 21.0, a, a + 270.0, 32, fade(ACCENT, 0.9));
    }

    fn draw_backdrop(&self, d: &mut RaylibDrawHandle) {
        let (w, h) = (self.inp.w as i32, self.inp.h as i32);
        d.draw_rectangle_gradient_v(0, 0, w, h, Color::new(28, 26, 34, 255), Color::new(10, 10, 13, 255));
    }

    fn draw_idle(&mut self, d: &mut RaylibDrawHandle) {
        self.draw_backdrop(d);
        let (w, h) = (self.inp.w, self.inp.h);
        let recents: Vec<String> = self.settings.recent.iter().take(6).cloned().collect();
        let list_h = if recents.is_empty() { 0.0 } else { 40.0 + recents.len() as f32 * 34.0 };
        let block_h = 300.0 + list_h;
        let mut y = ((h - block_h) / 2.0).max(20.0);
        let logo = (h * 0.16).clamp(60.0, 130.0);
        icons::teapot(d, v2(w / 2.0, y + logo * 0.8), logo, Color::new(230, 225, 215, 255), ACCENT);
        y += logo * 1.55;
        let title = "Ghoori";
        let tw = self.fonts.measure(title, 44.0);
        self.fonts.draw(d, title, v2((w - tw) / 2.0, y), 44.0, FG);
        y += 56.0;
        let hint = "Drop files or folders here to start playing";
        let hw = self.fonts.measure(hint, 18.0);
        self.fonts.draw(d, hint, v2((w - hw) / 2.0, y), 18.0, DIM);
        y += 40.0;

        // Primary buttons.
        let labels = [("Open file…", "Ctrl+O"), ("Open URL…", "Ctrl+U")];
        let bw = 190.0;
        let gap = 16.0;
        let x0 = (w - (bw * 2.0 + gap)) / 2.0;
        for (i, (label, key)) in labels.iter().enumerate() {
            let r = Rectangle::new(x0 + i as f32 * (bw + gap), y, bw, 42.0);
            let hot = self.hover(r, Layer::Video);
            d.draw_rectangle_rounded(
                r,
                0.3,
                8,
                if i == 0 {
                    fade(ACCENT, if hot { 1.0 } else { 0.88 })
                } else {
                    fade(Color::WHITE, if hot { 0.16 } else { 0.09 })
                },
            );
            let col = if i == 0 { Color::new(30, 22, 10, 255) } else { FG };
            let lw = self.fonts.measure(label, 18.0);
            let kw = self.fonts.measure(key, 13.0);
            let total = lw + 10.0 + kw;
            self.fonts.draw(d, label, v2(r.x + (bw - total) / 2.0, r.y + 10.0), 18.0, col);
            self.fonts.draw(d, key, v2(r.x + (bw - total) / 2.0 + lw + 10.0, r.y + 14.0), 13.0, fade(col, 0.65));
            if self.click(r, Layer::Video) {
                self.exec(if i == 0 { Action::Browse(BrowseMode::Media) } else { Action::OpenUrlDialog });
            }
        }
        y += 70.0;

        if !recents.is_empty() {
            let lw = 560f32.min(w - 40.0);
            let x = (w - lw) / 2.0;
            self.fonts.draw(d, "RECENT", v2(x + 4.0, y), 13.0, DIM);
            y += 24.0;
            for r in &recents {
                let row = Rectangle::new(x, y, lw, 32.0);
                let hot = self.hover(row, Layer::Video);
                if hot {
                    d.draw_rectangle_rounded(row, 0.25, 6, ROW_HOVER);
                }
                icons::file(d, v2(x + 18.0, y + 16.0), 16.0, if hot { ACCENT } else { DIM });
                let name = util::display_name(r);
                let fitted = self.fonts.fit(&name, 17.0, lw - 60.0).into_owned();
                self.fonts.draw(d, &fitted, v2(x + 38.0, y + 6.0), 17.0, if hot { Color::WHITE } else { FG });
                if self.click(row, Layer::Video) {
                    self.exec(Action::Open(vec![r.clone()], false));
                }
                y += 34.0;
            }
        }
        let help = "Press F1 for keyboard shortcuts";
        let hw = self.fonts.measure(help, 14.0);
        self.fonts.draw(d, help, v2((w - hw) / 2.0, h - BAR_H - 34.0), 14.0, fade(DIM, 0.7));
    }

    fn draw_audio_only(&mut self, d: &mut RaylibDrawHandle) {
        self.draw_backdrop(d);
        let (w, h) = (self.inp.w, self.inp.h);
        let st = &self.player.st;
        let c = v2(w / 2.0, h / 2.0 - 70.0);
        let r = (h * 0.14).clamp(50.0, 110.0);
        d.draw_circle_v(c, r, Color::new(40, 37, 48, 255));
        // Gentle pulse while playing.
        let pulse = if st.paused { 0.0 } else { ((self.inp.now * 2.4).sin() * 0.5 + 0.5) as f32 };
        d.draw_ring(c, r, r + 3.0 + pulse * 4.0, 0.0, 360.0, 64, fade(ACCENT, 0.5 + pulse * 0.3));
        icons::note(d, c, r * 1.0, ACCENT);
        let title = st.meta("title").map(str::to_string).unwrap_or_else(|| st.title.clone());
        let mut y = c.y + r + 34.0;
        let t = self.fonts.fit(&title, 30.0, w - 80.0).into_owned();
        let tw = self.fonts.measure(&t, 30.0);
        self.fonts.draw(d, &t, v2((w - tw) / 2.0, y), 30.0, FG);
        y += 42.0;
        let artist = st.meta("artist").or(st.meta("album_artist")).unwrap_or("");
        let album = st.meta("album").unwrap_or("");
        for (line, size) in [(artist, 20.0), (album, 17.0)] {
            if line.is_empty() {
                continue;
            }
            let t = self.fonts.fit(line, size, w - 80.0).into_owned();
            let tw = self.fonts.measure(&t, size);
            self.fonts.draw(d, &t, v2((w - tw) / 2.0, y), size, DIM);
            y += size + 12.0;
        }
    }

    // ------------------------------------------------------------------- UI

    fn draw_ui(&mut self, d: &mut RaylibDrawHandle) {
        if self.player.has_file() {
            self.draw_top_bar(d);
        }
        self.draw_bottom_bar(d);
        if self.show_playlist {
            self.draw_playlist(d);
        }
        if self.show_stats && self.player.has_file() {
            self.draw_stats(d);
        }
        self.draw_osd(d);
        if self.popup.is_some() {
            self.draw_popup(d);
        }
        if self.modal.is_some() {
            self.draw_modal(d);
        }
    }

    fn draw_top_bar(&mut self, d: &mut RaylibDrawHandle) {
        let a = self.ui_alpha;
        if a <= 0.0 {
            return;
        }
        let w = self.inp.w;
        d.draw_rectangle_gradient_v(0, 0, w as i32, 90, fade(Color::BLACK, 0.7 * a), fade(Color::BLACK, 0.0));
        let st = &self.player.st;
        let right = if st.playlist.len() > 1 {
            format!("{} / {}", st.playlist_pos + 1, st.playlist.len())
        } else {
            String::new()
        };
        let rw = self.fonts.measure(&right, 16.0);
        let title = self.fonts.fit(&st.title, 20.0, w - rw - 60.0).into_owned();
        self.fonts.draw_shadow(d, &title, v2(18.0, 14.0), 20.0, fade(FG, a));
        if !right.is_empty() {
            self.fonts.draw_shadow(d, &right, v2(w - rw - 18.0, 17.0), 16.0, fade(DIM, a));
        }
        if let Some(ch) = st.current_chapter() {
            let t = self.fonts.fit(&ch.title, 15.0, w - 60.0).into_owned();
            self.fonts.draw_shadow(d, &t, v2(18.0, 40.0), 15.0, fade(DIM, a));
        }
    }

    fn draw_bottom_bar(&mut self, d: &mut RaylibDrawHandle) {
        let a = self.ui_alpha;
        if a <= 0.0 {
            self.seek_drag = false;
            self.vol_drag = false;
            return;
        }
        let (w, h) = (self.inp.w, self.inp.h);
        d.draw_rectangle_gradient_v(
            0,
            (h - BAR_H - 50.0) as i32,
            w as i32,
            (BAR_H + 50.0) as i32,
            fade(Color::BLACK, 0.0),
            fade(Color::BLACK, 0.82 * a),
        );

        self.draw_seekbar(d);

        let cy = h - 28.0;
        let st = &self.player.st;
        let has = self.player.has_file();
        let paused = st.paused || !has;
        let ended = has && st.eof;
        let mut x = 30.0;
        if self.icon_button(d, v2(x, cy), 26.0, Layer::Bar, false, |d, c, s, col| icons::skip(d, c, s, col, false)) {
            self.exec(Action::Prev);
        }
        x += 46.0;
        let clicked = self.icon_button(d, v2(x, cy), 34.0, Layer::Bar, false, |d, c, s, col| {
            if ended {
                icons::replay(d, c, s, col)
            } else if paused {
                icons::play(d, c, s, col)
            } else {
                icons::pause(d, c, s, col)
            }
        });
        if clicked {
            if has { self.exec(Action::TogglePause) } else { self.exec(Action::Browse(BrowseMode::Media)) }
        }
        x += 46.0;
        if self.icon_button(d, v2(x, cy), 26.0, Layer::Bar, false, |d, c, s, col| icons::skip(d, c, s, col, true)) {
            self.exec(Action::Next);
        }
        x += 48.0;

        // Volume.
        let (vol, muted, vmax) = (self.player.st.volume, self.player.st.muted, self.player.st.volume_max.max(100.0));
        if self
            .icon_button(d, v2(x, cy), 28.0, Layer::Bar, false, |d, c, s, col| icons::volume(d, c, s, col, vol, muted))
        {
            self.exec(Action::ToggleMute);
        }
        x += 24.0;
        let vr = Rectangle::new(x, cy - 12.0, 96.0, 24.0);
        let vhot = self.hover(vr, Layer::Bar);
        if self.click(vr, Layer::Bar) {
            self.vol_drag = true;
        }
        if self.vol_drag {
            if self.inp.left_down {
                let frac = ((self.inp.mouse.x - vr.x - 6.0) / (vr.width - 12.0)).clamp(0.0, 1.0) as f64;
                let nv = (frac * vmax).round();
                if (nv - vol).abs() >= 1.0 {
                    self.player.set_volume(nv);
                    if muted {
                        self.player.set_bool("mute", false);
                    }
                }
            } else {
                self.vol_drag = false;
            }
        }
        if vhot && self.inp.wheel != 0.0 {
            self.exec(Action::VolumeBy(self.inp.wheel as f64 * 5.0));
            self.consumed = true;
        }
        let track = Rectangle::new(vr.x + 6.0, cy - 2.0, vr.width - 12.0, 4.0);
        d.draw_rectangle_rounded(track, 1.0, 4, fade(Color::WHITE, 0.25 * a));
        let frac = if muted { 0.0 } else { (vol / vmax).clamp(0.0, 1.0) as f32 };
        let filled = Rectangle::new(track.x, track.y, track.width * frac, 4.0);
        d.draw_rectangle_rounded(filled, 1.0, 4, fade(if vol > 100.0 { ACCENT } else { FG }, a));
        if vhot || self.vol_drag {
            d.draw_circle_v(v2(track.x + track.width * frac, cy), 6.0, fade(Color::WHITE, a));
            // The 100% mark matters when boosting past it.
            let mark = track.x + track.width * (100.0 / vmax) as f32;
            d.draw_rectangle_rec(Rectangle::new(mark - 0.5, cy - 5.0, 1.0, 10.0), fade(DIM, a));
        }
        x += vr.width + 14.0;

        // Time.
        let st = &self.player.st;
        if has {
            let cur = fmt_time(st.time_pos);
            let time = if st.duration > 0.0 {
                if self.settings.show_remaining {
                    format!("-{} / {}", fmt_time((st.duration - st.time_pos).max(0.0)), fmt_time(st.duration))
                } else {
                    format!("{cur} / {}", fmt_time(st.duration))
                }
            } else if !st.seekable {
                format!("{cur}  ● LIVE")
            } else {
                cur
            };
            let tw = self.fonts.measure(&time, 16.0);
            let tr = Rectangle::new(x, cy - 12.0, tw, 24.0);
            let col = if self.hover(tr, Layer::Bar) { Color::WHITE } else { FG };
            self.fonts.draw(d, &time, v2(x, cy - 9.0), 16.0, fade(col, a));
            if self.click(tr, Layer::Bar) {
                self.exec(Action::ToggleRemaining);
            }
        }

        // Right cluster.
        let mut rx = w - 30.0;
        let fs = self.fullscreen;
        if self.icon_button(d, v2(rx, cy), 26.0, Layer::Bar, false, |d, c, s, col| icons::fullscreen(d, c, s, col, fs))
        {
            self.request_fullscreen = true;
        }
        rx -= 44.0;
        let plist = self.show_playlist;
        if self.icon_button(d, v2(rx, cy), 26.0, Layer::Bar, plist, icons::playlist) {
            self.exec(Action::TogglePlaylist);
        }
        rx -= 44.0;
        let subs_on = self.player.st.tracks.iter().any(|t| t.kind == "sub" && t.selected);
        let subs_open = self.popup.as_ref().is_some_and(|p| p.kind == PopupKind::Subs);
        if self.icon_button(d, v2(rx, cy), 26.0, Layer::Bar, subs_on || subs_open, icons::subtitles) {
            self.exec(Action::Popup(PopupKind::Subs));
        }
        rx -= 40.0;
        let audio_open = self.popup.as_ref().is_some_and(|p| p.kind == PopupKind::Audio);
        if self.icon_button(d, v2(rx, cy), 26.0, Layer::Bar, audio_open, icons::audio_tracks) {
            self.exec(Action::Popup(PopupKind::Audio));
        }
        rx -= 50.0;
        let speed = format!("{:.2}×", self.player.st.speed).replace(".00×", "×");
        let sw = self.fonts.measure(&speed, 16.0);
        let sr = Rectangle::new(rx - sw / 2.0 - 8.0, cy - 14.0, sw + 16.0, 28.0);
        let shot = self.hover(sr, Layer::Bar);
        if shot {
            d.draw_rectangle_rounded(sr, 0.5, 6, fade(Color::WHITE, 0.12 * a));
        }
        let scol = if (self.player.st.speed - 1.0).abs() > 0.001 {
            ACCENT
        } else if shot {
            Color::WHITE
        } else {
            FG
        };
        self.fonts.draw(d, &speed, v2(rx - sw / 2.0, cy - 9.0), 16.0, fade(scol, a));
        if self.click(sr, Layer::Bar) {
            self.exec(Action::Popup(PopupKind::Speed));
        }
        // Repeat: toggles repeat-file, or clears an active A-B loop.
        let (ab, loop_file) = (self.player.st.ab_a.is_some(), self.player.st.loop_file);
        let lc = v2(rx - sw / 2.0 - 30.0, cy);
        if self.icon_button(d, lc, 22.0, Layer::Bar, ab || loop_file, |d, c, s, col| icons::repeat(d, c, s, col, !ab)) {
            self.exec(if ab { Action::ClearAbLoop } else { Action::ToggleLoopFile });
        }
    }

    fn draw_seekbar(&mut self, d: &mut RaylibDrawHandle) {
        let a = self.ui_alpha;
        let (w, h) = (self.inp.w, self.inp.h);
        let st = &self.player.st;
        let has = self.player.has_file();
        let dur = st.duration;
        let y = h - 64.0;
        let x0 = 18.0;
        let tw = w - 36.0;
        let hit = Rectangle::new(x0 - 6.0, y - 12.0, tw + 12.0, 24.0);
        let hot = has && dur > 0.0 && self.hover(hit, Layer::Bar);
        let thick = if hot || self.seek_drag { 6.0 } else { 4.0 };
        let track = Rectangle::new(x0, y - thick / 2.0, tw, thick);
        d.draw_rectangle_rounded(track, 1.0, 4, fade(Color::WHITE, 0.22 * a));
        if !has || dur <= 0.0 {
            return;
        }
        let to_x = |t: f64| x0 + (t / dur).clamp(0.0, 1.0) as f32 * tw;
        let mouse_t = (((self.inp.mouse.x - x0) / tw).clamp(0.0, 1.0) as f64) * dur;

        // Buffered range.
        if st.cache_end > st.time_pos {
            let r = Rectangle::new(to_x(st.time_pos), track.y, to_x(st.cache_end) - to_x(st.time_pos), thick);
            d.draw_rectangle_rounded(r, 1.0, 4, fade(Color::WHITE, 0.25 * a));
        }
        // A-B loop range.
        if let Some(la) = st.ab_a {
            let lb = st.ab_b.unwrap_or(la);
            let r = Rectangle::new(to_x(la), track.y - 2.0, (to_x(lb) - to_x(la)).max(2.0), thick + 4.0);
            d.draw_rectangle_rec(r, fade(ACCENT, 0.35 * a));
        }
        let shown_t = if self.seek_drag { mouse_t } else { st.time_pos };
        let played = Rectangle::new(x0, track.y, to_x(shown_t) - x0, thick);
        d.draw_rectangle_rounded(played, 1.0, 4, fade(ACCENT, a));
        for ch in &st.chapters {
            if ch.time > 0.5 {
                let cx = to_x(ch.time);
                d.draw_rectangle_rec(Rectangle::new(cx - 1.0, track.y, 2.0, thick), fade(Color::BLACK, 0.7 * a));
            }
        }
        if hot || self.seek_drag {
            d.draw_circle_v(v2(to_x(shown_t), y), 7.5, fade(Color::WHITE, a));
        }

        let seekable = st.seekable;
        if self.click(hit, Layer::Bar) && seekable {
            self.seek_drag = true;
            self.last_drag_seek = -1.0;
        }
        if self.seek_drag {
            if self.inp.left_down {
                if (mouse_t - self.last_drag_seek).abs() > dur * 0.002 {
                    self.player.seek_absolute(mouse_t, false);
                    self.last_drag_seek = mouse_t;
                }
            } else {
                self.seek_drag = false;
                self.player.seek_absolute(mouse_t, true);
            }
        }
        if hot && self.inp.wheel != 0.0 {
            self.player.seek_relative(self.inp.wheel as f64 * 5.0, false);
            self.consumed = true;
        }

        if hot || self.seek_drag {
            // Tooltip: preview thumbnail, time and chapter.
            let chapter = self
                .player
                .st
                .chapters
                .iter()
                .rev()
                .find(|c| c.time <= mouse_t)
                .map(|c| c.title.clone())
                .filter(|t| !t.is_empty());
            let label = fmt_time(mouse_t);
            let mx = to_x(mouse_t);
            let mut top = y - 22.0;
            if let Some(t) = &mut self.thumbs {
                t.request(mouse_t);
            }
            let thumb = self.thumbs.as_ref().and_then(|t| t.texture().map(|tex| (tex, t.size())));
            let lw = self.fonts.measure(&label, 16.0);
            let cw = chapter.as_ref().map(|c| self.fonts.measure(c, 14.0).min(300.0)).unwrap_or(0.0);
            let box_w = lw.max(cw) + 20.0;
            if let Some((tex, (tw_, th_))) = thumb {
                let (tw_, th_) = (tw_ as f32, th_ as f32);
                let tx = (mx - tw_ / 2.0).clamp(4.0, w - tw_ - 4.0);
                let ty = top - th_ - 30.0 - if chapter.is_some() { 20.0 } else { 0.0 };
                let frame = Rectangle::new(tx - 2.0, ty - 2.0, tw_ + 4.0, th_ + 4.0);
                d.draw_rectangle_rounded(frame, 0.06, 6, fade(Color::new(12, 12, 14, 255), a));
                let src = Rectangle::new(0.0, 0.0, tex.texture.width as f32, tex.texture.height as f32);
                d.draw_texture_pro(
                    tex.texture(),
                    src,
                    Rectangle::new(tx, ty, tw_, th_),
                    v2(0.0, 0.0),
                    0.0,
                    fade(Color::WHITE, a),
                );
                d.draw_rectangle_rounded_lines_ex(frame, 0.06, 6, 1.5, fade(Color::WHITE, 0.35 * a));
            }
            let bh = if chapter.is_some() { 46.0 } else { 26.0 };
            top -= bh;
            let bx = (mx - box_w / 2.0).clamp(4.0, w - box_w - 4.0);
            d.draw_rectangle_rounded(Rectangle::new(bx, top, box_w, bh), 0.3, 6, fade(Color::new(12, 12, 14, 230), a));
            self.fonts.draw(d, &label, v2(bx + (box_w - lw) / 2.0, top + 4.0), 16.0, fade(FG, a));
            if let Some(c) = chapter {
                let c = self.fonts.fit(&c, 14.0, 300.0).into_owned();
                let cw = self.fonts.measure(&c, 14.0);
                self.fonts.draw(d, &c, v2(bx + (box_w - cw) / 2.0, top + 25.0), 14.0, fade(DIM, a));
            }
        }
    }

    fn scroll_playlist_to_current(&mut self) {
        let pos = self.player.st.playlist_pos.max(0) as f32;
        let view = self.panel_rect().height - 64.0;
        let want = pos * PL_ROW - view / 2.0 + PL_ROW / 2.0;
        self.pl_scroll = want.max(0.0);
    }

    fn hovered_playlist_row(&self) -> Option<usize> {
        let r = self.panel_rect();
        let list_top = r.y + 60.0;
        let m = self.inp.mouse;
        if !inside(r, m) || m.y < list_top {
            return None;
        }
        let idx = ((m.y - list_top + self.pl_scroll) / PL_ROW) as usize;
        (idx < self.player.st.playlist.len()).then_some(idx)
    }

    fn draw_playlist(&mut self, d: &mut RaylibDrawHandle) {
        let r = self.panel_rect();
        d.draw_rectangle_rec(r, PANEL);
        d.draw_rectangle_rec(Rectangle::new(r.x, r.y, 1.0, r.height), fade(Color::WHITE, 0.08));
        let n = self.player.st.playlist.len();
        self.fonts.draw(d, "Playlist", v2(r.x + 18.0, r.y + 16.0), 22.0, FG);
        let count = format!("{n} item{}", if n == 1 { "" } else { "s" });
        let cw = self.fonts.measure("Playlist", 22.0);
        self.fonts.draw(d, &count, v2(r.x + 28.0 + cw, r.y + 22.0), 14.0, DIM);

        // Header buttons.
        let by = r.y + 28.0;
        let mut bx = r.x + r.width - 26.0;
        if self.icon_button(d, v2(bx, by), 20.0, Layer::Panel, false, icons::close) {
            self.show_playlist = false;
        }
        bx -= 36.0;
        if self.icon_button(d, v2(bx, by), 20.0, Layer::Panel, false, icons::trash) {
            self.exec(Action::ClearPlaylist);
        }
        bx -= 36.0;
        let lp = self.player.st.loop_playlist;
        if self.icon_button(d, v2(bx, by), 22.0, Layer::Panel, lp, |d, c, s, col| icons::repeat(d, c, s, col, false)) {
            self.exec(Action::ToggleLoopPlaylist);
        }
        bx -= 36.0;
        if self.icon_button(d, v2(bx, by), 22.0, Layer::Panel, false, icons::shuffle) {
            self.exec(Action::Shuffle);
        }
        bx -= 36.0;
        if self.icon_button(d, v2(bx, by), 22.0, Layer::Panel, false, icons::folder) {
            self.exec(Action::Browse(BrowseMode::Media));
        }

        let list_top = r.y + 60.0;
        let view_h = r.height - 60.0;
        let content_h = n as f32 * PL_ROW;
        let max_scroll = (content_h - view_h).max(0.0);
        if self.layer == Layer::Panel && self.inp.wheel != 0.0 {
            self.pl_scroll -= self.inp.wheel * PL_ROW * 2.0;
            self.consumed = true;
        }
        self.pl_scroll = self.pl_scroll.clamp(0.0, max_scroll);

        if n == 0 {
            let t = "Empty — drop files here";
            let tw = self.fonts.measure(t, 16.0);
            self.fonts.draw(d, t, v2(r.x + (r.width - tw) / 2.0, list_top + 40.0), 16.0, DIM);
        }

        let hovered = if self.layer == Layer::Panel { self.hovered_playlist_row() } else { None };
        let mut action = None;
        unsafe { raylib::ffi::BeginScissorMode(r.x as i32, list_top as i32, r.width as i32, view_h as i32) };
        let items = self.player.st.playlist.clone();
        let first = (self.pl_scroll / PL_ROW) as usize;
        let last = ((self.pl_scroll + view_h) / PL_ROW).ceil() as usize + 1;
        for (i, item) in items.iter().enumerate().take(last.min(n)).skip(first) {
            let y = list_top + i as f32 * PL_ROW - self.pl_scroll;
            let row = Rectangle::new(r.x, y, r.width, PL_ROW);
            let hot = hovered == Some(i) && !self.pl_dragging;
            if item.current {
                d.draw_rectangle_rec(row, fade(ACCENT, 0.14));
                d.draw_rectangle_rec(Rectangle::new(r.x, y, 3.0, PL_ROW), ACCENT);
            } else if hot {
                d.draw_rectangle_rec(row, ROW_HOVER);
            }
            let num = format!("{}", i + 1);
            let nw = self.fonts.measure(&num, 14.0);
            if item.current && !self.player.st.paused {
                icons::play(d, v2(r.x + 26.0, y + PL_ROW / 2.0), 12.0, ACCENT);
            } else {
                self.fonts.draw(d, &num, v2(r.x + 30.0 - nw / 2.0, y + 12.0), 14.0, DIM);
            }
            let name = item.name();
            let maxw = r.width - 60.0 - if hot { 30.0 } else { 0.0 };
            let fitted = self.fonts.fit(&name, 16.0, maxw).into_owned();
            self.fonts.draw(d, &fitted, v2(r.x + 50.0, y + 10.0), 16.0, if item.current { ACCENT } else { FG });
            if hot {
                let xr = Rectangle::new(r.x + r.width - 36.0, y + 6.0, 28.0, 28.0);
                let xhot = inside(xr, self.inp.mouse);
                icons::close(d, v2(xr.x + 14.0, xr.y + 14.0), 16.0, if xhot { ERROR } else { DIM });
                if xhot && self.inp.left_pressed && !self.consumed {
                    self.consumed = true;
                    action = Some(Action::RemoveIndex(i));
                }
            }
        }
        // Drag-to-reorder insertion marker.
        if self.pl_dragging {
            let ins = (((self.inp.mouse.y - list_top + self.pl_scroll) / PL_ROW).round() as usize).min(n);
            let y = list_top + ins as f32 * PL_ROW - self.pl_scroll;
            d.draw_rectangle_rec(Rectangle::new(r.x + 8.0, y - 1.5, r.width - 16.0, 3.0), ACCENT);
        }
        unsafe { raylib::ffi::EndScissorMode() };

        // Press / drag / release handling for rows.
        if self.inp.left_pressed
            && !self.consumed
            && let Some(i) = hovered
        {
            self.consumed = true;
            self.pl_press = Some((i, self.inp.mouse.y));
            self.pl_dragging = false;
        }
        if let Some((from, y0)) = self.pl_press {
            if self.inp.left_down {
                if (self.inp.mouse.y - y0).abs() > 6.0 {
                    self.pl_dragging = true;
                }
            } else {
                if self.pl_dragging {
                    let ins = (((self.inp.mouse.y - list_top + self.pl_scroll) / PL_ROW).round() as usize).min(n);
                    if ins != from && ins != from + 1 {
                        action = Some(Action::MoveIndex(from, ins));
                    }
                } else if hovered == Some(from) {
                    action = Some(Action::PlayIndex(from));
                }
                self.pl_press = None;
                self.pl_dragging = false;
            }
        }
        if let Some(a) = action {
            self.exec(a);
        }
    }

    fn draw_stats(&mut self, d: &mut RaylibDrawHandle) {
        let st = &self.player.st;
        let mut lines: Vec<(String, String)> = vec![("File".into(), util::display_name(&st.path))];
        let mut fmt = st.file_format.clone();
        if st.file_size > 0 {
            fmt += &format!("  {}", util::fmt_size(st.file_size as u64));
        }
        lines.push(("Format".into(), fmt));
        if st.video_track().is_some() {
            lines.push(("Video".into(), format!("{}  {}×{}  {:.3} fps", st.video_codec, st.width, st.height, st.fps)));
            let hw = if st.hwdec.is_empty() || st.hwdec == "no" { "software".to_string() } else { st.hwdec.clone() };
            lines.push(("Decoder".into(), format!("{hw}   dropped frames: {}", st.dropped)));
            if st.video_bitrate > 0.0 {
                lines.push(("Video rate".into(), format!("{:.0} kbps", st.video_bitrate / 1000.0)));
            }
        }
        if !st.audio_codec.is_empty() {
            lines.push((
                "Audio".into(),
                format!(
                    "{}  {} Hz  {}  {:.0} kbps",
                    st.audio_codec,
                    st.samplerate,
                    st.channels,
                    st.audio_bitrate / 1000.0
                ),
            ));
        }
        lines.push(("Cache".into(), format!("{:.1} s ahead", (st.cache_end - st.time_pos).max(0.0))));
        lines.push((
            "Speed".into(),
            format!("{:.2}×   volume {:.0}%{}", st.speed, st.volume, if st.muted { " (muted)" } else { "" }),
        ));
        if st.sub_delay.abs() > 0.0001 {
            lines.push(("Sub delay".into(), format!("{:+.0} ms", st.sub_delay * 1000.0)));
        }
        let size = 15.0;
        let kw = lines.iter().map(|(k, _)| self.fonts.measure(k, size)).fold(0.0, f32::max);
        let vw = lines.iter().map(|(_, v)| self.fonts.measure(v, size)).fold(0.0, f32::max).min(self.inp.w * 0.6);
        let r = Rectangle::new(16.0, 64.0, kw + vw + 44.0, lines.len() as f32 * 22.0 + 20.0);
        d.draw_rectangle_rounded(r, 0.06, 6, Color::new(10, 10, 12, 200));
        for (i, (k, v)) in lines.iter().enumerate() {
            let y = r.y + 10.0 + i as f32 * 22.0;
            self.fonts.draw(d, k, v2(r.x + 14.0, y), size, ACCENT);
            let v = self.fonts.fit(v, size, vw).into_owned();
            self.fonts.draw(d, &v, v2(r.x + 30.0 + kw, y), size, FG);
        }
    }

    fn draw_osd(&mut self, d: &mut RaylibDrawHandle) {
        let now = self.inp.now;
        let w = self.inp.w;
        if let Some((msg, until)) = &self.osd {
            if now > *until {
                self.osd = None;
            } else {
                let a = ((until - now) / 0.3).min(1.0) as f32;
                let size = 22.0;
                let tw = self.fonts.measure(msg, size);
                let r = Rectangle::new((w - tw) / 2.0 - 18.0, 70.0, tw + 36.0, 42.0);
                d.draw_rectangle_rounded(r, 0.5, 8, fade(Color::new(10, 10, 12, 210), a));
                self.fonts.draw(d, msg, v2(r.x + 18.0, r.y + 9.0), size, fade(FG, a));
            }
        }
        if let Some((msg, until)) = &self.toast {
            if now > *until {
                self.toast = None;
            } else {
                let a = ((until - now) / 0.4).min(1.0) as f32;
                let msg = self.fonts.fit(msg, 17.0, w - 100.0).into_owned();
                let tw = self.fonts.measure(&msg, 17.0);
                let r = Rectangle::new((w - tw) / 2.0 - 20.0, self.inp.h - BAR_H - 62.0, tw + 40.0, 40.0);
                d.draw_rectangle_rounded(r, 0.4, 8, fade(Color::new(60, 18, 20, 235), a));
                d.draw_rectangle_rounded_lines_ex(r, 0.4, 8, 1.5, fade(ERROR, a));
                self.fonts.draw(d, &msg, v2(r.x + 20.0, r.y + 10.0), 17.0, fade(FG, a));
            }
        }
    }

    // -------------------------------------------------------------- popups

    fn popup_items(&self, kind: PopupKind) -> (Option<&'static str>, Vec<MenuItem>) {
        let st = &self.player.st;
        let has = self.player.has_file();
        match kind {
            PopupKind::Subs => {
                let subs: Vec<&Track> = st.tracks.iter().filter(|t| t.kind == "sub").collect();
                let any = subs.iter().any(|t| t.selected);
                let mut items = vec![MenuItem::new("Off", Action::SelectTrack("sub", None)).checked(!any)];
                items.extend(
                    subs.iter()
                        .map(|t| MenuItem::new(t.label(), Action::SelectTrack("sub", Some(t.id))).checked(t.selected)),
                );
                items.push(MenuItem::sep());
                items.push(MenuItem::new("Load subtitle file…", Action::Browse(BrowseMode::Subtitle)));
                if any {
                    items.push(
                        MenuItem::new(
                            format!("Delay  {:+.0} ms", st.sub_delay * 1000.0),
                            Action::SubDelay(-st.sub_delay),
                        )
                        .hint("Z / X"),
                    );
                }
                (Some("Subtitles"), items)
            }
            PopupKind::Audio => {
                let auds: Vec<MenuItem> = st
                    .tracks
                    .iter()
                    .filter(|t| t.kind == "audio")
                    .map(|t| MenuItem::new(t.label(), Action::SelectTrack("audio", Some(t.id))).checked(t.selected))
                    .collect();
                if auds.is_empty() {
                    (Some("Audio"), vec![MenuItem::info("No audio tracks")])
                } else {
                    (Some("Audio"), auds)
                }
            }
            PopupKind::Speed => (
                Some("Playback speed"),
                SPEEDS
                    .iter()
                    .map(|&s| {
                        let label = if s == 1.0 { "Normal".to_string() } else { format!("{s}×") };
                        MenuItem::new(label, Action::SetSpeed(s)).checked((st.speed - s).abs() < 0.001)
                    })
                    .collect(),
            ),
            PopupKind::Context => {
                let mut items = Vec::new();
                if has {
                    items.push(
                        MenuItem::new(if st.paused { "Play" } else { "Pause" }, Action::TogglePause).hint("Space"),
                    );
                }
                items.push(MenuItem::new("Open file…", Action::Browse(BrowseMode::Media)).hint("Ctrl+O"));
                items.push(MenuItem::new("Open URL…", Action::OpenUrlDialog).hint("Ctrl+U"));
                items.push(MenuItem::sep());
                items.push(MenuItem::new("Playlist", Action::TogglePlaylist).hint("Tab").checked(self.show_playlist));
                if has {
                    items.push(MenuItem::new("Subtitles  ›", Action::Popup(PopupKind::Subs)));
                    items.push(MenuItem::new("Audio track  ›", Action::Popup(PopupKind::Audio)));
                    items.push(MenuItem::new("Speed  ›", Action::Popup(PopupKind::Speed)));
                    items.push(MenuItem::sep());
                    items.push(MenuItem::new("Repeat file", Action::ToggleLoopFile).hint("R").checked(st.loop_file));
                    items.push(MenuItem::new("A-B loop", Action::AbLoop).hint("Shift+R").checked(st.ab_a.is_some()));
                    items.push(MenuItem::new("Screenshot", Action::Screenshot).hint("S"));
                    items.push(MenuItem::new("Statistics", Action::ToggleStats).hint("I").checked(self.show_stats));
                    items.push(MenuItem::new("Stop", Action::Stop));
                }
                items.push(MenuItem::new("Fullscreen", Action::ToggleFullscreen).hint("F").checked(self.fullscreen));
                items.push(
                    MenuItem::new("Seek previews", Action::ToggleThumbs)
                        .hint("Ctrl+T")
                        .checked(self.settings.thumbnails),
                );
                items.push(MenuItem::new("Keyboard shortcuts", Action::ToggleHelp).hint("F1"));
                items.push(MenuItem::sep());
                items.push(MenuItem::new("Quit", Action::Quit).hint("Q"));
                (None, items)
            }
        }
    }

    fn draw_popup(&mut self, d: &mut RaylibDrawHandle) {
        let Some(popup) = &self.popup else { return };
        let (kind, anchor, upward) = (popup.kind, popup.anchor, popup.upward);
        let (title, items) = self.popup_items(kind);
        let size = 16.0;
        let label_w = items.iter().map(|i| self.fonts.measure(&i.label, size)).fold(0.0, f32::max);
        let hint_w = items.iter().map(|i| self.fonts.measure(&i.hint, 13.0)).fold(0.0, f32::max);
        let w = (label_w + hint_w + 80.0).clamp(200.0, (self.inp.w - 20.0).min(520.0));
        let head = if title.is_some() { 36.0 } else { 6.0 };
        let body: f32 = items.iter().map(|i| if i.label.is_empty() { 9.0 } else { MENU_ROW }).sum();
        let h = (head + body + 6.0).min(self.inp.h - 20.0);
        let x = (anchor.x - if upward { w / 2.0 } else { 0.0 }).clamp(10.0, self.inp.w - w - 10.0);
        let y = if upward { anchor.y - h - 8.0 } else { anchor.y }.clamp(10.0, (self.inp.h - h - 10.0).max(10.0));
        let r = Rectangle::new(x, y, w, h);
        // A popup opened by a click earlier in this frame must not see that same click as "outside".
        let live = self.layer == Layer::Popup;

        d.draw_rectangle_rounded(
            Rectangle::new(r.x + 3.0, r.y + 5.0, r.width, r.height),
            0.06,
            8,
            fade(Color::BLACK, 0.35),
        );
        d.draw_rectangle_rounded(r, 0.06, 8, Color::new(30, 30, 36, 250));
        d.draw_rectangle_rounded_lines_ex(r, 0.06, 8, 1.0, fade(Color::WHITE, 0.08));
        if let Some(t) = title {
            self.fonts.draw(d, t, v2(x + 16.0, y + 10.0), 15.0, DIM);
        }
        let mut iy = y + head;
        let mut chosen = None;
        for item in &items {
            if item.label.is_empty() {
                d.draw_rectangle_rec(Rectangle::new(x + 10.0, iy + 4.0, w - 20.0, 1.0), fade(Color::WHITE, 0.1));
                iy += 9.0;
                continue;
            }
            if iy + MENU_ROW > y + h {
                break;
            }
            let row = Rectangle::new(x + 4.0, iy, w - 8.0, MENU_ROW);
            let enabled = item.action.is_some();
            let hot = live && enabled && inside(row, self.inp.mouse);
            if hot {
                d.draw_rectangle_rounded(row, 0.25, 6, fade(ACCENT, 0.2));
            }
            if item.checked {
                d.draw_circle_v(v2(x + 20.0, iy + MENU_ROW / 2.0), 4.0, ACCENT);
            }
            let label = self.fonts.fit(&item.label, size, w - 60.0 - hint_w).into_owned();
            let col = if !enabled {
                DIM
            } else if item.checked {
                ACCENT
            } else {
                FG
            };
            self.fonts.draw(d, &label, v2(x + 34.0, iy + 7.0), size, col);
            if !item.hint.is_empty() {
                let hw = self.fonts.measure(&item.hint, 13.0);
                self.fonts.draw(d, &item.hint, v2(x + w - hw - 14.0, iy + 9.0), 13.0, DIM);
            }
            if hot && self.inp.left_pressed {
                chosen = item.action.clone();
            }
            iy += MENU_ROW;
        }
        let m = self.inp.mouse;
        if live && (self.inp.left_pressed || self.inp.right_pressed) {
            self.consumed = true;
            if let Some(a) = chosen {
                // Submenu entries swap the popup in place.
                if let Action::Popup(k) = a {
                    self.popup = Some(Popup { kind: k, anchor: v2(x, y), upward: false });
                } else {
                    self.popup = None;
                    self.exec(a);
                }
            } else if !inside(r, m) {
                self.popup = None;
                // Right-click elsewhere re-opens the context menu there.
                if self.inp.right_pressed && self.layer_at_without_popup(m) == Layer::Video {
                    self.popup = Some(Popup { kind: PopupKind::Context, anchor: m, upward: false });
                }
            }
        }
    }

    fn layer_at_without_popup(&self, m: Vector2) -> Layer {
        if self.show_playlist && inside(self.panel_rect(), m) {
            Layer::Panel
        } else if m.y > self.inp.h - BAR_H {
            Layer::Bar
        } else {
            Layer::Video
        }
    }

    // --------------------------------------------------------------- modals

    fn draw_modal(&mut self, d: &mut RaylibDrawHandle) {
        let (w, h) = (self.inp.w as i32, self.inp.h as i32);
        d.draw_rectangle(0, 0, w, h, fade(Color::BLACK, 0.55));
        let r = self.modal_rect();
        d.draw_rectangle_rounded(r, 0.025, 8, Color::new(26, 26, 31, 252));
        d.draw_rectangle_rounded_lines_ex(r, 0.025, 8, 1.0, fade(Color::WHITE, 0.1));
        if self.inp.left_pressed && !inside(r, self.inp.mouse) {
            self.consumed = true;
            if let Some(Modal::Browser(b)) = &self.modal {
                self.settings.browser_dir = Some(b.cwd.to_string_lossy().into_owned());
            }
            self.modal = None;
            return;
        }
        match &self.modal {
            Some(Modal::Help) => self.draw_help(d, r),
            Some(Modal::Url(_)) => self.draw_url(d, r),
            Some(Modal::Browser(_)) => self.draw_browser(d, r),
            None => {}
        }
    }

    fn draw_help(&mut self, d: &mut RaylibDrawHandle, r: Rectangle) {
        const KEYS: &[(&str, &str)] = &[
            ("Space / K", "Play / pause"),
            ("Left / Right", "Seek 5 s (Shift: 1 s)"),
            ("J / L", "Seek 10 s"),
            ("Shift + Up / Down", "Seek 1 min"),
            ("0 – 9", "Jump to 0% – 90%"),
            ("PgUp / PgDn", "Previous / next chapter"),
            (", / .", "Frame back / step"),
            ("Up / Down · wheel", "Volume"),
            ("M · middle click", "Mute"),
            ("[ / ]  ·  Backspace", "Speed −/+ · reset"),
            ("N / P · Ctrl+Right/Left", "Next / previous file"),
            ("F · double click", "Fullscreen"),
            ("Tab", "Playlist"),
            ("C / A", "Cycle subtitles / audio"),
            ("V", "Show / hide subtitles"),
            ("Z / X", "Subtitle delay −/+ 100 ms"),
            ("R / Shift+R", "Repeat file / A-B loop"),
            ("S", "Screenshot"),
            ("I", "Statistics"),
            ("Ctrl+O", "Open file browser"),
            ("Ctrl+U", "Open URL or path"),
            ("Ctrl+V", "Play paths/URLs from clipboard"),
            ("Ctrl+S", "Shuffle playlist"),
            ("Ctrl+T", "Toggle seek previews"),
            ("Right click", "Menu"),
            ("Q / Ctrl+Q", "Quit (remembers position)"),
        ];
        self.fonts.draw(d, "Keyboard shortcuts", v2(r.x + 24.0, r.y + 18.0), 24.0, FG);
        if self.icon_button(d, v2(r.x + r.width - 28.0, r.y + 30.0), 20.0, Layer::Modal, false, icons::close) {
            self.modal = None;
            return;
        }
        let cols = if r.width > 700.0 { 2 } else { 1 };
        let per = KEYS.len().div_ceil(cols);
        let colw = (r.width - 48.0) / cols as f32;
        let row_h = ((r.height - 80.0) / per as f32).min(26.0);
        let size = (row_h * 0.62).clamp(11.0, 15.0);
        for (i, (k, desc)) in KEYS.iter().enumerate() {
            let (c, row) = (i / per, i % per);
            let x = r.x + 24.0 + c as f32 * colw;
            let y = r.y + 66.0 + row as f32 * row_h;
            self.fonts.draw(d, k, v2(x, y), size, ACCENT);
            self.fonts.draw(d, desc, v2(x + colw * 0.45, y), size, FG);
        }
    }

    fn draw_url(&mut self, d: &mut RaylibDrawHandle, r: Rectangle) {
        let Some(Modal::Url(text)) = &self.modal else { return };
        let text = text.clone();
        self.fonts.draw(d, "Open URL or path", v2(r.x + 24.0, r.y + 18.0), 22.0, FG);
        let field = Rectangle::new(r.x + 24.0, r.y + 60.0, r.width - 48.0, 44.0);
        d.draw_rectangle_rounded(field, 0.2, 6, Color::new(14, 14, 17, 255));
        d.draw_rectangle_rounded_lines_ex(field, 0.2, 6, 1.5, fade(ACCENT, 0.8));
        let size = 18.0;
        // Show the tail of long input.
        let mut shown: String = text.clone();
        while self.fonts.measure(&shown, size) > field.width - 30.0 && !shown.is_empty() {
            shown.remove(0);
        }
        if text.is_empty() {
            self.fonts.draw(
                d,
                "https://…  or  /path/to/file.mkv",
                v2(field.x + 14.0, field.y + 11.0),
                size,
                fade(DIM, 0.6),
            );
        } else {
            self.fonts.draw(d, &shown, v2(field.x + 14.0, field.y + 11.0), size, FG);
        }
        if (self.inp.now * 2.0) as i64 % 2 == 0 {
            let cx = field.x + 15.0 + if text.is_empty() { 0.0 } else { self.fonts.measure(&shown, size) };
            d.draw_rectangle_rec(Rectangle::new(cx, field.y + 11.0, 2.0, 22.0), FG);
        }
        let hint = "Enter: play  ·  Shift+Enter: add to playlist  ·  Ctrl+V: paste  ·  Esc: cancel";
        let hint = self.fonts.fit(hint, 14.0, r.width - 48.0).into_owned();
        self.fonts.draw(d, &hint, v2(r.x + 24.0, r.y + 122.0), 14.0, DIM);
    }

    fn browser_scroll_to_selection(&mut self) {
        let view = self.modal_rect().height - 190.0;
        if let Some(Modal::Browser(b)) = &mut self.modal {
            let y = b.selected as f32 * 34.0;
            if y < b.scroll {
                b.scroll = y;
            } else if y + 34.0 > b.scroll + view {
                b.scroll = y + 34.0 - view;
            }
        }
    }

    fn browser_activate(&mut self, append: bool) {
        let Some(Modal::Browser(b)) = &mut self.modal else { return };
        let Some(e) = b.current() else { return };
        if e.is_dir {
            b.navigate(&e.path);
            return;
        }
        let path = e.path.to_string_lossy().into_owned();
        let mode = b.mode;
        self.settings.browser_dir = Some(b.cwd.to_string_lossy().into_owned());
        self.modal = None;
        match mode {
            BrowseMode::Subtitle => match self.player.command(&["sub-add", &path, "select"]) {
                Ok(()) => self.osd(format!("Subtitle: {}", util::display_name(&path))),
                Err(err) => self.toast(format!("Could not load subtitle: {err}")),
            },
            BrowseMode::Media => {
                let append = append && self.player.has_file();
                self.exec(Action::Open(vec![path], append));
                if append {
                    self.osd("Added to playlist");
                }
            }
        }
    }

    fn browser_play_folder(&mut self) {
        let Some(Modal::Browser(b)) = &self.modal else { return };
        if b.mode != BrowseMode::Media {
            return;
        }
        let files = b.files_here();
        self.settings.browser_dir = Some(b.cwd.to_string_lossy().into_owned());
        if files.is_empty() {
            self.toast("No playable files in this folder");
            return;
        }
        self.modal = None;
        self.exec(Action::Open(files, false));
    }

    fn draw_browser(&mut self, d: &mut RaylibDrawHandle, r: Rectangle) {
        let Some(Modal::Browser(b)) = &self.modal else { return };
        let title = match b.mode {
            BrowseMode::Media => "Open media",
            BrowseMode::Subtitle => "Load subtitle",
        };
        let mode = b.mode;
        self.fonts.draw(d, title, v2(r.x + 24.0, r.y + 16.0), 22.0, FG);
        if self.icon_button(d, v2(r.x + r.width - 28.0, r.y + 28.0), 20.0, Layer::Modal, false, icons::close) {
            self.modal = None;
            return;
        }

        // Places.
        let places = [
            ("Home", util::home_dir()),
            ("Videos", util::user_dir("VIDEOS", "Videos")),
            ("Music", util::user_dir("MUSIC", "Music")),
            ("Downloads", util::user_dir("DOWNLOAD", "Downloads")),
            ("Root", PathBuf::from("/")),
        ];
        let mut px = r.x + 24.0;
        let py = r.y + 52.0;
        let mut nav: Option<PathBuf> = None;
        let up_r = Rectangle::new(px, py, 34.0, 28.0);
        let up_hot = self.hover(up_r, Layer::Modal);
        d.draw_rectangle_rounded(up_r, 0.3, 6, fade(Color::WHITE, if up_hot { 0.16 } else { 0.07 }));
        icons::up(d, v2(px + 17.0, py + 14.0), 16.0, FG);
        let mut go_up = self.click(up_r, Layer::Modal);
        px += 44.0;
        for (name, path) in &places {
            if !path.is_dir() {
                continue;
            }
            let tw = self.fonts.measure(name, 14.0);
            let pr = Rectangle::new(px, py, tw + 20.0, 28.0);
            let hot = self.hover(pr, Layer::Modal);
            d.draw_rectangle_rounded(pr, 0.3, 6, fade(Color::WHITE, if hot { 0.16 } else { 0.07 }));
            self.fonts.draw(d, name, v2(px + 10.0, py + 6.0), 14.0, FG);
            if self.click(pr, Layer::Modal) {
                nav = Some(path.clone());
            }
            px += tw + 28.0;
        }

        let Some(Modal::Browser(b)) = &mut self.modal else { return };
        if go_up {
            b.up();
            go_up = false;
        }
        if let Some(p) = nav {
            b.navigate(&p);
        }
        let _ = go_up;

        // Path + filter line.
        let cwd = b.cwd.to_string_lossy().into_owned();
        let filter = b.filter.clone();
        let error = b.error.clone();
        let cwd_line = self.fonts.fit(&cwd, 15.0, r.width - 48.0).into_owned();
        self.fonts.draw(d, &cwd_line, v2(r.x + 24.0, r.y + 92.0), 15.0, DIM);
        let fr = Rectangle::new(r.x + 24.0, r.y + 116.0, r.width - 48.0, 32.0);
        d.draw_rectangle_rounded(fr, 0.25, 6, Color::new(14, 14, 17, 255));
        if filter.is_empty() {
            self.fonts.draw(d, "Type to filter…", v2(fr.x + 12.0, fr.y + 7.0), 15.0, fade(DIM, 0.6));
        } else {
            self.fonts.draw(d, &filter, v2(fr.x + 12.0, fr.y + 7.0), 15.0, FG);
            d.draw_rectangle_rounded_lines_ex(fr, 0.25, 6, 1.5, fade(ACCENT, 0.7));
        }

        // Listing.
        let list = Rectangle::new(r.x + 12.0, r.y + 158.0, r.width - 24.0, r.height - 158.0 - 56.0);
        let row_h = 34.0;
        let Some(Modal::Browser(b)) = &mut self.modal else { return };
        let entries: Vec<_> = b.visible().into_iter().cloned().collect();
        let content = entries.len() as f32 * row_h;
        if self.layer == Layer::Modal && inside(list, self.inp.mouse) && self.inp.wheel != 0.0 {
            b.scroll -= self.inp.wheel * row_h * 3.0;
        }
        b.scroll = b.scroll.clamp(0.0, (content - list.height).max(0.0));
        let scroll = b.scroll;
        let selected = b.selected;
        if let Some(e) = &error {
            self.fonts.draw(d, e, v2(list.x + 12.0, list.y + 10.0), 15.0, ERROR);
        } else if entries.is_empty() {
            let msg = match mode {
                BrowseMode::Media => "No folders or media files here",
                BrowseMode::Subtitle => "No folders or subtitle files here",
            };
            self.fonts.draw(d, msg, v2(list.x + 12.0, list.y + 10.0), 15.0, DIM);
        }
        let mut clicked_idx = None;
        let mut double = false;
        unsafe { raylib::ffi::BeginScissorMode(list.x as i32, list.y as i32, list.width as i32, list.height as i32) };
        for (i, e) in entries.iter().enumerate() {
            let y = list.y + i as f32 * row_h - scroll;
            if y + row_h < list.y || y > list.y + list.height {
                continue;
            }
            let row = Rectangle::new(list.x, y, list.width, row_h);
            let hot = self.layer == Layer::Modal && inside(row, self.inp.mouse) && inside(list, self.inp.mouse);
            if i == selected {
                d.draw_rectangle_rounded(row, 0.2, 6, fade(ACCENT, 0.2));
            } else if hot {
                d.draw_rectangle_rounded(row, 0.2, 6, ROW_HOVER);
            }
            let ic = v2(list.x + 22.0, y + row_h / 2.0);
            if e.is_dir {
                icons::folder(d, ic, 18.0, Color::new(120, 170, 230, 255));
            } else {
                icons::file(d, ic, 18.0, ACCENT);
            }
            let size_s = if e.is_dir { String::new() } else { util::fmt_size(e.size) };
            let sw = self.fonts.measure(&size_s, 14.0);
            let name = self.fonts.fit(&e.name, 16.0, list.width - 80.0 - sw).into_owned();
            self.fonts.draw(d, &name, v2(list.x + 44.0, y + 7.0), 16.0, FG);
            self.fonts.draw(d, &size_s, v2(list.x + list.width - sw - 14.0, y + 9.0), 14.0, DIM);
            if hot && self.inp.left_pressed && !self.consumed {
                self.consumed = true;
                clicked_idx = Some(i);
                let (t, _) = self.last_click;
                double = i == selected && self.inp.now - t < 0.4;
                self.last_click = (self.inp.now, self.inp.mouse);
            }
        }
        unsafe { raylib::ffi::EndScissorMode() };

        // Footer buttons.
        let fy = r.y + r.height - 46.0;
        let mut buttons: Vec<(&str, u8)> = vec![("Open", 0)];
        if mode == BrowseMode::Media {
            buttons.push(("Add to playlist", 1));
            buttons.push(("Play folder", 2));
        }
        let mut bx = r.x + r.width - 24.0;
        let mut pressed = None;
        for (label, id) in buttons.iter().rev() {
            let tw = self.fonts.measure(label, 15.0);
            let br = Rectangle::new(bx - tw - 28.0, fy, tw + 28.0, 32.0);
            let hot = self.hover(br, Layer::Modal);
            let primary = *id == 0;
            let bg = if primary {
                fade(ACCENT, if hot { 1.0 } else { 0.85 })
            } else {
                fade(Color::WHITE, if hot { 0.16 } else { 0.08 })
            };
            d.draw_rectangle_rounded(br, 0.3, 6, bg);
            self.fonts.draw(
                d,
                label,
                v2(br.x + 14.0, br.y + 7.0),
                15.0,
                if primary { Color::new(30, 22, 10, 255) } else { FG },
            );
            if self.click(br, Layer::Modal) {
                pressed = Some(*id);
            }
            bx = br.x - 10.0;
        }
        // Toggles on the left.
        let Some(Modal::Browser(b)) = &mut self.modal else { return };
        let (hidden, all) = (b.show_hidden, b.show_all);
        let mut tx = r.x + 24.0;
        let mut toggled = None;
        for (label, on, id) in [("Hidden files", hidden, 0u8), ("All files", all, 1u8)] {
            let tw = self.fonts.measure(label, 14.0);
            let tr = Rectangle::new(tx, fy + 4.0, tw + 30.0, 24.0);
            let box_r = Rectangle::new(tx, fy + 9.0, 14.0, 14.0);
            if on {
                d.draw_rectangle_rounded(box_r, 0.3, 4, ACCENT);
            } else {
                d.draw_rectangle_rounded_lines_ex(box_r, 0.3, 4, 1.5, DIM);
            }
            self.fonts.draw(d, label, v2(tx + 22.0, fy + 7.0), 14.0, FG);
            if self.click(tr, Layer::Modal) {
                toggled = Some(id);
            }
            tx += tw + 44.0;
        }
        if let Some(Modal::Browser(b)) = &mut self.modal {
            match toggled {
                Some(0) => {
                    b.show_hidden = !b.show_hidden;
                    b.refresh();
                }
                Some(1) => {
                    b.show_all = !b.show_all;
                    b.refresh();
                }
                _ => {}
            }
            if let Some(i) = clicked_idx {
                b.selected = i;
            }
        }
        if double {
            self.browser_activate(self.inp.shift);
        } else {
            match pressed {
                Some(0) => self.browser_activate(false),
                Some(1) => self.browser_activate(true),
                Some(2) => self.browser_play_folder(),
                _ => {}
            }
        }
    }
}
