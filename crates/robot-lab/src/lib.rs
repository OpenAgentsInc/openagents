//! Robot Lab's checked documents and explicit private project file store.
//!
//! Rendering, physics, dataset decoding, and policy inference are adapters.
//! Stored references name immutable versions, including in cards and jobs.

pub mod document;
pub mod operations;
pub mod store;

pub use document::{Document, Reference};
pub use operations::{Operation, Reply};
pub use store::{Store, Version};

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
