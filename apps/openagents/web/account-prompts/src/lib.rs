//! Account memory and schedule controllers shared by two lazy page modules.
#[cfg(target_family = "wasm")]
mod pages;
#[cfg(target_family = "wasm")]
pub use pages::{Memory, Schedules};
