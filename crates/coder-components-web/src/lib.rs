//! Browser-local fixture interaction for the shared Coder component catalog.

#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(target_arch = "wasm32")]
pub use browser::start;
