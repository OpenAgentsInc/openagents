//! Threshold ECDH for NIP-44 encrypted state decryption
//!
//! # Status: Not Yet Implemented
//!
//! Threshold ECDH requires computing shared_secret = private_key * peer_public_key
//! using k-of-n secret shares without reconstructing the private key.
//!
//! ## Challenge
//!
//! The naive approach of performing ECDH with each share and aggregating
//! the points using Lagrange interpolation does NOT work because:
//! - ECDH operates on elliptic curve points (non-linear)
//! - Lagrange interpolation requires linearity
//! - Point coordinates don't preserve polynomial structure
//!
//! ## Correct Approaches
//!
//! 1. **Multiplicative threshold ECDH** (complex):
//!    - Requires homomorphic properties of elliptic curves
//!    - Need to compute Lagrange coefficients in scalar field
//!    - Then combine: shared_secret = Σ(λ_i * (share_i * P))
//!
//! 2. **Reconstruct-then-ECDH** (simpler but less secure):
//!    - Reconstruct full private key in secure environment
//!    - Perform standard ECDH
//!    - Immediately zero private key
//!
//! For now, applications needing encrypted state can use alternative approaches
//! such as threshold signing with derived keys.

use crate::{Error, Result};

/// Perform threshold ECDH using k-of-n shares
///
/// # Not Yet Implemented
///
/// This function is not yet implemented because threshold ECDH requires
/// special cryptographic techniques that preserve the polynomial structure
/// across elliptic curve operations. The naive approach of interpolating
/// point coordinates fails mathematically.
///
/// Proper implementation requires either:
/// 1. Multiplicative threshold ECDH with Lagrange in the scalar field
/// 2. Or reconstruct-and-ECDH approach (less secure but simpler)
///
/// See module documentation for technical details.
pub fn threshold_ecdh(_shares: &[crate::keygen::Share], _target_pubkey: &[u8; 32]) -> Result<[u8; 32]> {
    Err(Error::Protocol(
        "Threshold ECDH not yet implemented. Requires multiplicative \
         threshold ECDH with Lagrange coefficients in scalar field. \
         See crates/frostr/src/ecdh.rs for details.".into()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keygen::split_secret;

    #[test]
    fn test_threshold_ecdh_not_implemented() {
        // Threshold ECDH should return an error
        let secret = [0x42; 32];
        let shares = split_secret(&secret, 2, 3).unwrap();
        let target_pk = [0x88; 32];

        let result = threshold_ecdh(&shares[0..2], &target_pk);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not yet implemented"));
    }
}
