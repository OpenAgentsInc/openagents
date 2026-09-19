use std::fmt;

/// A protocol-domain validation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    EmptyTag,
    InvalidHex {
        field: &'static str,
        expected_bytes: usize,
    },
    InvalidPublicKey,
    EventIdMismatch {
        expected: String,
        actual: String,
    },
    InvalidSignature,
    ExpiredEvent {
        expiration: u64,
        now: u64,
    },
    FutureTimestamp {
        created_at: u64,
        latest_allowed: u64,
    },
    InvalidEvent(String),
    InvalidFilter(String),
    InvalidReplacementAddress(String),
    ReplacementAddressMismatch,
    NotReplaceable,
    NotDeletionRequest,
    Serialization(String),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTag => f.write_str("tags must contain at least a name"),
            Self::InvalidHex {
                field,
                expected_bytes,
            } => write!(
                f,
                "{field} must be {} lowercase hexadecimal characters",
                expected_bytes * 2
            ),
            Self::InvalidPublicKey => f.write_str("pubkey is not a valid x-only secp256k1 key"),
            Self::EventIdMismatch { expected, actual } => {
                write!(
                    f,
                    "event id mismatch: expected {expected}, received {actual}"
                )
            }
            Self::InvalidSignature => f.write_str("invalid Schnorr signature"),
            Self::ExpiredEvent { expiration, now } => {
                write!(f, "event expired at {expiration}; current time is {now}")
            }
            Self::FutureTimestamp {
                created_at,
                latest_allowed,
            } => write!(
                f,
                "event timestamp {created_at} is later than allowed maximum {latest_allowed}"
            ),
            Self::InvalidEvent(reason) => write!(f, "invalid event: {reason}"),
            Self::InvalidFilter(reason) => write!(f, "invalid NIP-01 filter: {reason}"),
            Self::InvalidReplacementAddress(value) => {
                write!(f, "invalid replacement address: {value:?}")
            }
            Self::ReplacementAddressMismatch => {
                f.write_str("events do not have the same replacement address")
            }
            Self::NotReplaceable => f.write_str("event is not replaceable or addressable"),
            Self::NotDeletionRequest => f.write_str("event is not a kind 5 deletion request"),
            Self::Serialization(reason) => write!(f, "canonical serialization failed: {reason}"),
        }
    }
}

impl std::error::Error for DomainError {}
