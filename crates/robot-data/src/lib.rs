//! Checked local LeRobot v2.1/v3.0 files and pure recorded-motion playback.
//! A replay owns no simulator or policy and cannot start a controller.

pub mod dataset;
#[cfg(feature = "export")]
pub mod export;
pub mod mapping;
mod page_bounds;
#[cfg(feature = "publish")]
pub mod publish;
pub mod replay;
mod table;
#[cfg(feature = "video")]
pub mod video;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("This dataset uses {0}. Choose LeRobot v2.1 or v3.0.")]
    Version(String),
    #[error("This recording couldn't be read: {0}")]
    Io(#[from] std::io::Error),
    #[error("This recording's metadata couldn't be read: {0}")]
    Json(#[from] serde_json::Error),
    #[error("This recording's table couldn't be read: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),
    #[error("This recording's table couldn't be read: {0}")]
    Arrow(#[from] arrow_schema::ArrowError),
}

pub(crate) fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::Invalid(message.into()))
    }
}

pub(crate) fn name(value: &str) -> Result<()> {
    require(
        (1..=128).contains(&value.len())
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-.:".contains(&b)),
        "Use a short joint, camera, or feature name.",
    )
}
