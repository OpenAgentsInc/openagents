//! Web platform implementation for GPUI using wgpu
//!
//! This module provides browser support for GPUI applications, using wgpu
//! for WebGPU/WebGL2 rendering.

mod dispatcher;
mod platform;
mod text_system;
mod wgpu;
mod window;

pub(crate) use platform::WebPlatform;
pub(crate) use wgpu::WgpuRenderer;

use wasm_bindgen::prelude::*;

/// Initialize web platform
#[wasm_bindgen(start)]
pub fn init_web() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).ok();
    log::info!("GPUI web platform initialized");
}
