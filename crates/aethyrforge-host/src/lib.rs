//! Public integration types. This crate contains no engine implementation.

mod catalog;
mod host;
mod protocol;
pub use catalog::*;
pub use host::*;
pub use protocol::*;

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::*;
mod engine;
pub use engine::*;

#[cfg(target_arch = "wasm32")]
mod demo;
#[cfg(target_arch = "wasm32")]
pub use demo::*;
