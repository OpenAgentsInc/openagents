//! Inline visual windows, loaded only when a transcript draws a visual.
#[cfg(target_family = "wasm")]
mod browser;
#[cfg(target_family = "wasm")]
pub use browser::{mount, reposition, update};
