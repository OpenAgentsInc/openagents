//! The live sealed-inference demo at openagents.com/att.
//!
//! A Greco-futurist scene drawn with WebGL2 (`scene`, `gl`), the step list
//! and per-step panels (`show`), and the round itself (`flow`).

pub mod copy;
pub mod mesh;
pub mod scene;
pub mod steps;

#[cfg(target_arch = "wasm32")]
pub mod flow;
#[cfg(target_arch = "wasm32")]
mod gl;
#[cfg(target_arch = "wasm32")]
pub mod show;

/// Mounts the page when the module loads.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    if let Some(show) = show::Show::mount() {
        flow::start(show);
    }
}
