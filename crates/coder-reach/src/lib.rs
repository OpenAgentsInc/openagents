//! NIP-REACH: find the hosts an owner runs and reach them over a proven route.
//!
//! The crate implements the owner host directory, host presence with bounded
//! telemetry, reachability hints, the direct-channel handshake and frame
//! format, and the placement rule. Every record here is a private `3188`
//! artifact; none grants access. Grant checks go through
//! [`channel::GrantCheck`] so the host can wire its own grant store.
//!
//! Read `nips/openagents/NIP-REACH.md` before changing a schema, a bound, or
//! the handshake transcript.

pub mod artifact;
pub mod channel;
pub mod directory;
pub mod hints;
pub mod placement;
pub mod presence;
pub mod split;

use std::fmt;
use std::str::FromStr;

use secp256k1::{Secp256k1, SecretKey, XOnlyPublicKey, rand::RngCore};

/// The NIP-REACH protocol version this crate speaks.
pub const PROTOCOL_VERSION: u32 = 1;

/// Typed refusal causes. The first group matches the shared contract codes;
/// `replayed` is a NIP-REACH cause for a reused handshake nonce.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Malformed,
    UnsupportedVersion,
    UnsupportedFeature,
    NotAdmitted,
    Unavailable,
    IdentityMismatch,
    Incompatible,
    Revoked,
    Stale,
    LimitExceeded,
    Conflict,
    Replayed,
}

impl Refusal {
    /// Wire spelling of the refusal code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Malformed => "malformed",
            Self::UnsupportedVersion => "unsupported_version",
            Self::UnsupportedFeature => "unsupported_feature",
            Self::NotAdmitted => "not_admitted",
            Self::Unavailable => "unavailable",
            Self::IdentityMismatch => "identity_mismatch",
            Self::Incompatible => "incompatible",
            Self::Revoked => "revoked",
            Self::Stale => "stale",
            Self::LimitExceeded => "limit_exceeded",
            Self::Conflict => "conflict",
            Self::Replayed => "replayed",
        }
    }

    /// Parse a wire refusal code.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        [
            Self::Malformed,
            Self::UnsupportedVersion,
            Self::UnsupportedFeature,
            Self::NotAdmitted,
            Self::Unavailable,
            Self::IdentityMismatch,
            Self::Incompatible,
            Self::Revoked,
            Self::Stale,
            Self::LimitExceeded,
            Self::Conflict,
            Self::Replayed,
        ]
        .into_iter()
        .find(|code| code.as_str() == value)
    }
}

/// A refusal with a fixed, content-free detail string safe to log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub code: Refusal,
    pub detail: &'static str,
}

impl Error {
    #[must_use]
    pub const fn new(code: Refusal, detail: &'static str) -> Self {
        Self { code, detail }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.detail)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn fail<T>(code: Refusal, detail: &'static str) -> Result<T> {
    Err(Error::new(code, detail))
}

/// Lowercase hex x-only public key for a secret key.
#[must_use]
pub fn pubkey(secret: &SecretKey) -> String {
    secret.x_only_public_key(&Secp256k1::new()).0.to_string()
}

/// Parse a lowercase 64-hex x-only public key.
///
/// # Errors
/// Refuses anything other than a valid lowercase x-only key.
pub fn parse_pubkey(value: &str) -> Result<XOnlyPublicKey> {
    random_id(value)?;
    XOnlyPublicKey::from_str(value).map_err(|_| Error::new(Refusal::Malformed, "public key"))
}

/// Check a 32-byte lowercase hex identifier.
///
/// # Errors
/// Refuses other lengths, uppercase, or non-hex characters.
pub fn random_id(value: &str) -> Result<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        Ok(())
    } else {
        fail(Refusal::Malformed, "identifier must be 64 lowercase hex")
    }
}

/// 32 fresh random bytes.
#[must_use]
pub fn random_bytes() -> [u8; 32] {
    let mut bytes = [0; 32];
    secp256k1::rand::rng().fill_bytes(&mut bytes);
    bytes
}

/// A fresh random 64-hex identifier, for mailboxes, nonces, and IDs.
#[must_use]
pub fn new_id() -> String {
    hex(&random_bytes())
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn unhex32(value: &str) -> Result<[u8; 32]> {
    random_id(value)?;
    let mut out = [0; 32];
    for (index, chunk) in value.as_bytes().chunks(2).enumerate() {
        let text = std::str::from_utf8(chunk).map_err(|_| Error::new(Refusal::Malformed, "hex"))?;
        out[index] =
            u8::from_str_radix(text, 16).map_err(|_| Error::new(Refusal::Malformed, "hex"))?;
    }
    Ok(out)
}

/// Check a feature list: every entry must be supported. The initial supported
/// list is empty, so any required feature refuses.
pub(crate) fn requires(list: &[String]) -> Result<()> {
    if list.is_empty() {
        Ok(())
    } else {
        fail(Refusal::UnsupportedFeature, "unsupported required feature")
    }
}

/// Short display text: nonempty, bounded, and free of control characters.
pub(crate) fn label(value: &str, max: usize) -> Result<()> {
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        fail(Refusal::Malformed, "label must be short printable text")
    } else {
        Ok(())
    }
}

/// Unqualified slug grammar from the shared contracts.
pub(crate) fn slug(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_' || *b == b'-')
}

/// Relay URL for a directory or relay hint: exact `wss`, no credentials, query,
/// or fragment. `ws` is accepted only for a loopback test relay.
pub(crate) fn relay_url(value: &str, allow_loopback_ws: bool) -> Result<()> {
    let url = url::Url::parse(value).map_err(|_| Error::new(Refusal::Malformed, "relay URL"))?;
    let loopback = hints::host_is_loopback(url.host());
    if value.len() > 512
        || !value.is_ascii()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.host().is_none()
        || !(url.scheme() == "wss" || (allow_loopback_ws && loopback && url.scheme() == "ws"))
    {
        return fail(
            Refusal::Malformed,
            "relay must be a credential-free wss URL without query or fragment",
        );
    }
    Ok(())
}
