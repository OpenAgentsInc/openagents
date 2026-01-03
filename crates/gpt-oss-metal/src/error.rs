use std::ffi::NulError;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GptOssMetalError {
    #[error("GPT-OSS Metal is only supported on macOS")]
    UnsupportedPlatform,
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("FFI error: {0}")]
    FfiError(String),
    #[error("Harmony error: {0}")]
    HarmonyError(String),
    #[error("CString error: {0}")]
    CStringError(#[from] NulError),
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("UTF-8 decode error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
}

pub type Result<T> = std::result::Result<T, GptOssMetalError>;
