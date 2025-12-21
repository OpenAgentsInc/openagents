//! Threshold ECDH for encryption
//!
//! This module will implement threshold Elliptic Curve Diffie-Hellman for
//! decrypting messages encrypted to the threshold group public key.
//!
//! # Status: Not Yet Implemented
//!
//! FROST shares cannot be directly used for threshold ECDH using standard SSS
//! reconstruction because FROST's dealer uses a different polynomial generation
//! scheme than our SSS implementation.
//!
//! Proper implementation requires either:
//! 1. Multiplicative threshold ECDH using FROST's scalar field arithmetic
//! 2. A custom ECDH-specific key generation that's compatible with both FROST and SSS
//! 3. Using FROST's internal reconstruction (not currently exposed in the API)
//!
//! For now, this module provides the API scaffold but returns an error.

use crate::{keygen::FrostShare, Error, Result};
use secp256k1::{PublicKey};

/// Threshold ECDH share
#[derive(Debug, Clone)]
pub struct EcdhShare {
    /// Participant identifier (1-based)
    pub id: u8,
    /// The FROST share containing the secret
    pub frost_share: FrostShare,
}

/// Perform threshold ECDH using k-of-n shares
///
/// # Not Yet Implemented
///
/// This function is not yet implemented because FROST shares use a different
/// polynomial structure than standard Shamir Secret Sharing. Proper threshold
/// ECDH requires implementing multiplicative threshold ECDH using FROST's
/// scalar field arithmetic, which is complex and beyond the scope of the
/// initial implementation.
///
/// # Future Work
///
/// A full implementation would:
/// 1. Use multiplicative threshold ECDH to avoid reconstructing the full key
/// 2. Implement Lagrange interpolation in the exponent
/// 3. Or integrate with FROST's internal scalar reconstruction (when API available)
///
/// For now, this returns an error. Applications needing ECDH can use the
/// signing protocol combined with custom key derivation as a workaround.
pub fn threshold_ecdh(
    _shares: &[(u8, &FrostShare)],
    _peer_pubkey: &PublicKey,
) -> Result<[u8; 32]> {
    Err(Error::Protocol(
        "Threshold ECDH not yet implemented. FROST shares require \
         multiplicative threshold ECDH which is not currently supported. \
         See module documentation for details.".into()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keygen::generate_key_shares;
    use secp256k1::SECP256K1;
    use secp256k1::SecretKey;

    #[test]
    fn test_threshold_ecdh_not_implemented() {
        // Generate 2-of-3 threshold shares
        let shares = generate_key_shares(2, 3).unwrap();

        // Generate a peer keypair
        let peer_secret = SecretKey::from_slice(&[0x42; 32]).unwrap();
        let peer_pubkey = PublicKey::from_secret_key(SECP256K1, &peer_secret);

        // Threshold ECDH should return an error
        let threshold_shares = vec![(1, &shares[0]), (2, &shares[1])];
        let result = threshold_ecdh(&threshold_shares, &peer_pubkey);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not yet implemented"));
    }
}
