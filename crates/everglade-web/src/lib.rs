//! Everglade in a browser.
//!
//! The page's `<canvas id="everglade-canvas">` shows the Everglade zone from
//! Verse's `web` build: the module fetches the pinned pack from the same
//! origin, checks its length and SHA-256, installs Everglade directly (no
//! plaza, relay, or studio host), and draws it with WebGPU, or WebGL2 where
//! the browser has no WebGPU. Read `README.md` for the build and the page
//! contract.
//!
//! The same module draws the shared Grid with `?zone=grid`, where the
//! player joins the other players over the browser's WebSocket (`grid`).
//!
//! A native build of this crate holds only the plain data its tests cover.

#[cfg(target_arch = "wasm32")]
mod chamber;
#[cfg(target_arch = "wasm32")]
mod grid;
#[cfg(target_arch = "wasm32")]
mod input;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod presence_ui;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
mod terminal;
