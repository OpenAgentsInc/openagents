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
        #[error("Failed to read image: {source}")]
        Read {
            #[source]
            source: std::io::Error,
        },
        #[error("Invalid image format")]
        InvalidImage,
    }

    impl ImageProcessingError {
        /// Check if this error indicates an invalid image format
        pub fn is_invalid_image(&self) -> bool {
            matches!(self, ImageProcessingError::InvalidImage)
        }
    }
}

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use std::path::Path;

/// Extension trait for converting image bytes to data URLs
pub trait IntoDataUrl {
    /// Convert to a base64 data URL
    fn into_data_url(self) -> String;
}

impl IntoDataUrl for Vec<u8> {
    fn into_data_url(self) -> String {
        let encoded = BASE64.encode(&self);
        // Default to PNG, could be smarter about detecting format
        format!("data:image/png;base64,{}", encoded)
    }
}

/// Stub implementation that returns an error
pub fn load_and_resize_to_fit(
    _path: &Path,
    _max_width: u32,
    _max_height: u32,
) -> Result<Vec<u8>, error::ImageProcessingError> {
    Err(error::ImageProcessingError::NotSupported)
}
