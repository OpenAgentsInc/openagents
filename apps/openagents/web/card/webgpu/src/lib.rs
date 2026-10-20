//! The agent demo card, WebGPU only: `openagents-web-card` with its
//! `mount`, linked against a wgpu without the WebGL2 backend.

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

/// Draws the card in `#gpui-mount`; see `openagents_web_card::run`.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(still: bool) {
    openagents_web_card::run(still);
}
