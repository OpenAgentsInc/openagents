//! C ABI for the render lab on native `webgpu.h` (feature `native`).
//!
//! The browser build exports `af_lab_*` over a single global lab; natively a
//! host may run several, so each call takes the handle `af_lab_native_create`
//! returned. Frames render offscreen, or into a host window created with
//! `af_lab_native_create_window`; the stats JSON is the same document the web
//! lab reports (`Stats`).
//!
//! Strings returned by these functions stay valid until the next call on the
//! same handle (or, for `af_lab_native_last_error`, the next failing call on the
//! same thread). JSON inputs are UTF-8 and need not be NUL-terminated.

use crate::{Config, Lab};
use af_gpu::{Gpu, NativeOptions, PresentMode, SurfaceSource};
use serde::Deserialize;
use std::cell::RefCell;
use std::ffi::{CString, c_char};

/// Adapter selection and pacing, as JSON:
/// `{"backend": "vulkan" | "metal" | ..., "low-power": false, "present-mode": "fifo" | "mailbox" | "immediate"}`.
/// The present mode only applies to a window.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct HostOptions {
    pub backend: Option<String>,
    pub low_power: bool,
    pub present_mode: Option<String>,
}

impl HostOptions {
    fn native(&self, width: u32, height: u32, surface: Option<SurfaceSource>) -> Result<NativeOptions, String> {
        let present_mode = match self.present_mode.as_deref() {
            None => PresentMode::Fifo,
            Some(name) => PresentMode::parse(name).ok_or_else(|| format!("unknown present mode {name}"))?,
        };
        Ok(NativeOptions { width, height, backend: self.backend.clone(), low_power: self.low_power, surface, present_mode, ..Default::default() })
    }
}

pub struct NativeLab {
    lab: Lab,
    out: CString,
}

impl NativeLab {
    /// Creates a device and a lab rendering offscreen at `width` x `height`.
    pub fn new(width: u32, height: u32, config: Config, host: &HostOptions) -> Result<NativeLab, String> {
        Self::with_surface(width, height, config, host, None)
    }

    /// Creates a device and a lab presenting into a window (or offscreen
    /// when `surface` is `None`). The window must outlive the lab.
    pub fn with_surface(width: u32, height: u32, config: Config, host: &HostOptions, surface: Option<SurfaceSource>) -> Result<NativeLab, String> {
        let gpu = Gpu::new_native(&host.native(width, height, surface)?)?;
        Ok(NativeLab { lab: Lab::new(gpu, config, width, height), out: CString::default() })
    }

    pub fn lab(&mut self) -> &mut Lab {
        &mut self.lab
    }

    fn publish(&mut self, text: String) -> *const c_char {
        self.out = CString::new(text).unwrap_or_default();
        self.out.as_ptr()
    }
}

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

fn set_error(message: &str) {
    LAST_ERROR.with(|e| *e.borrow_mut() = CString::new(message.replace('\0', " ")).unwrap_or_default());
}

/// # Safety
/// `ptr` is null or points to `len` readable bytes.
unsafe fn bytes<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if ptr.is_null() || len == 0 { &[] } else { unsafe { std::slice::from_raw_parts(ptr, len) } }
}

fn parse<T: for<'de> Deserialize<'de> + Default>(data: &[u8]) -> Result<T, String> {
    if data.is_empty() {
        return Ok(T::default());
    }
    serde_json::from_slice(data).map_err(|e| e.to_string())
}

/// Creates a lab. Returns null on failure; see `af_lab_native_last_error`.
///
/// # Safety
/// The JSON pointers are null or point to the given number of readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn af_lab_native_create(
    width: u32,
    height: u32,
    config: *const u8,
    config_len: usize,
    options: *const u8,
    options_len: usize,
) -> *mut NativeLab {
    let result = parse::<Config>(unsafe { bytes(config, config_len) })
        .and_then(|config| Ok((config, parse::<HostOptions>(unsafe { bytes(options, options_len) })?)))
        .and_then(|(config, host)| NativeLab::new(width, height, config, &host));
    match result {
        Ok(lab) => Box::into_raw(Box::new(lab)),
        Err(message) => {
            set_error(&message);
            std::ptr::null_mut()
        }
    }
}

/// Creates a lab that presents into a host window. `kind` selects the meaning
/// of `a` and `b`: 1 = Metal (`CAMetalLayer*`, unused), 2 = Xlib (`Display*`,
/// `Window`), 3 = Wayland (`wl_display*`, `wl_surface*`), 4 = Win32
/// (`HINSTANCE`, `HWND`). Returns null on failure; see `af_lab_native_last_error`.
///
/// # Safety
/// As `af_lab_native_create`; the window handles stay valid until the lab is destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn af_lab_native_create_window(
    width: u32,
    height: u32,
    config: *const u8,
    config_len: usize,
    options: *const u8,
    options_len: usize,
    kind: u32,
    a: *mut std::ffi::c_void,
    b: *mut std::ffi::c_void,
) -> *mut NativeLab {
    let surface = match kind {
        1 => Ok(SurfaceSource::MetalLayer(a)),
        2 => Ok(SurfaceSource::Xlib { display: a, window: b as u64 }),
        3 => Ok(SurfaceSource::Wayland { display: a, surface: b }),
        4 => Ok(SurfaceSource::Windows { hinstance: a, hwnd: b }),
        other => Err(format!("unknown surface kind {other}")),
    };
    let result = surface.and_then(|surface| {
        let config = parse::<Config>(unsafe { bytes(config, config_len) })?;
        let host = parse::<HostOptions>(unsafe { bytes(options, options_len) })?;
        NativeLab::with_surface(width, height, config, &host, Some(surface))
    });
    match result {
        Ok(lab) => Box::into_raw(Box::new(lab)),
        Err(message) => {
            set_error(&message);
            std::ptr::null_mut()
        }
    }
}

/// The last error on this thread, NUL-terminated.
#[unsafe(no_mangle)]
pub extern "C" fn af_lab_native_last_error() -> *const c_char {
    LAST_ERROR.with(|e| e.borrow().as_ptr())
}

/// Replaces the configuration. Returns 0 on success, -1 for invalid JSON.
///
/// # Safety
/// `lab` came from `af_lab_native_create`; `config` points to `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn af_lab_native_configure(lab: *mut NativeLab, config: *const u8, len: usize) -> i32 {
    let Some(lab) = (unsafe { lab.as_mut() }) else { return -1 };
    match parse::<Config>(unsafe { bytes(config, len) }) {
        Ok(config) => {
            lab.lab.set_config(config);
            0
        }
        Err(message) => {
            set_error(&message);
            -1
        }
    }
}

/// # Safety
/// `lab` came from `af_lab_native_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn af_lab_native_resize(lab: *mut NativeLab, width: u32, height: u32) {
    if let Some(lab) = unsafe { lab.as_mut() } {
        lab.lab.resize(width, height);
    }
}

/// # Safety
/// `lab` came from `af_lab_native_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn af_lab_native_orbit(lab: *mut NativeLab, dx: f32, dy: f32, zoom: f32) {
    if let Some(lab) = unsafe { lab.as_mut() } {
        lab.lab.orbit(dx, dy, zoom);
    }
}

/// Renders one frame and returns the stats JSON.
///
/// # Safety
/// `lab` came from `af_lab_native_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn af_lab_native_frame(lab: *mut NativeLab, time_ms: f64) -> *const c_char {
    let Some(lab) = (unsafe { lab.as_mut() }) else { return std::ptr::null() };
    let text = serde_json::to_string(lab.lab.frame(time_ms)).unwrap_or_default();
    lab.publish(text)
}

/// The latest stats JSON without rendering.
///
/// # Safety
/// `lab` came from `af_lab_native_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn af_lab_native_stats(lab: *mut NativeLab) -> *const c_char {
    let Some(lab) = (unsafe { lab.as_mut() }) else { return std::ptr::null() };
    let text = serde_json::to_string(lab.lab.stats()).unwrap_or_default();
    lab.publish(text)
}

/// Blocks until submitted GPU work finishes.
///
/// # Safety
/// `lab` came from `af_lab_native_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn af_lab_native_wait_idle(lab: *mut NativeLab) {
    if let Some(lab) = unsafe { lab.as_ref() } {
        lab.lab.gpu().wait_idle();
    }
}

/// Copies the last frame as RGBA8 (`width * height * 4` bytes) into `out`.
/// Returns the bytes written, or 0 when `cap` is too small or capture failed.
///
/// # Safety
/// `lab` came from `af_lab_native_create`; `out` points to `cap` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn af_lab_native_capture(lab: *mut NativeLab, out: *mut u8, cap: usize) -> usize {
    let Some(lab) = (unsafe { lab.as_mut() }) else { return 0 };
    let Some((_, _, rgba)) = lab.lab.capture() else { return 0 };
    if out.is_null() || rgba.len() > cap {
        return 0;
    }
    unsafe { std::ptr::copy_nonoverlapping(rgba.as_ptr(), out, rgba.len()) };
    rgba.len()
}

/// # Safety
/// `lab` came from `af_lab_native_create` and is not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn af_lab_native_destroy(lab: *mut NativeLab) {
    if !lab.is_null() {
        drop(unsafe { Box::from_raw(lab) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_options_parse() {
        let o: HostOptions = parse(br#"{"backend":"metal","low-power":true}"#).unwrap();
        assert_eq!(o.backend.as_deref(), Some("metal"));
        assert!(o.low_power);
        let o: HostOptions = parse(br#"{"present-mode":"mailbox"}"#).unwrap();
        assert_eq!(o.native(8, 8, None).unwrap().present_mode, PresentMode::Mailbox);
        let o: HostOptions = parse(br#"{"present-mode":"vsync"}"#).unwrap();
        assert!(o.native(8, 8, None).is_err());
        assert!(parse::<HostOptions>(b"").unwrap().backend.is_none());
        assert!(parse::<Config>(b"{").is_err());
    }

    #[test]
    fn create_window_rejects_unknown_surfaces() {
        let lab = unsafe { af_lab_native_create_window(64, 64, std::ptr::null(), 0, std::ptr::null(), 0, 9, std::ptr::null_mut(), std::ptr::null_mut()) };
        assert!(lab.is_null());
        let message = unsafe { std::ffi::CStr::from_ptr(af_lab_native_last_error()) };
        assert!(message.to_string_lossy().contains("surface kind"));
    }

    #[test]
    fn create_reports_errors_instead_of_panicking() {
        let bad = b"{not json";
        let lab = unsafe { af_lab_native_create(64, 64, bad.as_ptr(), bad.len(), std::ptr::null(), 0) };
        assert!(lab.is_null());
        let message = unsafe { std::ffi::CStr::from_ptr(af_lab_native_last_error()) };
        assert!(!message.to_bytes().is_empty());
    }

    /// Renders every path where an adapter exists; skips in GPU-less containers.
    #[test]
    fn every_path_renders_natively() {
        let config = Config { instances: 2_000, variants: 2, crowd: 500, time: Some(4.0), ..Default::default() };
        let mut lab = match NativeLab::new(160, 90, config.clone(), &HostOptions::default()) {
            Ok(lab) => lab,
            Err(e) => {
                eprintln!("no adapter ({e}); skipping");
                return;
            }
        };
        for path in [crate::Path::Cpu, crate::Path::Gpu, crate::Path::Visbuffer] {
            lab.lab().set_config(Config { path, ..config.clone() });
            for i in 0..8 {
                lab.lab().frame(i as f64 * 16.0);
            }
            lab.lab().gpu().wait_idle();
            let stats = lab.lab().frame(200.0).clone();
            assert!(stats.errors.is_empty(), "{path:?}: {:?}", stats.errors);
            assert!(stats.visible_instances > 0, "{path:?} drew nothing");
            let (_, _, rgba) = lab.lab().capture().expect("offscreen capture");
            assert!(rgba.as_chunks::<4>().0.iter().any(|p| p[0] > 0 || p[1] > 0 || p[2] > 0), "{path:?} image is black");
        }
    }
}
