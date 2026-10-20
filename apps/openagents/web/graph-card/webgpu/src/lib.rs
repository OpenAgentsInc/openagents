//! The AgentGraph demo, WebGPU only: `openagents-web-graph-card` with its
//! `mount`, linked against a wgpu without the WebGL2 backend.

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

/// Draws the demo in `#gpui-mount`; see `openagents_web_graph_card::run`.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(still: bool) {
    openagents_web_graph_card::run(still);
}
