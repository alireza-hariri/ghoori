//! Seek-bar preview thumbnails, produced by a second, muted mpv instance that
//! renders keyframes into a small offscreen texture.

use crate::render::GlRenderer;
use libmpv2::Mpv;
use libmpv2_sys as sys;
use raylib::prelude::*;
use std::ffi::CString;
use std::time::{Duration, Instant};

const THUMB_W: i32 = 240;

/// Queue a command without waiting for the core: a blocking call can stall the UI thread while
/// this instance's video output waits for the UI thread to render.
fn command_async(mpv: &Mpv, args: &[&str]) -> bool {
    let Ok(args) = args.iter().map(|a| CString::new(*a)).collect::<Result<Vec<_>, _>>() else { return false };
    let mut ptrs: Vec<*const std::ffi::c_char> = args.iter().map(|a| a.as_ptr()).collect();
    ptrs.push(std::ptr::null());
    unsafe { sys::mpv_command_async(mpv.ctx.as_ptr(), 0, ptrs.as_mut_ptr()) >= 0 }
}

pub struct Thumbnailer {
    // Field order matters: the render context must be freed before its mpv core.
    gl: GlRenderer,
    mpv: Mpv,
    target: Option<RenderTexture2D>,
    aspect: f64,
    source: Option<String>,
    ready: bool,
    failed: bool,
    seek_started: Option<Instant>,
    want: Option<f64>,
    requested: Option<f64>,
    shown: Option<f64>,
}

impl Thumbnailer {
    pub fn new() -> Result<Self, String> {
        let opts: &[(&str, &str)] = &[
            ("vo", "libmpv"),
            ("config", "no"),
            ("terminal", "no"),
            ("idle", "yes"),
            ("keep-open", "always"),
            ("pause", "yes"),
            ("aid", "no"),
            ("sid", "no"),
            ("audio", "no"),
            ("hr-seek", "no"),
            ("hwdec", "no"),
            // Without this the render context still probes every interop (CUDA can stall for seconds).
            ("gpu-hwdec-interop", "no"),
            ("cache", "no"),
            ("ytdl", "no"),
            ("sub-auto", "no"),
            ("audio-file-auto", "no"),
            ("resume-playback", "no"),
            ("save-position-on-quit", "no"),
            ("load-scripts", "no"),
            ("input-default-bindings", "no"),
            ("vd-lavc-skiploopfilter", "all"),
            ("vd-lavc-fast", "yes"),
            ("vd-lavc-threads", "2"),
            ("demuxer-readahead-secs", "0"),
            ("demuxer-max-bytes", "8MiB"),
            ("osd-level", "0"),
        ];
        let mpv = Mpv::with_initializer(|init| {
            for (k, v) in opts {
                let _ = init.set_option(k, *v);
            }
            Ok(())
        })
        .map_err(|e| format!("{e:?}"))?;
        let gl = GlRenderer::new(&mpv, false)?;
        Ok(Self {
            gl,
            mpv,
            target: None,
            aspect: 16.0 / 9.0,
            source: None,
            ready: false,
            failed: false,
            seek_started: None,
            want: None,
            requested: None,
            shown: None,
        })
    }

    /// Point the thumbnailer at a file (or nothing). Cheap when unchanged.
    pub fn set_source(&mut self, path: Option<&str>) {
        if self.source.as_deref() == path {
            return;
        }
        self.source = path.map(str::to_string);
        self.ready = false;
        self.failed = false;
        self.seek_started = None;
        self.want = None;
        self.requested = None;
        self.shown = None;
        match path {
            Some(p) => command_async(&self.mpv, &["loadfile", p, "replace"]),
            None => command_async(&self.mpv, &["stop"]),
        };
    }

    pub fn request(&mut self, t: f64) {
        self.want = Some(t);
    }

    pub fn set_aspect(&mut self, aspect: f64) {
        if aspect.is_finite() && aspect > 0.1 {
            self.aspect = aspect.clamp(0.3, 4.0);
        }
    }

    /// Size of the thumbnail in pixels for the current aspect ratio.
    pub fn size(&self) -> (i32, i32) {
        let h = (THUMB_W as f64 / self.aspect).round() as i32;
        if h > THUMB_W { ((THUMB_W as f64 * THUMB_W as f64 / h as f64) as i32, THUMB_W) } else { (THUMB_W, h.max(16)) }
    }

    pub fn update(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread) {
        loop {
            let ev = unsafe { &*sys::mpv_wait_event(self.mpv.ctx.as_ptr(), 0.0) };
            match ev.event_id {
                sys::mpv_event_id_MPV_EVENT_NONE => break,
                sys::mpv_event_id_MPV_EVENT_FILE_LOADED => self.ready = true,
                sys::mpv_event_id_MPV_EVENT_PLAYBACK_RESTART => self.seek_started = None,
                sys::mpv_event_id_MPV_EVENT_END_FILE => {
                    let ef = unsafe { &*(ev.data as *const sys::mpv_event_end_file) };
                    if ef.reason == sys::mpv_end_file_reason_MPV_END_FILE_REASON_ERROR {
                        self.failed = true;
                    }
                }
                _ => {}
            }
        }
        if !self.ready || self.failed {
            return;
        }
        // A seek that never reports back must not wedge the thumbnailer.
        if self.seek_started.is_some_and(|t| t.elapsed() > Duration::from_secs(2)) {
            self.seek_started = None;
        }
        if self.seek_started.is_none()
            && let Some(want) = self.want
            && self.requested.is_none_or(|r| (r - want).abs() > 0.5)
        {
            let t = format!("{want:.2}");
            if command_async(&self.mpv, &["seek", &t, "absolute+keyframes"]) {
                self.seek_started = Some(Instant::now());
                self.requested = Some(want);
            }
        }

        let (w, h) = self.size();
        let stale = self.target.as_ref().is_none_or(|t| t.texture.width != w || t.texture.height != h);
        if stale {
            self.target = rl.load_render_texture(thread, w as u32, h as u32).ok();
        }
        if (self.gl.wants_render() || stale)
            && let Some(t) = &self.target
        {
            self.gl.render(t.id, w, h, false);
            self.shown = self.requested;
        }
    }

    /// The latest thumbnail, if one exists for the current source.
    pub fn texture(&self) -> Option<&RenderTexture2D> {
        if self.ready && !self.failed && self.shown.is_some() { self.target.as_ref() } else { None }
    }
}
