//! Browser-local interaction for the shared Coder component catalog and demo.

#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(target_arch = "wasm32")]
pub use browser::start;

#[cfg(any(target_arch = "wasm32", test))]
mod demo;

#[cfg(target_arch = "wasm32")]
pub use demo::browser::{demo_receipt, start_demo};
