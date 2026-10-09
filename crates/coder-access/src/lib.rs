//! Host-wide device enrollment and scoped rights for one Coder host (NIP-HOST).
//!
//! The host is the only issuer of access. An invitation, an approval, or a
//! relay delivery introduces a device; only the host's current grant record
//! admits an operation. The client modules build without the host store.
use serde::{Deserialize, Serialize};
use std::fmt;

pub mod agent;
#[cfg(not(target_arch = "wasm32"))]
pub mod cj;
pub mod client;
pub mod cloud;
pub mod computer;
pub mod crew;
pub mod day_plan;
pub mod environment;
#[cfg(feature = "host")]
pub mod host;
pub mod media;
pub mod project;
pub mod protocol;
pub mod review;
pub mod rights;
pub mod spend;
pub mod studio;
pub mod studio_intents;
pub mod task_read;
pub mod thread;
pub mod wallet_link;

pub use client::{Client, Pending};
pub use coder_connect::RelayPolicy;
pub use protocol::{Access, CommandAction, Enrollment, Grant, Operation, Outcome, TaskCommand};
pub use rights::{Right, Rights};

pub type Result<T> = std::result::Result<T, Error>;

/// Stable refusal codes. A `missing_right` refusal also names the right.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Code {
    Malformed,
    Unsupported,
    Forbidden,
    MissingRight,
    Expired,
    Revoked,
    Stale,
    Conflict,
    Bounds,
    Unavailable,
    Transport,
    RateLimited,
    WrongCode,
    Denied,
}

/// The local diagnostic message carries no capability, code, or key material.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub code: Code,
    pub missing: Option<Right>,
    pub message: String,
}
impl Error {
    pub fn new(code: Code, message: impl Into<String>) -> Self {
        Self {
            code,
            missing: None,
            message: message.into(),
        }
    }
    pub fn missing(right: Right) -> Self {
        Self {
            code: Code::MissingRight,
            missing: Some(right),
            message: format!("this device does not hold the `{}` right", right.as_str()),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.missing {
            Some(right) => write!(f, "{:?} ({}): {}", self.code, right.as_str(), self.message),
            None => write!(f, "{:?}: {}", self.code, self.message),
        }
    }
}
impl std::error::Error for Error {}
impl From<coder_connect::Error> for Error {
    fn from(error: coder_connect::Error) -> Self {
        use coder_connect::ErrorCode as C;
        let code = match error.code {
            C::Revoked => Code::Revoked,
            C::Expired => Code::Expired,
            C::SourceChanged => Code::Stale,
            C::Unavailable => Code::Unavailable,
            C::RateLimited => Code::RateLimited,
            C::Transport => Code::Transport,
            C::Malformed => Code::Malformed,
            C::Forbidden => Code::Forbidden,
            C::Unsupported => Code::Unsupported,
            C::Bounds => Code::Bounds,
            C::Conflict => Code::Conflict,
        };
        // The shared primitives describe their failures in observer terms.
        // Keep the stable code and replace the wording with an access context.
        Self::new(code, "host access artifact or transport check failed")
    }
}
pub(crate) fn fail<T>(code: Code, message: &str) -> Result<T> {
    Err(Error::new(code, message))
}
pub fn unix_time() -> Result<u64> {
    coder_connect::unix_time().map_err(Error::from)
}

#[cfg(all(test, feature = "host"))]
mod tests;
