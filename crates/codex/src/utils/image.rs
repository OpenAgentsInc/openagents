//! Image utilities - stub implementation
//!
//! This is a simplified stub of the codex-utils-image crate.
//! Full image processing can be enabled later if needed.

pub mod error {
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum ImageProcessingError {
        #[error("Image processing not supported in this build")]
        NotSupported,
        #[error("IO error: {0}")]
        Io(#[from] std::io::Error),
    }
}

use std::path::Path;

/// Stub implementation that returns an error
pub fn load_and_resize_to_fit(
    _path: &Path,
    _max_width: u32,
    _max_height: u32,
) -> Result<Vec<u8>, error::ImageProcessingError> {
    Err(error::ImageProcessingError::NotSupported)
}
