//! Snap Station in the browser (`docs/verse/games/snap-station.md`).
//!
//! The rules live in `snap-rules`; this crate draws a job with WebGL2:
//! bright pieces on a space backdrop, built by dragging pieces from a tray
//! at the bottom of the screen (they snap on when close and lined up), and
//! the run Go starts, drawn from the simulation so leaning squares lean and
//! loose pieces drift (`scene`, `kit`, `mesh`). A small HUD shows the job
//! from its crew member, the tray, the storm clock and the plant gravity
//! meter; cards show the job list, the menu, the results and a short
//! tutorial (`tutorial`). Sounds are made with Web Audio. The player's
//! stars stay in the browser's storage (`snap_rules::progress`).
//!
//! It draws with WebGL2 directly rather than through `verse-pbr`, as Grow
//! Little Bunny does: a few hundred flat-coloured shapes don't need wgpu,
//! naga and glTF. The game's contract with a host is `zone`.
//!
//! The page provides `<canvas id="snap-canvas">` inside a positioned
//! container, and optionally a status line `#snap-status` the game clears
//! when it starts; `scripts/build-snap-web.sh` builds it.

pub mod copy;
pub mod kit;
pub mod mesh;
pub mod options;
pub mod scene;
pub mod tutorial;
pub mod zone;

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
