//! Grow Little Bunny in the browser (`docs/verse/games/grow-little-bunny.md`).
//!
//! The rules live in `bunny-rules`; this crate draws one garden run with
//! WebGL2 in the game's outline style (flat gray fills with dark
//! inverted-hull outlines; only the bunny and the carrots in colour), reads
//! the keyboard and swipes, shows a small HUD, and keeps the win count in
//! the browser's storage.
//!
//! It draws with WebGL2 directly rather than through `verse-pbr`: that
//! renderer brings wgpu, naga, glTF and the physics crate, several
//! megabytes of wasm and minutes of build for a few hundred flat-shaded
//! boxes.
//!
//! The page provides `<canvas id="bunny-canvas">` inside a positioned
//! container, and optionally a status line `#bunny-status` the game clears
//! when it starts; `scripts/build-bunny-web.sh` builds it.

pub mod copy;
pub mod mesh;
pub mod scene;

#[cfg(target_arch = "wasm32")]
mod app;

/// Starts the game when the module loads.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    app::start();
}
