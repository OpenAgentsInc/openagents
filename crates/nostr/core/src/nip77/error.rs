use std::io;
use thiserror::Error;

/// NIP-77 error types
#[derive(Debug, Error)]
pub enum Nip77Error {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid protocol version: {0}")]
    InvalidProtocolVersion(u8),

    #[error("Invalid mode: {0}")]
    InvalidMode(u64),

    #[error("Invalid hex encoding: {0}")]
    InvalidHex(String),

    #[error("Varint decode error: {0}")]
    VarintDecode(String),

    #[error("Varint encode error: {0}")]
    VarintEncode(String),

    #[error("Invalid bound: {0}")]
    InvalidBound(String),

    #[error("Invalid range: {0}")]
    InvalidRange(String),

    #[error("Invalid fingerprint length: expected 16, got {0}")]
    InvalidFingerprintLength(usize),

    #[error("Invalid ID length: expected 32, got {0}")]
    InvalidIdLength(usize),
}

pub type Result<T> = std::result::Result<T, Nip77Error>;
