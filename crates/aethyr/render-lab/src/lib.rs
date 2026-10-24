//! Render lab for decisions 0006–0008.
//!
//! One Rust renderer drives browser WebGPU (wasm32, through `af-gpu`'s import
//! table) and native `webgpu.h` (Dawn or wgpu-native). It compares the
//! CPU-batched submission the current engine resembles with GPU-driven
//! culling, a visibility buffer, temporal anti-aliasing, LOD, and crowd tiers.
//! See `docs/research/render-lab.md`.

pub mod camera;
pub mod crowd;
pub mod lab;
pub mod scene;

#[cfg(feature = "native")]
pub mod capi;

pub use lab::{Aa, Config, Encoder, Lab, Path, ShaderSource, Stats};

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        static LAB: RefCell<Option<Lab>> = const { RefCell::new(None) };
        static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    }

    #[link(wasm_import_module = "af_lab")]
    unsafe extern "C" {
        /// Encodes the CPU path's draws in TypeScript; returns elapsed ms, or a
        /// negative value when the host has no TypeScript encoder.
        fn ts_encode(pass: u32, log: *const u32, len: u32) -> f64;
    }

    fn ts_hook(pass: af_gpu::Handle, log: &[u32]) -> Option<f64> {
        let ms = unsafe { ts_encode(pass, log.as_ptr(), log.len() as u32) };
        (ms >= 0.0).then_some(ms)
    }

    fn config_from(ptr: *const u8, len: u32) -> Config {
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
        serde_json::from_slice(bytes).unwrap_or_default()
    }

    fn publish(text: String) -> u32 {
        OUT.with(|o| {
            let mut o = o.borrow_mut();
            *o = text.into_bytes();
            o.len() as u32
        })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn af_lab_create(width: u32, height: u32, ptr: *const u8, len: u32) {
        let mut lab = Lab::new(af_gpu::Gpu::from_host(), config_from(ptr, len), width, height);
        lab.set_external_encoder(ts_hook);
        LAB.with(|l| *l.borrow_mut() = Some(lab));
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn af_lab_configure(ptr: *const u8, len: u32) {
        let config = config_from(ptr, len);
        LAB.with(|l| l.borrow_mut().as_mut().map(|lab| lab.set_config(config)));
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn af_lab_resize(width: u32, height: u32) {
        LAB.with(|l| l.borrow_mut().as_mut().map(|lab| lab.resize(width, height)));
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn af_lab_orbit(dx: f32, dy: f32, zoom: f32) {
        LAB.with(|l| l.borrow_mut().as_mut().map(|lab| lab.orbit(dx, dy, zoom)));
    }

    /// Renders a frame; returns the length of the stats JSON at `af_lab_output`.
    #[unsafe(no_mangle)]
    pub extern "C" fn af_lab_frame(time_ms: f64) -> u32 {
        let text = LAB.with(|l| l.borrow_mut().as_mut().map(|lab| serde_json::to_string(lab.frame(time_ms)).unwrap()));
        publish(text.unwrap_or_default())
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn af_lab_output() -> *const u8 {
        OUT.with(|o| o.borrow().as_ptr())
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn af_lab_slang_available() -> u32 {
        lab::slang_available() as u32
    }
}
