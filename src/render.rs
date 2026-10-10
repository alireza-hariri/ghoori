//! Bridge between mpv's OpenGL render API and raylib's GL context.
//!
//! The render context is driven through `libmpv2-sys` directly: the safe
//! wrapper's FBO struct lacks the `internal_format` field mpv reads, and we
//! need `BLOCK_FOR_TARGET_TIME` control per context.

use libmpv2::Mpv;
use libmpv2_sys as sys;
use raylib::ffi;
use std::ffi::{c_char, c_int, c_void};
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

unsafe extern "C" fn get_proc_address(_ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    unsafe { ffi::rlGetProcAddress(name) }
}

unsafe extern "C" {
    // Provided by the GLFW copy statically linked into raylib.
    fn glfwGetPlatform() -> c_int;
    fn glfwGetWaylandDisplay() -> *mut c_void;
    fn glfwGetX11Display() -> *mut c_void;
    fn glfwGetCurrentContext() -> *mut c_void;
    fn glfwGetWindowSize(window: *mut c_void, w: *mut c_int, h: *mut c_int);
}

const GLFW_X11_CLASS_NAME: c_int = 0x0002_4001;
const GLFW_X11_INSTANCE_NAME: c_int = 0x0002_4002;
const GLFW_WAYLAND_APP_ID: c_int = 0x0002_6001;

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn glfwWindowHintString(hint: c_int, value: *const c_char);
    fn __real_glfwCreateWindow(
        w: c_int,
        h: c_int,
        title: *const c_char,
        mon: *mut c_void,
        share: *mut c_void,
    ) -> *mut c_void;
}

/// Linked in place of `glfwCreateWindow` (see build.rs): names the window "ghoori" so the desktop matches it
/// with ghoori.desktop (taskbar icon, grouping) on Wayland (app-id) and X11 (WM_CLASS).
#[cfg(target_os = "linux")]
#[unsafe(no_mangle)]
unsafe extern "C" fn __wrap_glfwCreateWindow(
    w: c_int,
    h: c_int,
    title: *const c_char,
    mon: *mut c_void,
    share: *mut c_void,
) -> *mut c_void {
    unsafe {
        for hint in [GLFW_WAYLAND_APP_ID, GLFW_X11_CLASS_NAME, GLFW_X11_INSTANCE_NAME] {
            glfwWindowHintString(hint, c"ghoori".as_ptr());
        }
        __real_glfwCreateWindow(w, h, title, mon, share)
    }
}

const GLFW_PLATFORM_WAYLAND: c_int = 0x0006_0003;
const GLFW_PLATFORM_X11: c_int = 0x0006_0004;

/// The native display handle mpv needs for hardware-decoding interop.
fn native_display() -> Option<(sys::mpv_render_param_type, *mut c_void)> {
    let (kind, ptr) = unsafe {
        match glfwGetPlatform() {
            GLFW_PLATFORM_WAYLAND => (sys::mpv_render_param_type_MPV_RENDER_PARAM_WL_DISPLAY, glfwGetWaylandDisplay()),
            GLFW_PLATFORM_X11 => (sys::mpv_render_param_type_MPV_RENDER_PARAM_X11_DISPLAY, glfwGetX11Display()),
            _ => return None,
        }
    };
    (!ptr.is_null()).then_some((kind, ptr))
}

/// Window size in GLFW screen coordinates, the space cursor positions are reported in.
pub fn window_size() -> Option<(i32, i32)> {
    let (mut w, mut h) = (0, 0);
    unsafe {
        let win = glfwGetCurrentContext();
        if win.is_null() {
            return None;
        }
        glfwGetWindowSize(win, &mut w, &mut h);
    }
    (w > 0 && h > 0).then_some((w, h))
}

pub fn is_wayland() -> bool {
    unsafe { glfwGetPlatform() == GLFW_PLATFORM_WAYLAND }
}

unsafe extern "C" fn on_update(data: *mut c_void) {
    // Called from an mpv thread: only flag, never call back into mpv here.
    let flag = unsafe { &*(data as *const AtomicBool) };
    flag.store(true, Ordering::Release);
}

pub struct GlRenderer {
    ctx: *mut sys::mpv_render_context,
    dirty: Arc<AtomicBool>,
    block: bool,
}

fn param(type_: sys::mpv_render_param_type, data: *mut c_void) -> sys::mpv_render_param {
    sys::mpv_render_param { type_, data }
}

impl GlRenderer {
    /// Must be called on the thread owning the raylib GL context.
    /// `block` controls whether rendering waits for the frame's target display time.
    pub fn new(mpv: &Mpv, block: bool) -> Result<Self, String> {
        let mut init = sys::mpv_opengl_init_params {
            get_proc_address: Some(get_proc_address),
            get_proc_address_ctx: ptr::null_mut(),
        };
        let mut params = vec![
            param(
                sys::mpv_render_param_type_MPV_RENDER_PARAM_API_TYPE,
                sys::MPV_RENDER_API_TYPE_OPENGL.as_ptr() as *mut c_void,
            ),
            param(sys::mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_INIT_PARAMS, &mut init as *mut _ as *mut c_void),
        ];
        if let Some((kind, display)) = native_display() {
            params.push(param(kind, display));
        }
        params.push(param(sys::mpv_render_param_type_MPV_RENDER_PARAM_INVALID, ptr::null_mut()));
        let mut ctx = ptr::null_mut();
        let err = unsafe { sys::mpv_render_context_create(&mut ctx, mpv.ctx.as_ptr(), params.as_mut_ptr()) };
        if err < 0 {
            return Err(crate::player::mpv_err_str(err));
        }
        let dirty = Arc::new(AtomicBool::new(true));
        unsafe {
            sys::mpv_render_context_set_update_callback(ctx, Some(on_update), Arc::as_ptr(&dirty) as *mut c_void);
        }
        Ok(Self { ctx, dirty, block })
    }

    /// Returns true when mpv has a new frame (or needs a redraw) since the last call.
    pub fn wants_render(&self) -> bool {
        if !self.dirty.swap(false, Ordering::AcqRel) {
            return false;
        }
        let flags = unsafe { sys::mpv_render_context_update(self.ctx) };
        flags & sys::mpv_render_update_flag_MPV_RENDER_UPDATE_FRAME as u64 != 0
    }

    /// Render the current frame into `fbo` (0 = default framebuffer) and restore raylib's GL state.
    pub fn render(&self, fbo: u32, w: i32, h: i32, flip_y: bool) {
        unsafe {
            ffi::rlDrawRenderBatchActive();
            // mpv assumes default GL state; raylib enables culling, blending and may scissor.
            ffi::rlDisableBackfaceCulling();
            ffi::rlDisableDepthTest();
            ffi::rlDisableScissorTest();
            ffi::rlDisableColorBlend();
        }
        let mut target = sys::mpv_opengl_fbo { fbo: fbo as c_int, w, h, internal_format: 0 };
        let mut flip: c_int = flip_y as c_int;
        let mut block: c_int = self.block as c_int;
        let mut params = [
            param(sys::mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_FBO, &mut target as *mut _ as *mut c_void),
            param(sys::mpv_render_param_type_MPV_RENDER_PARAM_FLIP_Y, &mut flip as *mut _ as *mut c_void),
            param(
                sys::mpv_render_param_type_MPV_RENDER_PARAM_BLOCK_FOR_TARGET_TIME,
                &mut block as *mut _ as *mut c_void,
            ),
            param(sys::mpv_render_param_type_MPV_RENDER_PARAM_INVALID, ptr::null_mut()),
        ];
        unsafe {
            sys::mpv_render_context_render(self.ctx, params.as_mut_ptr());
            restore_raylib_gl_state();
        }
    }

    pub fn report_swap(&self) {
        unsafe { sys::mpv_render_context_report_swap(self.ctx) };
    }
}

impl Drop for GlRenderer {
    fn drop(&mut self) {
        unsafe {
            sys::mpv_render_context_set_update_callback(self.ctx, None, ptr::null_mut());
            sys::mpv_render_context_free(self.ctx);
            restore_raylib_gl_state();
        }
    }
}

/// mpv freely changes GL state; put back what raylib's batch renderer assumes.
unsafe fn restore_raylib_gl_state() {
    unsafe {
        ffi::rlDisableFramebuffer();
        ffi::rlViewport(0, 0, ffi::GetRenderWidth(), ffi::GetRenderHeight());
        ffi::rlDisableScissorTest();
        ffi::rlDisableDepthTest();
        ffi::rlEnableBackfaceCulling();
        ffi::rlEnableColorBlend();
        // rlSetBlendMode caches the mode, so bounce through another one to force re-apply.
        ffi::rlSetBlendMode(ffi::BlendMode::BLEND_ADDITIVE as i32);
        ffi::rlSetBlendMode(ffi::BlendMode::BLEND_ALPHA as i32);
    }
}
