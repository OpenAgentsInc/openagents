//! Shared controllers for the separate Claude and Data page modules.
#[cfg(target_family = "wasm")]
mod page;
#[cfg(target_family = "wasm")]
pub use page::AccountPage;
