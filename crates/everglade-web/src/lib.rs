//! Everglade in a browser.
//!
//! The page's `<canvas id="everglade-canvas">` shows the Everglade zone from
//! Verse's `web` build: the module fetches the pinned pack from the same
//! origin, checks its length and SHA-256, installs Everglade directly (no
//! plaza, relay, or studio host), and draws it with WebGPU, or WebGL2 where
//! the browser has no WebGPU. Read `README.md` for the build and the page
//! contract.
//!
//! A native build of this crate is empty.

#[cfg(target_arch = "wasm32")]
mod input;
#[cfg(target_arch = "wasm32")]
mod web;
