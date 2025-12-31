//! FROSTR - Native Rust implementation of FROST threshold signatures for Nostr
//!
//! FROSTR (FROST for Nostr) enables k-of-n threshold Schnorr signatures where:
//! - The private key is never reconstructed
//! - Signatures are indistinguishable from single-key signatures
//! - Compromised shares can be replaced without changing identity
//! - Coordination happens via encrypted Nostr events
//!
//! This crate provides the cryptographic foundation for NIP-SA Sovereign Agents,
//! enabling agent identities protected by threshold signatures where operators
//! cannot extract the full private key.
//!
//! ## Modules
//!
//! - [`keygen`] - Key generation and Shamir Secret Sharing
//! - [`signing`] - FROST signing protocol
//! - [`ecdh`] - Threshold ECDH for encryption
//! - [`credential`] - Bech32 encoding for group and share credentials
//! - [`bifrost`] - Bifrost protocol for peer coordination via Nostr
//!
//! ## Example
//!
//! ```
//! use frostr::keygen::generate_key_shares;
//!
//! // Generate 2-of-3 threshold key shares
//! let shares = generate_key_shares(2, 3).unwrap();
//! assert_eq!(shares.len(), 3);
//!
//! // Any 2 shares can sign, but no single share can
//! // Signing happens via the Bifrost protocol
//! ```

pub mod bifrost;
pub mod credential;
pub mod ecdh;
pub mod keygen;
pub mod signing;

// Re-export commonly used types
pub use credential::{GroupCredential, ShareCredential};
pub use keygen::{FrostShare, Share, generate_key_shares, reshare_frost_shares};

// Re-export frost-secp256k1 types for consumers that need direct access
pub use frost_secp256k1 as frost;

/// FROSTR protocol version
pub const VERSION: &str = "0.1.0";

/// Error types for FROSTR operations
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid threshold: k={0} must be <= n={1}")]
    InvalidThreshold(u32, u32),

    #[error("Invalid share count: need {need} shares, got {got}")]
    InvalidShareCount { need: u32, got: u32 },

    #[error("Cryptographic error: {0}")]
    Crypto(String),

    #[error("FROST error: {0}")]
    FrostError(String),

    #[error("Encoding error: {0}")]
    Encoding(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Signing error: {0}")]
    Signing(String),

    #[error("Timeout waiting for threshold peers")]
    Timeout,

    #[error("ECDH error: {0}")]
    EcdhError(String),

    #[error("Invalid public key: {0}")]
    InvalidPublicKey(String),

    #[error("Invalid secret key: {0}")]
    InvalidSecretKey(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
