//! FROST type serialization for Bifrost protocol
//!
//! This module provides serialization/deserialization utilities for FROST cryptographic
//! types that need to be transmitted over the Bifrost protocol. The frost-secp256k1 crate
//! types don't have serde support, so we manually serialize to/from byte arrays.
//!
//! # Wire Format
//!
//! - **SigningCommitments**: 66 bytes (hiding: 33 bytes + binding: 33 bytes)
//! - **SignatureShare**: 32 bytes (scalar)
//! - **Identifier**: 32 bytes (scalar) - uses frost-secp256k1's native serialization

use crate::{Error, Result};
use frost_secp256k1::{Identifier, round1::SigningCommitments, round2::SignatureShare};

/// Commitment wire format: hiding nonce (33 bytes) + binding nonce (33 bytes)
pub const COMMITMENT_SIZE: usize = 66;

/// Signature share wire format: 32-byte scalar
pub const SIG_SHARE_SIZE: usize = 32;

/// Identifier wire format: 32-byte scalar (frost-secp256k1 native format)
pub const IDENTIFIER_SIZE: usize = 32;

/// Serialize SigningCommitments to 66 bytes
///
/// Format: hiding_nonce (33 bytes) || binding_nonce (33 bytes)
///
/// # Wire Format
///
/// The hiding and binding nonces are SEC1 compressed elliptic curve points:
/// - First byte: 0x02 (even Y) or 0x03 (odd Y)
/// - Next 32 bytes: X coordinate in big-endian
///
/// # Panics
///
/// Panics if the FROST commitment serialization fails (should not happen with valid commitments)
pub fn serialize_commitments(commitments: &SigningCommitments) -> [u8; COMMITMENT_SIZE] {
    let mut bytes = [0u8; COMMITMENT_SIZE];

    // Get the hiding and binding nonces as compressed points
    // These serialize() calls return Result<Vec<u8>, Error> but should always succeed
    // for valid commitments created by round1_commit()
    let hiding_bytes = commitments
        .hiding()
        .serialize()
        .expect("Valid commitment should serialize");
    let binding_bytes = commitments
        .binding()
        .serialize()
        .expect("Valid commitment should serialize");

    // Copy into fixed-size array
    bytes[..33].copy_from_slice(&hiding_bytes);
    bytes[33..66].copy_from_slice(&binding_bytes);

    bytes
}

/// Deserialize SigningCommitments from 66 bytes
///
/// # Errors
///
/// Returns error if:
/// - Bytes are not valid SEC1 compressed points
/// - Points are not on the secp256k1 curve
pub fn deserialize_commitments(bytes: &[u8; COMMITMENT_SIZE]) -> Result<SigningCommitments> {
    use frost_secp256k1::round1::NonceCommitment;

    // Split into hiding and binding portions
    let hiding_slice = &bytes[..33];
    let binding_slice = &bytes[33..66];

    // Deserialize each nonce commitment
    let hiding = NonceCommitment::deserialize(hiding_slice)
        .map_err(|e| Error::Encoding(format!("Invalid hiding commitment: {:?}", e)))?;
    let binding = NonceCommitment::deserialize(binding_slice)
        .map_err(|e| Error::Encoding(format!("Invalid binding commitment: {:?}", e)))?;

    // Construct SigningCommitments (this doesn't return Result, just Self)
    Ok(SigningCommitments::new(hiding, binding))
}

/// Serialize SignatureShare to 32 bytes
///
/// Format: scalar in big-endian
pub fn serialize_sig_share(share: &SignatureShare) -> [u8; SIG_SHARE_SIZE] {
    let vec = share.serialize();
    let mut bytes = [0u8; SIG_SHARE_SIZE];
    bytes.copy_from_slice(&vec);
    bytes
}

/// Deserialize SignatureShare from 32 bytes
///
/// # Errors
///
/// Returns error if bytes don't represent a valid secp256k1 scalar
pub fn deserialize_sig_share(bytes: &[u8; SIG_SHARE_SIZE]) -> Result<SignatureShare> {
    SignatureShare::deserialize(bytes)
        .map_err(|e| Error::Encoding(format!("Invalid signature share: {:?}", e)))
}

/// Serialize Identifier to 32 bytes (scalar)
pub fn serialize_identifier(id: &Identifier) -> [u8; IDENTIFIER_SIZE] {
    let vec = id.serialize();
    let mut bytes = [0u8; IDENTIFIER_SIZE];
    bytes.copy_from_slice(&vec);
    bytes
}

/// Deserialize Identifier from 32 bytes (scalar)
///
/// # Errors
///
/// Returns error if the identifier is zero (invalid for FROST)
pub fn deserialize_identifier(bytes: &[u8; IDENTIFIER_SIZE]) -> Result<Identifier> {
    Identifier::deserialize(bytes)
        .map_err(|e| Error::Encoding(format!("Invalid identifier: {:?}", e)))
}

/// Bundle of commitment and identifier for wire transmission
#[derive(Debug, Clone)]
pub struct CommitmentBundle {
    /// Participant identifier
    pub identifier: Identifier,
    /// Serialized commitments (66 bytes)
    pub commitment_bytes: [u8; COMMITMENT_SIZE],
}

impl CommitmentBundle {
    /// Create from FROST types
    pub fn new(identifier: Identifier, commitments: &SigningCommitments) -> Self {
        Self {
            identifier,
            commitment_bytes: serialize_commitments(commitments),
        }
    }

    /// Serialize to wire format: id (32 bytes) + commitment (66 bytes) = 98 bytes
    pub fn to_bytes(&self) -> [u8; 98] {
        let mut bytes = [0u8; 98];
        bytes[..32].copy_from_slice(&serialize_identifier(&self.identifier));
        bytes[32..98].copy_from_slice(&self.commitment_bytes);
        bytes
    }

    /// Deserialize from wire format
    pub fn from_bytes(bytes: &[u8; 98]) -> Result<Self> {
        let id_bytes: [u8; 32] = bytes[..32]
            .try_into()
            .map_err(|_| Error::Encoding("Invalid bundle length".to_string()))?;
        let commitment_bytes: [u8; 66] = bytes[32..98]
            .try_into()
            .map_err(|_| Error::Encoding("Invalid bundle length".to_string()))?;

        let identifier = deserialize_identifier(&id_bytes)?;

        Ok(Self {
            identifier,
            commitment_bytes,
        })
    }

    /// Get the deserialized commitments
    pub fn commitments(&self) -> Result<SigningCommitments> {
        deserialize_commitments(&self.commitment_bytes)
    }
}

/// Bundle of signature share and identifier for wire transmission
#[derive(Debug, Clone)]
pub struct SignatureBundle {
    /// Participant identifier
    pub identifier: Identifier,
    /// Serialized signature share (32 bytes)
    pub sig_share_bytes: [u8; SIG_SHARE_SIZE],
}

impl SignatureBundle {
    /// Create from FROST types
    pub fn new(identifier: Identifier, sig_share: &SignatureShare) -> Self {
        Self {
            identifier,
            sig_share_bytes: serialize_sig_share(sig_share),
        }
    }

    /// Serialize to wire format: id (32 bytes) + sig (32 bytes) = 64 bytes
    pub fn to_bytes(&self) -> [u8; 64] {
        let mut bytes = [0u8; 64];
        bytes[..32].copy_from_slice(&serialize_identifier(&self.identifier));
        bytes[32..64].copy_from_slice(&self.sig_share_bytes);
        bytes
    }

    /// Deserialize from wire format
    pub fn from_bytes(bytes: &[u8; 64]) -> Result<Self> {
        let id_bytes: [u8; 32] = bytes[..32]
            .try_into()
            .map_err(|_| Error::Encoding("Invalid bundle length".to_string()))?;
        let sig_share_bytes: [u8; 32] = bytes[32..64]
            .try_into()
            .map_err(|_| Error::Encoding("Invalid bundle length".to_string()))?;

        let identifier = deserialize_identifier(&id_bytes)?;

        Ok(Self {
            identifier,
            sig_share_bytes,
        })
    }

    /// Get the deserialized signature share
    pub fn sig_share(&self) -> Result<SignatureShare> {
        deserialize_sig_share(&self.sig_share_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keygen::generate_key_shares;
    use crate::signing::round1_commit;

    #[test]
    fn test_commitment_serialization_round_trip() {
        // Generate shares and create commitments
        let shares = generate_key_shares(2, 3).unwrap();
        let (_, commitments) = round1_commit(&shares[0]);

        // Serialize
        let bytes = serialize_commitments(&commitments);
        assert_eq!(bytes.len(), COMMITMENT_SIZE);

        // Deserialize
        let deserialized = deserialize_commitments(&bytes).unwrap();

        // Re-serialize and compare bytes (can't directly compare commitments)
        let bytes2 = serialize_commitments(&deserialized);
        assert_eq!(bytes, bytes2);
    }

    #[test]
    fn test_identifier_serialization_round_trip() {
        let shares = generate_key_shares(2, 3).unwrap();
        let identifier = *shares[0].key_package.identifier();

        // Serialize
        let bytes = serialize_identifier(&identifier);
        assert_eq!(bytes.len(), IDENTIFIER_SIZE);

        // Deserialize
        let deserialized = deserialize_identifier(&bytes).unwrap();

        // Compare by re-serializing
        let bytes2 = serialize_identifier(&deserialized);
        assert_eq!(bytes, bytes2);
    }

    #[test]
    fn test_commitment_bundle_round_trip() {
        let shares = generate_key_shares(2, 3).unwrap();
        let (_, commitments) = round1_commit(&shares[0]);
        let identifier = *shares[0].key_package.identifier();

        // Create bundle
        let bundle = CommitmentBundle::new(identifier, &commitments);

        // Serialize to bytes
        let bytes = bundle.to_bytes();
        assert_eq!(bytes.len(), 98);

        // Deserialize
        let deserialized = CommitmentBundle::from_bytes(&bytes).unwrap();

        // Verify contents match
        assert_eq!(bundle.commitment_bytes, deserialized.commitment_bytes);
    }

    #[test]
    fn test_signature_share_serialization() {
        use crate::signing::{round1_commit, round2_sign};
        use frost_secp256k1::SigningPackage;
        use std::collections::BTreeMap;

        let shares = generate_key_shares(2, 3).unwrap();
        let message = b"test message";

        // Generate commitments
        let (nonces0, commitments0) = round1_commit(&shares[0]);
        let (_, commitments1) = round1_commit(&shares[1]);

        // Create signing package
        let mut signing_commitments = BTreeMap::new();
        signing_commitments.insert(*shares[0].key_package.identifier(), commitments0);
        signing_commitments.insert(*shares[1].key_package.identifier(), commitments1);
        let signing_package = SigningPackage::new(signing_commitments, message);

        // Generate signature share
        let sig_share = round2_sign(&shares[0], &nonces0, &signing_package).unwrap();

        // Serialize
        let bytes = serialize_sig_share(&sig_share);
        assert_eq!(bytes.len(), SIG_SHARE_SIZE);

        // Deserialize
        let deserialized = deserialize_sig_share(&bytes).unwrap();

        // Verify by re-serializing
        let bytes2 = serialize_sig_share(&deserialized);
        assert_eq!(bytes, bytes2);
    }

    #[test]
    fn test_signature_bundle_round_trip() {
        use crate::signing::{round1_commit, round2_sign};
        use frost_secp256k1::SigningPackage;
        use std::collections::BTreeMap;

        let shares = generate_key_shares(2, 3).unwrap();
        let message = b"test message";

        // Generate commitments
        let (nonces0, commitments0) = round1_commit(&shares[0]);
        let (_, commitments1) = round1_commit(&shares[1]);

        // Create signing package
        let mut signing_commitments = BTreeMap::new();
        signing_commitments.insert(*shares[0].key_package.identifier(), commitments0);
        signing_commitments.insert(*shares[1].key_package.identifier(), commitments1);
        let signing_package = SigningPackage::new(signing_commitments, message);

        // Generate signature share
        let sig_share = round2_sign(&shares[0], &nonces0, &signing_package).unwrap();
        let identifier = *shares[0].key_package.identifier();

        // Create bundle
        let bundle = SignatureBundle::new(identifier, &sig_share);

        // Serialize
        let bytes = bundle.to_bytes();
        assert_eq!(bytes.len(), 64);

        // Deserialize
        let deserialized = SignatureBundle::from_bytes(&bytes).unwrap();

        // Verify
        assert_eq!(bundle.sig_share_bytes, deserialized.sig_share_bytes);
    }

    #[test]
    fn test_invalid_identifier_zero() {
        // Zero identifier is invalid in FROST
        let zero_bytes = [0u8; IDENTIFIER_SIZE];
        let result = deserialize_identifier(&zero_bytes);
        assert!(result.is_err());
    }
}
