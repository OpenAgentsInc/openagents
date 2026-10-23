//! Public integration types. This crate contains no engine implementation.

mod access;
pub use access::*;
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

#[cfg(target_arch = "wasm32")]
mod studio_browser;
#[cfg(target_arch = "wasm32")]
pub use studio_browser::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn protocol_version() -> u16 {
    VERSION
}
