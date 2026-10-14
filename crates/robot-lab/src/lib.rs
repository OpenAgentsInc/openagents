//! Robot Lab's checked documents and explicit private project file store.
//!
//! Rendering, physics, dataset decoding, and policy inference are adapters.
//! Stored references name immutable versions, including in cards and jobs.

pub mod document;
pub mod version;
pub use version::{Note, Selection, Version};
// Storage and its operations touch the disk; the browser build only reads
// checked documents.
#[cfg(not(target_arch = "wasm32"))]
pub mod operations;
#[cfg(not(target_arch = "wasm32"))]
pub mod store;

pub use document::{Document, Reference};
#[cfg(not(target_arch = "wasm32"))]
pub use operations::{Operation, Reply};
#[cfg(not(target_arch = "wasm32"))]
pub use store::Store;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("This lab changed. Open the latest version and try again.")]
    Conflict,
    #[error("This item isn't in the lab.")]
    Missing,
    #[error("This saved item is damaged. Restore it from a backup.")]
    Corrupt,
    #[error("The lab couldn't read or save this file: {0}")]
    Io(#[from] std::io::Error),
    #[error("This file couldn't be read: {0}")]
    Json(#[from] serde_json::Error),
}

pub(crate) fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::Invalid(message.into()))
    }
}

pub mod view;
