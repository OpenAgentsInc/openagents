//! The /lander8 chat, WebGPU only: `openagents-web-lander-chat` with its
//! `mount`, linked against a wgpu without the WebGL2 backend.

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

/// Draws the chat in `#gpui-mount`; see `openagents_web_lander_chat::run`.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(still: bool) {
    openagents_web_lander_chat::run(still);
}
