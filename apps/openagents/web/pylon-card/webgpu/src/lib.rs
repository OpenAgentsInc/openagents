//! The pylon card, WebGPU only: `openagents-web-pylon-card` with its
//! `mount` and `show`, linked against a wgpu without the WebGL2 backend.

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

/// Draws the card in `#gpui-mount`; see `openagents_web_pylon_card::run`.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(still: bool) {
    openagents_web_pylon_card::run(still);
}

/// Hands the card a pylon; see `openagents_web_pylon_card::take`.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn show(json: &str) {
    openagents_web_pylon_card::take(json);
}
