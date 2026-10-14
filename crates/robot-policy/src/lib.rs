//! Robot policy inference uses explicit input mappings and pinned weights.
//! Models produce joint-position chunks; they cannot start a robot or job.
pub mod act;
pub mod artifacts;
pub mod controller;
pub mod preprocess;
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("The policy couldn't compute this call: {0}")]
    Tensor(#[from] candle_core::Error),
    #[error("The policy file couldn't be read: {0}")]
    Io(#[from] std::io::Error),
    #[error("The policy settings couldn't be read: {0}")]
    Json(#[from] serde_json::Error),
}
pub(crate) fn require(valid: bool, message: &str) -> Result<()> {
    if valid {
        Ok(())
    } else {
        Err(Error::Invalid(message.into()))
    }
}
