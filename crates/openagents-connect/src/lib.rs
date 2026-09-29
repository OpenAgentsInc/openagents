//! Connect a phone to a computer over iroh.
//!
//! This crate is the shared library behind QR pairing
//! (`docs/coder/design/2026-09-29-auto-pairing.md`). The host
//! (`coder host serve`), the desktop app, `openagents connect`, and the
//! phone's Rust core all build on it:
//!
//! - [`endpoint`]: an iroh endpoint with `presets::Minimal`, our relay only
//!   (`RelayMode::Custom`) or no relay, and a [`iroh::address_lookup::MemoryLookup`]
//!   fed from the QR code and from NIP-REACH hints. No n0 relay, DNS, or DHT.
//! - [`ENROLL_ALPN`] and [`REACH_ALPN`]: the two application protocols a host
//!   serves. [`enroll`] carries one NIP-HOST `enroll.redeem` request and its
//!   reply; [`reach`] runs the unchanged NIP-REACH direct-channel handshake
//!   (`coder_reach::channel`) over one QUIC bidirectional stream.
//! - [`code`]: the `openagents-connect:` QR payload, a carriage for a NIP-HOST
//!   host invitation plus the computer's iroh address.
//! - [`ledger`]: NIP-HOST's single-use redemption rules as a pure state
//!   machine, for fakes and tests.
//! - [`control`]: the local control protocol the desktop app and
//!   `openagents connect` speak to the host over a same-user socket.
//! - [`keys`]: where secret keys live, behind the [`keys::KeySource`] trait.
//!
//! iroh is transport only. An `EndpointId` proves which iroh key answered;
//! it never authorizes anything. Access is decided by the NIP-REACH
//! handshake and the host's grant check, exactly as over TCP.
//!
//! On Android, call `iroh::dns::install_android_jni_context` before binding
//! an endpoint.

pub mod code;
pub mod control;
pub mod endpoint;
pub mod enroll;
pub mod keys;
pub mod ledger;
pub mod reach;
pub mod stream;
pub mod wire;

pub use iroh;

use std::borrow::Cow;
use std::fmt;

/// The ALPN for enrollment: one `enroll.redeem` request and one reply.
/// Any iroh key may open it; the invitation's capability decides.
pub const ENROLL_ALPN: &[u8] = b"openagents/enroll/1";

/// The ALPN for the NIP-REACH direct channel. The handshake inside proves
/// both Nostr keys and checks the device's grant.
pub const REACH_ALPN: &[u8] = b"openagents/reach/1";

/// The iroh relay OpenAgents runs (issue #9968). No other relay is ever
/// configured by default.
pub const RELAY_URL: &str = "https://iroh.openagents.com";

/// How far ahead of the reader an `issued_at` may be, in seconds. The same
/// value as `coder_connect::protocol::CLOCK_SKEW`.
pub const CLOCK_SKEW: u64 = 60;

/// Typed failure causes. The wire spellings match the NIP-HOST codes where
/// one exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    /// The input is not in the expected shape.
    Malformed,
    /// A version this crate does not speak.
    UnsupportedVersion,
    /// A length or count is over its bound.
    Bounds,
    /// A time window has closed.
    Expired,
    /// A single-use item was already used by someone else, or the request
    /// lies outside its window.
    Forbidden,
    /// The item was cancelled or revoked.
    Revoked,
    /// A storage, socket, or peer failure.
    Unavailable,
    /// Too many items are held.
    LimitExceeded,
}

impl Code {
    /// Wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Malformed => "malformed",
            Self::UnsupportedVersion => "unsupported_version",
            Self::Bounds => "bounds",
            Self::Expired => "expired",
            Self::Forbidden => "forbidden",
            Self::Revoked => "revoked",
            Self::Unavailable => "unavailable",
            Self::LimitExceeded => "limit_exceeded",
        }
    }
}

/// An error with a typed code and a detail that never carries a secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub code: Code,
    pub detail: Cow<'static, str>,
}

impl Error {
    #[must_use]
    pub fn new(code: Code, detail: impl Into<Cow<'static, str>>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.detail)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn fail<T>(code: Code, detail: impl Into<Cow<'static, str>>) -> Result<T> {
    Err(Error::new(code, detail))
}

/// When the device's clock differs from the host's by more than
/// [`CLOCK_SKEW`], the difference in seconds, so the phone can say "Your
/// phone's clock is off" instead of reading a refusal as "expired".
#[must_use]
pub fn clock_warning(host_now: u64, device_now: u64) -> Option<u64> {
    let offset = host_now.abs_diff(device_now);
    (offset > CLOCK_SKEW).then_some(offset)
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    use fmt::Write;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

pub(crate) fn unhex32(value: &str) -> Result<[u8; 32]> {
    let bytes = value.as_bytes();
    if bytes.len() != 64 {
        return fail(Code::Malformed, "expected 64 lowercase hex characters");
    }
    let mut out = [0u8; 32];
    for (i, pair) in bytes.chunks_exact(2).enumerate() {
        let digit = |c: u8| match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            _ => None,
        };
        match (digit(pair[0]), digit(pair[1])) {
            (Some(hi), Some(lo)) => out[i] = hi << 4 | lo,
            _ => return fail(Code::Malformed, "expected 64 lowercase hex characters"),
        }
    }
    Ok(out)
}

/// Seconds since the Unix epoch, from the system clock.
#[must_use]
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_and_refuses_uppercase() {
        let bytes = [0xab; 32];
        assert_eq!(unhex32(&hex(&bytes)).unwrap(), bytes);
        assert!(unhex32(&"AB".repeat(32)).is_err());
        assert!(unhex32("ab").is_err());
    }

    #[test]
    fn clock_warning_names_offsets_over_the_skew() {
        assert_eq!(clock_warning(1_000, 1_000), None);
        assert_eq!(clock_warning(1_000, 940), None);
        assert_eq!(clock_warning(1_000, 939), Some(61));
        assert_eq!(clock_warning(1_000, 1_200), Some(200));
    }
}
