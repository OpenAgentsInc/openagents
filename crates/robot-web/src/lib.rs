//! Browser mounting for the shared Robot Lab state and Verse renderer.
#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
pub use app::start;
