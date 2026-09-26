//! Private Gym boards and separately admitted recipe launches over Nostr.
//! A board is an attributable projection, not independent evaluation evidence.
pub use coder_connect::protocol::{pubkey, random_id};
pub use coder_connect::{Error, ErrorCode, RelayPolicy, unix_time};
pub type Result<T> = std::result::Result<T, Error>;

mod protocol;
pub use protocol::*;
mod client;
pub mod transport;
pub use client::Client;
#[cfg(feature = "host")]
mod confined;
#[cfg(feature = "host")]
pub mod host;
#[cfg(feature = "host")]
mod sources;
#[cfg(feature = "host")]
mod store;

pub(crate) fn error(code: ErrorCode, message: &str) -> Error {
    Error::new(code, message)
}

#[cfg(all(test, feature = "host"))]
mod tests;

/// Only a verified signed domain refusal establishes that this request was
/// refused. Other errors, including reply validation failures, can follow
/// dispatch and must preserve the original launch ID for reconciliation.
pub fn confirmed_refusal(error: &Error) -> bool {
    error.message == "Gym host refused this request"
}
