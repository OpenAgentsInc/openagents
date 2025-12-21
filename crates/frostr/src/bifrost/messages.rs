//! Bifrost protocol message types

use serde::{Deserialize, Serialize};
use serde_with::serde_as;

/// Bifrost protocol message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BifrostMessage {
    /// Signing request
    #[serde(rename = "/sign/req")]
    SignRequest(SignRequest),

    /// Signing response
    #[serde(rename = "/sign/res")]
    SignResponse(SignResponse),

    /// Signing result
    #[serde(rename = "/sign/ret")]
    SignResult(SignResult),

    /// Signing error
    #[serde(rename = "/sign/err")]
    SignError(SignError),

    /// ECDH request
    #[serde(rename = "/ecdh/req")]
    EcdhRequest(EcdhRequest),

    /// ECDH response
    #[serde(rename = "/ecdh/res")]
    EcdhResponse(EcdhResponse),
}

/// Request to sign an event hash
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRequest {
    /// Event hash to sign (32 bytes)
    pub event_hash: [u8; 32],
    /// Nonce commitment (33 bytes compressed point)
    #[serde_as(as = "[_; 33]")]
    pub nonce_commitment: [u8; 33],
}

/// Partial signature from a peer
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignResponse {
    /// Partial signature (32 bytes)
    pub partial_sig: [u8; 32],
    /// Nonce share (33 bytes compressed point)
    #[serde_as(as = "[_; 33]")]
    pub nonce_share: [u8; 33],
}

/// Final aggregated signature
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignResult {
    /// Final Schnorr signature (64 bytes)
    #[serde_as(as = "[_; 64]")]
    pub signature: [u8; 64],
}

/// Signing error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignError {
    /// Error message
    pub reason: String,
}

/// Request for threshold ECDH
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcdhRequest {
    /// Target public key (32 bytes x-only)
    pub target_pubkey: [u8; 32],
}

/// Partial ECDH response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcdhResponse {
    /// Partial ECDH result (32 bytes)
    pub partial_ecdh: [u8; 32],
}
