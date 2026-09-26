//! A read-only SESS observer, distinct from managed sessions and CTRL task grants.
//! Pairing admits exact retained source collections. It cannot run an agent.
use serde::{Deserialize, Serialize};
use std::fmt;

pub mod client;
#[cfg(feature = "host")]
pub mod host;
pub mod protocol;
#[cfg(feature = "host")]
mod store;
pub mod transport;
pub use client::Client;
pub use coder_history;
pub use protocol::{
    ConnectionCode, Grant, Observation, Query, RelayPolicy, SourceKind, SourceScope,
};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Revoked,
    Expired,
    SourceChanged,
    Unavailable,
    RateLimited,
    Transport,
    Malformed,
    Forbidden,
    Unsupported,
    Bounds,
    Conflict,
}

/// The code is stable; the local diagnostic message carries no source content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
}
impl Error {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}
impl std::error::Error for Error {}
pub(crate) fn fail<T>(code: ErrorCode, message: &str) -> Result<T> {
    Err(Error::new(code, message))
}
pub fn unix_time() -> Result<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| Error::new(ErrorCode::Unavailable, "system clock is unavailable"))
}

#[cfg(all(test, feature = "host"))]
mod tests;
