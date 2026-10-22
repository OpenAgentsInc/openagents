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

#[cfg(any(target_arch = "wasm32", test))]
mod arch_marks;
#[cfg(target_arch = "wasm32")]
mod build_board;
#[cfg(target_arch = "wasm32")]
mod cad_sync;
#[cfg(target_arch = "wasm32")]
mod chamber;
#[cfg(target_arch = "wasm32")]
mod controls_hint;
#[cfg(target_arch = "wasm32")]
mod foreman;
#[cfg(target_arch = "wasm32")]
mod game_lab;
#[cfg(target_arch = "wasm32")]
mod grid;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod hand_input;
#[cfg(target_arch = "wasm32")]
mod help_desk;
#[cfg(target_arch = "wasm32")]
mod home_terminal;
#[cfg(target_arch = "wasm32")]
mod hosted;
#[cfg(target_arch = "wasm32")]
mod input;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod load_screen;
#[cfg(target_arch = "wasm32")]
mod load_view;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod overview;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod presence_ui;
#[cfg(target_arch = "wasm32")]
mod pylon_bay;
#[cfg(target_arch = "wasm32")]
mod suggestion_box;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod transit;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod visit;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod zone_ui;

#[cfg(any(target_arch = "wasm32", test))]
mod spawn_guard;
#[cfg(target_arch = "wasm32")]
mod terminal;
#[cfg(target_arch = "wasm32")]
mod theme;

#[cfg(all(test, target_arch = "wasm32"))]
mod light_proof;
