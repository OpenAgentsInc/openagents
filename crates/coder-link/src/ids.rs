//! Identifiers the supervisor and registry use.
use std::fmt;

/// The longest host key or credential identifier the registry accepts.
const MAX_ID: usize = 128;

/// An identifier that is empty, too long, or contains a byte outside visible
/// ASCII.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidId;

impl fmt::Display for InvalidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "an identifier must be 1 to {MAX_ID} bytes of visible ASCII"
        )
    }
}

impl std::error::Error for InvalidId {}

fn check(value: &str) -> Result<(), InvalidId> {
    if value.is_empty() || value.len() > MAX_ID || !value.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(InvalidId);
    }
    Ok(())
}

/// The stable identity of one host, such as its public key in hex.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostKey(String);

impl HostKey {
    /// Validates and wraps a host key.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidId> {
        let value = value.into();
        check(&value)?;
        Ok(Self(value))
    }

    /// Returns the key's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HostKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A name for a credential that a route depends on, such as a grant ID.
///
/// It names the credential; it never holds secret material.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CredentialId(String);

impl CredentialId {
    /// Validates and wraps a credential identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidId> {
        let value = value.into();
        check(&value)?;
        Ok(Self(value))
    }

    /// Returns the identifier's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One connection or probe attempt. A supervisor never reuses an ID, so a
/// report for a superseded attempt is recognized and ignored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttemptId(pub u64);

/// An established connection. It carries the ID of the attempt that opened it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConnectionId(pub u64);
