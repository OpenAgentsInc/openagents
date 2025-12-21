//! Credential encoding for FROSTR
//!
//! This module handles bech32 encoding of group and share credentials.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// Group credential (bfgroup1...)
///
/// Contains the threshold configuration and group public key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupCredential {
    /// Threshold (k)
    pub threshold: u32,
    /// Total shares (n)
    pub total: u32,
    /// Group public key (32 bytes x-only)
    pub group_pk: [u8; 32],
}

impl GroupCredential {
    /// Encode as bech32 string
    pub fn to_bech32(&self) -> Result<String> {
        // TODO: Implement bech32 encoding with "bfgroup" prefix
        todo!("Implement bech32 encoding")
    }

    /// Decode from bech32 string
    pub fn from_bech32(s: &str) -> Result<Self> {
        // TODO: Implement bech32 decoding
        todo!("Implement bech32 decoding")
    }
}

/// Share credential (bfshare1...)
///
/// Contains an individual secret share.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareCredential {
    /// Share index (1..n)
    pub index: u32,
    /// Secret share (32 bytes)
    pub secret: [u8; 32],
    /// Group public key (must match GroupCredential)
    pub group_pk: [u8; 32],
}

impl ShareCredential {
    /// Encode as bech32 string
    pub fn to_bech32(&self) -> Result<String> {
        // TODO: Implement bech32 encoding with "bfshare" prefix
        todo!("Implement bech32 encoding")
    }

    /// Decode from bech32 string
    pub fn from_bech32(s: &str) -> Result<Self> {
        // TODO: Implement bech32 decoding
        todo!("Implement bech32 decoding")
    }
}
