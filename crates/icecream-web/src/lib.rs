//! The Ice Cream Man in the browser (`docs/verse/games/the-ice-cream-man.md`).
//!
//! The rules live in `icecream-rules`; this crate draws one run with WebGL2
//! the way Grow Little Bunny does (low-poly meshes made in code, `mesh`,
//! with inverted-hull outlines), but in pastel ice-cream colours under
//! flickering freezer lights. It reads the keyboard, the mouse and touch
//! with Roblox-style controls (`camera` holds the orbit camera and Shift
//! Lock), shows a DOM HUD with a hot bar, plays placeholder sounds made
//! with Web Audio, and opens a four-card tutorial (`tutorial`) on the first
//! run.
//!
//! The page provides `<canvas id="icecream-canvas">` inside a positioned
//! container, and optionally a status line `#icecream-status` the game
//! clears when it starts; `scripts/build-icecream-web.sh` builds it.

pub mod camera;
pub mod copy;
pub mod mesh;
pub mod options;
pub mod scene;
pub mod settings;
pub mod tutorial;

#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
mod hud;
#[cfg(target_arch = "wasm32")]
mod sound;

/// Starts the game when the module loads.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    app::start();
}
