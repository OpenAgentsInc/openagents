//! Refusal codes from the shared contract vocabulary.
//!
//! Human detail is data for a caller. It is not rendered as a terminal
//! control sequence and it does not select a code.

use std::fmt;

/// A shared refusal cause. Profiles may add versioned causes beside these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalCode {
    /// The body is not the required shape.
    Malformed,
    /// `v` is not a schema this layer implements.
    UnsupportedVersion,
    /// A feature, keyword, or semantic field has no defined behavior.
    UnsupportedFeature,
    /// The caller is not admitted to the operation.
    NotAdmitted,
    /// The operation cannot be performed now.
    Unavailable,
    /// Referenced bytes are not in the supplied store.
    ContentUnavailable,
    /// A digest, identifier, or signature does not match the bytes.
    IdentityMismatch,
    /// Two required constraints cannot be satisfied together.
    Incompatible,
    /// A referenced record was revoked.
    Revoked,
    /// A record is outside its admitted freshness window.
    Stale,
    /// A required bound has no host mechanism that can enforce it.
    CannotEnforce,
    /// A declared ceiling was exceeded.
    LimitExceeded,
    /// The same idempotency key arrived with a different body.
    IdempotencyConflict,
    /// Two records claim the same identity with different bytes.
    Conflict,
}

impl RefusalCode {
    /// The stable machine name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Malformed => "malformed",
            Self::UnsupportedVersion => "unsupported_version",
            Self::UnsupportedFeature => "unsupported_feature",
            Self::NotAdmitted => "not_admitted",
            Self::Unavailable => "unavailable",
            Self::ContentUnavailable => "content_unavailable",
            Self::IdentityMismatch => "identity_mismatch",
            Self::Incompatible => "incompatible",
            Self::Revoked => "revoked",
            Self::Stale => "stale",
            Self::CannotEnforce => "cannot_enforce",
            Self::LimitExceeded => "limit_exceeded",
            Self::IdempotencyConflict => "idempotency_conflict",
            Self::Conflict => "conflict",
        }
    }
}

impl fmt::Display for RefusalCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A contract check failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractError {
    /// The typed cause.
    pub code: RefusalCode,
    /// A short locator, such as a field path. Not a message to execute.
    pub detail: String,
}

impl ContractError {
    /// A refusal at `detail`.
    #[must_use]
    pub fn new(code: RefusalCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for ContractError {}
