//! The Haunted House in the browser (`docs/verse/games/the-haunted-house.md`).
//!
//! The house is a `haunt.v1` document (`haunt-rules`); this crate draws it
//! with WebGL2 in first person, Doom style: a quick, smooth glide through
//! boxy halls with a strong vignette and a lantern, so you only see where
//! you look. The art is late-90s cartoon shapes in vintage-Halloween
//! colours: two-band toon shading on low-poly meshes made in code
//! (`haunt-assets`, `toon-mesh`), patterns made in the shader from a
//! pattern id and two palette colours ([`arch`]), die-cut cards that face
//! you, and a paper grain over everything. Sounds are made in the page and
//! mixed through five buses.
//!
//! The page provides `<canvas id="haunt-canvas">`. With `data-live-tenant`
//! it loads the house from that livehost tenant and applies new versions
//! without a reload; without, it plays version 1 built in.

pub mod arch;
pub mod scene;
pub mod sounds;
pub mod walker;

#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
mod follow;
#[cfg(target_arch = "wasm32")]
mod gpu;
#[cfg(target_arch = "wasm32")]
mod live;
#[cfg(target_arch = "wasm32")]
mod sound;
#[cfg(target_arch = "wasm32")]
mod ui;

/// Starts the house when the module loads.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    app::start();
}
