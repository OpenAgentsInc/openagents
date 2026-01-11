//! FROSTR Bridge - Connects agent-orchestrator to real threshold signing
//!
//! This module bridges the `ThresholdConfig` and `AgentIdentity` types to the
//! real FROSTR implementation for threshold-protected agent keys.
//!
//! # Example
//!
//! ```rust,ignore
//! use agent_orchestrator::integrations::{ThresholdConfig, AgentIdentity};
//! use agent_orchestrator::integrations::frostr_bridge::{generate_threshold_identity, FrostShareInfo};
//!
//! // Generate a new threshold-protected agent identity
//! let (identity, shares) = generate_threshold_identity("MyAgent", "codex-sonnet-4", 2, 3)?;
//!
//! assert!(identity.is_threshold_protected());
//! assert_eq!(shares.len(), 3);
//!
//! // Distribute shares to operators
//! for share in &shares {
//!     println!("Share {}: store securely", share.participant_id);
//! }
//! ```

#[cfg(feature = "frostr")]
use crate::integrations::advanced::AutonomyLevel;
use crate::integrations::advanced::{AgentIdentity, ThresholdConfig};
use serde::{Deserialize, Serialize};

/// Error type for FROSTR bridge operations
#[derive(Debug, thiserror::Error)]
pub enum FrostrBridgeError {
    #[error("Invalid threshold: k={0} must be <= n={1}")]
    InvalidThreshold(u32, u32),

    #[error("FROSTR keygen failed: {0}")]
    KeygenFailed(String),

    #[error("FROSTR is not available (feature not enabled)")]
    NotAvailable,
}

/// Information about a FROST share suitable for storage/distribution
///
/// This is a serializable representation of a FROST share that can be
/// persisted or transmitted to share holders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrostShareInfo {
    /// Participant ID (1-based)
    pub participant_id: u8,
    /// Threshold required to sign (k)
    pub threshold: u16,
    /// Total number of shares (n)
    pub total: u16,
    /// Hex-encoded group public key
    pub group_pubkey: String,
    /// Opaque share data (key package serialized) - kept opaque for security
    pub share_data: Vec<u8>,
}

/// Generate threshold key shares using real FROSTR
///
/// Creates a k-of-n threshold scheme where any k shares can sign,
/// but k-1 shares reveal nothing about the private key.
///
/// # Arguments
/// * `threshold` - Minimum shares needed to sign (k)
/// * `total` - Total shares to create (n)
///
/// # Returns
/// * `ThresholdConfig` with group public key and signer pubkeys
/// * Vector of `FrostShareInfo` for distribution to share holders
///
/// # Example
/// ```rust,ignore
/// let (config, shares) = generate_threshold_shares(2, 3)?;
/// assert!(config.is_valid());
/// assert_eq!(shares.len(), 3);
/// ```
#[cfg(feature = "frostr")]
pub fn generate_threshold_shares(
    threshold: u32,
    total: u32,
) -> Result<(ThresholdConfig, Vec<FrostShareInfo>), FrostrBridgeError> {
    use frostr::keygen::generate_key_shares;

    let frost_shares = generate_key_shares(threshold, total)
        .map_err(|e| FrostrBridgeError::KeygenFailed(e.to_string()))?;

    if frost_shares.is_empty() {
        return Err(FrostrBridgeError::KeygenFailed(
            "No shares generated".to_string(),
        ));
    }

    let group_verifying_key = frost_shares[0].public_key_package.verifying_key();
    let group_pubkey_bytes = group_verifying_key.serialize().map_err(|e| {
        FrostrBridgeError::KeygenFailed(format!("Failed to serialize group key: {:?}", e))
    })?;
    let group_pubkey = hex::encode(group_pubkey_bytes);

    let mut signer_pubkeys = Vec::with_capacity(total as usize);
    let mut share_infos = Vec::with_capacity(total as usize);

    for share in frost_shares {
        let identifier = share.key_package.identifier();
        let verifying_share = share
            .public_key_package
            .verifying_shares()
            .get(identifier)
            .expect("Participant should exist in public key package");

        let signer_pubkey_bytes = verifying_share.serialize().map_err(|e| {
            FrostrBridgeError::KeygenFailed(format!("Failed to serialize signer key: {:?}", e))
        })?;
        let signer_pubkey = hex::encode(signer_pubkey_bytes);
        signer_pubkeys.push(signer_pubkey);

        // SECURITY: Encrypt share_data before distributing to operators
        let share_data = postcard::to_stdvec(&share.key_package).unwrap_or_default();

        share_infos.push(FrostShareInfo {
            participant_id: share.participant_id,
            threshold: share.threshold,
            total: share.total,
            group_pubkey: group_pubkey.clone(),
            share_data,
        });
    }

    let config = ThresholdConfig::new(threshold, total)
        .with_signers(signer_pubkeys)
        .with_group_pubkey(group_pubkey);

    Ok((config, share_infos))
}

/// Generate threshold key shares (stub when FROSTR feature is disabled)
#[cfg(not(feature = "frostr"))]
pub fn generate_threshold_shares(
    _threshold: u32,
    _total: u32,
) -> Result<(ThresholdConfig, Vec<FrostShareInfo>), FrostrBridgeError> {
    Err(FrostrBridgeError::NotAvailable)
}

/// Generate a complete threshold-protected agent identity
///
/// This is a convenience function that combines threshold share generation
/// with agent identity creation.
///
/// # Arguments
/// * `name` - Agent name
/// * `model` - Model identifier (e.g., "codex-sonnet-4")
/// * `threshold` - Minimum shares needed to sign (k)
/// * `total` - Total shares to create (n)
///
/// # Returns
/// * `AgentIdentity` configured with threshold protection
/// * Vector of `FrostShareInfo` for distribution
#[cfg(feature = "frostr")]
pub fn generate_threshold_identity(
    name: &str,
    model: &str,
    threshold: u32,
    total: u32,
) -> Result<(AgentIdentity, Vec<FrostShareInfo>), FrostrBridgeError> {
    let (config, shares) = generate_threshold_shares(threshold, total)?;

    let identity = AgentIdentity::new(&config.group_pubkey, name, model)
        .with_threshold(config)
        .with_autonomy(AutonomyLevel::default());

    Ok((identity, shares))
}

/// Generate a complete threshold-protected agent identity (stub)
#[cfg(not(feature = "frostr"))]
pub fn generate_threshold_identity(
    _name: &str,
    _model: &str,
    _threshold: u32,
    _total: u32,
) -> Result<(AgentIdentity, Vec<FrostShareInfo>), FrostrBridgeError> {
    Err(FrostrBridgeError::NotAvailable)
}

/// Check if FROSTR support is available
pub fn is_frostr_available() -> bool {
    cfg!(feature = "frostr")
}

#[cfg(all(test, feature = "frostr"))]
mod tests {
    use super::*;

    #[test]
    fn test_generate_threshold_shares_2_of_3() {
        let (config, shares) = generate_threshold_shares(2, 3).expect("should generate shares");

        assert_eq!(config.threshold, 2);
        assert_eq!(config.total_signers, 3);
        assert_eq!(config.signer_pubkeys.len(), 3);
        assert!(!config.group_pubkey.is_empty());
        assert!(config.is_valid());

        assert_eq!(shares.len(), 3);
        for (i, share) in shares.iter().enumerate() {
            assert_eq!(share.participant_id, (i + 1) as u8);
            assert_eq!(share.threshold, 2);
            assert_eq!(share.total, 3);
            assert_eq!(share.group_pubkey, config.group_pubkey);
            assert!(!share.share_data.is_empty());
        }
    }

    #[test]
    fn test_generate_threshold_shares_3_of_5() {
        let (config, shares) = generate_threshold_shares(3, 5).expect("should generate shares");

        assert_eq!(config.threshold, 3);
        assert_eq!(config.total_signers, 5);
        assert_eq!(shares.len(), 5);
        assert!(config.is_valid());
    }

    #[test]
    fn test_generate_threshold_identity() {
        let (identity, shares) = generate_threshold_identity("TestAgent", "codex-sonnet-4", 2, 3)
            .expect("should generate identity");

        assert_eq!(identity.name, "TestAgent");
        assert_eq!(identity.model, "codex-sonnet-4");
        assert!(identity.is_threshold_protected());
        assert_eq!(shares.len(), 3);
    }

    #[test]
    fn test_invalid_threshold() {
        let result = generate_threshold_shares(4, 3);
        assert!(result.is_err());
    }

    #[test]
    fn test_group_pubkey_consistency() {
        let (config, shares) = generate_threshold_shares(2, 3).expect("should generate shares");

        for share in &shares {
            assert_eq!(share.group_pubkey, config.group_pubkey);
        }
    }
}

#[cfg(all(test, not(feature = "frostr")))]
mod tests_no_frostr {
    use super::*;

    #[test]
    fn test_frostr_not_available() {
        assert!(!is_frostr_available());

        let result = generate_threshold_shares(2, 3);
        assert!(matches!(result, Err(FrostrBridgeError::NotAvailable)));
    }
}
