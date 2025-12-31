//! Threshold ECDH for NIP-44 encrypted message decryption
//!
//! This module implements threshold ECDH where k-of-n participants can derive
//! a shared secret without reconstructing the private key.
//!
//! ## Algorithm
//!
//! For each participant i in the k-of-n quorum:
//! 1. Compute Lagrange coefficient: λ_i = Π_{j∈quorum,j≠i} (0-j)/(i-j) mod n
//! 2. Compute weighted share: weighted = λ_i * secret_share_i mod n
//! 3. Compute partial ECDH point: partial_i = peer_pubkey * weighted
//!
//! Combine:
//! 4. shared_secret = Σ partial_i (point addition)
//!
//! ## Why It Works
//!
//! The secret key s = Σ λ_i * share_i (Lagrange interpolation at x=0)
//! Therefore: s * P = (Σ λ_i * share_i) * P = Σ (λ_i * share_i * P)
//!
//! The key insight: Lagrange coefficients are computed in the SCALAR field
//! and applied BEFORE point multiplication, not after.
//!
//! ## Reference
//!
//! Based on the algorithm in @cmdcode/frost and @frostr/bifrost.
//! See: <https://github.com/cmdruid/frost/blob/master/src/lib/ecdh.ts>

use crate::{Error, Result, keygen::FrostShare};
use k256::elliptic_curve::group::ff::PrimeField;
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::{ProjectivePoint, PublicKey, Scalar};
use std::collections::HashMap;

/// A partial ECDH share from one participant
#[derive(Debug, Clone)]
pub struct EcdhShare {
    /// Participant index
    pub index: u16,
    /// Partial ECDH point (compressed public key format)
    pub partial_point: [u8; 33],
}

/// Compute Lagrange coefficient λ_i for participant at index `my_idx`
/// relative to other participants in `members`, evaluated at x=0.
///
/// λ_i(0) = Π_{j∈members,j≠i} (0-j)/(i-j) mod n
///
/// This is computed in the secp256k1 scalar field (mod n).
fn compute_lagrange_coefficient(my_idx: u16, members: &[u16]) -> Result<Scalar> {
    let my_idx_scalar = Scalar::from(my_idx as u64);

    let mut numerator = Scalar::ONE;
    let mut denominator = Scalar::ONE;

    for &j in members {
        if j == my_idx {
            continue;
        }

        let j_scalar = Scalar::from(j as u64);

        // numerator *= (0 - j) = -j
        numerator *= -j_scalar;

        // denominator *= (my_idx - j)
        denominator *= my_idx_scalar - j_scalar;
    }

    // Compute numerator / denominator = numerator * denominator^(-1)
    let denom_inv = denominator
        .invert()
        .into_option()
        .ok_or_else(|| Error::Crypto("Cannot invert zero denominator".into()))?;

    Ok(numerator * denom_inv)
}

fn compute_lagrange_coefficients(members: &[u16]) -> Result<HashMap<u16, Scalar>> {
    let mut coefficients = HashMap::with_capacity(members.len());
    for &member in members {
        let lambda = compute_lagrange_coefficient(member, members)?;
        coefficients.insert(member, lambda);
    }
    Ok(coefficients)
}

fn create_ecdh_share_with_lambda(
    frost_share: &FrostShare,
    my_idx: u16,
    lambda: &Scalar,
    peer_pubkey: &[u8; 32],
) -> Result<EcdhShare> {
    // Get our secret share from the FROST key package
    let signing_share = frost_share.key_package.signing_share();
    let share_vec = signing_share.serialize();

    // Convert to fixed-size array
    let share_bytes: [u8; 32] = share_vec
        .as_slice()
        .try_into()
        .map_err(|_| Error::Crypto("Share is not 32 bytes".into()))?;

    // Convert share to k256 Scalar
    let share_scalar = Scalar::from_repr(share_bytes.into())
        .into_option()
        .ok_or_else(|| Error::Crypto("Invalid share scalar".into()))?;

    // Compute weighted = lambda * share
    let weighted = *lambda * share_scalar;

    // Parse peer public key (x-only to full public key, assume even y)
    let mut full_pubkey_bytes = [0u8; 33];
    full_pubkey_bytes[0] = 0x02;
    full_pubkey_bytes[1..33].copy_from_slice(peer_pubkey);

    let peer_point = PublicKey::from_sec1_bytes(&full_pubkey_bytes)
        .map_err(|e| Error::InvalidPublicKey(format!("{}", e)))?;

    // Compute partial ECDH: peer_pubkey * weighted
    let peer_projective = ProjectivePoint::from(*peer_point.as_affine());
    let partial_point = peer_projective * weighted;

    // Serialize the partial point
    let partial_affine = partial_point.to_affine();
    let encoded = partial_affine.to_encoded_point(true);
    let partial_bytes: [u8; 33] = encoded
        .as_bytes()
        .try_into()
        .map_err(|_| Error::Crypto("Failed to serialize point".into()))?;

    Ok(EcdhShare {
        index: my_idx,
        partial_point: partial_bytes,
    })
}

/// Create an ECDH share for this participant
///
/// Given our FROST share and the list of participating member indices,
/// compute our partial ECDH contribution.
///
/// # Arguments
/// * `frost_share` - Our FROST key share
/// * `members` - All participant indices in this ECDH session (including ourselves)
/// * `peer_pubkey` - The public key to compute ECDH with (32-byte x-only)
///
/// # Returns
/// Our partial ECDH share that can be combined with others
pub fn create_ecdh_share(
    frost_share: &FrostShare,
    members: &[u16],
    peer_pubkey: &[u8; 32],
) -> Result<EcdhShare> {
    // Get our identifier/index from the FROST key package
    let id_bytes = frost_share.key_package.identifier().serialize();
    let my_idx = u16::from_be_bytes([id_bytes[id_bytes.len() - 2], id_bytes[id_bytes.len() - 1]]);

    // Compute Lagrange coefficient
    let lambda = compute_lagrange_coefficient(my_idx, members)?;

    create_ecdh_share_with_lambda(frost_share, my_idx, &lambda, peer_pubkey)
}

/// Create ECDH shares with precomputed Lagrange coefficients.
pub fn create_ecdh_shares(shares: &[FrostShare], peer_pubkey: &[u8; 32]) -> Result<Vec<EcdhShare>> {
    if shares.is_empty() {
        return Err(Error::InvalidShareCount { need: 1, got: 0 });
    }

    let members: Vec<u16> = shares
        .iter()
        .map(|share| {
            let id_bytes = share.key_package.identifier().serialize();
            u16::from_be_bytes([id_bytes[id_bytes.len() - 2], id_bytes[id_bytes.len() - 1]])
        })
        .collect();

    let coefficients = compute_lagrange_coefficients(&members)?;

    shares
        .iter()
        .map(|share| {
            let id_bytes = share.key_package.identifier().serialize();
            let my_idx =
                u16::from_be_bytes([id_bytes[id_bytes.len() - 2], id_bytes[id_bytes.len() - 1]]);
            let lambda = coefficients
                .get(&my_idx)
                .ok_or_else(|| Error::Crypto("Missing Lagrange coefficient".into()))?;
            create_ecdh_share_with_lambda(share, my_idx, lambda, peer_pubkey)
        })
        .collect()
}

/// Combine ECDH shares from threshold participants to derive the shared secret
///
/// # Arguments
/// * `shares` - Partial ECDH shares from k participants
///
/// # Returns
/// The 32-byte shared secret (x-coordinate of the combined point)
pub fn combine_ecdh_shares(shares: &[EcdhShare]) -> Result<[u8; 32]> {
    if shares.is_empty() {
        return Err(Error::InvalidShareCount { need: 1, got: 0 });
    }

    // Parse and sum all partial points
    let mut combined = ProjectivePoint::IDENTITY;

    for share in shares {
        let point = PublicKey::from_sec1_bytes(&share.partial_point)
            .map_err(|e| Error::Crypto(format!("Invalid partial point: {}", e)))?;
        combined += ProjectivePoint::from(*point.as_affine());
    }

    // Extract x-coordinate (shared secret for NIP-44)
    let affine = combined.to_affine();
    let encoded = affine.to_encoded_point(false); // uncompressed to get x easily
    let x_bytes = encoded
        .x()
        .ok_or_else(|| Error::Crypto("Point at infinity".into()))?;

    let mut result = [0u8; 32];
    result.copy_from_slice(x_bytes);
    Ok(result)
}

/// Perform threshold ECDH in a single step (for testing/simple cases)
///
/// Given k FROST shares from a k-of-n threshold scheme and a peer public key,
/// compute the shared ECDH secret.
///
/// # Arguments
/// * `shares` - At least k FROST shares
/// * `peer_pubkey` - 32-byte x-only public key of the peer
///
/// # Returns
/// 32-byte shared secret compatible with NIP-44
pub fn threshold_ecdh(shares: &[FrostShare], peer_pubkey: &[u8; 32]) -> Result<[u8; 32]> {
    if shares.is_empty() {
        return Err(Error::InvalidShareCount { need: 1, got: 0 });
    }

    let ecdh_shares = create_ecdh_shares(shares, peer_pubkey)?;

    // Combine shares to get shared secret
    combine_ecdh_shares(&ecdh_shares)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keygen::generate_key_shares;

    /// Generate a valid public key from a scalar (for testing)
    fn pubkey_from_scalar(scalar_bytes: &[u8; 32]) -> [u8; 32] {
        let scalar = Scalar::from_repr((*scalar_bytes).into()).unwrap();
        let point = ProjectivePoint::GENERATOR * scalar;
        let affine = point.to_affine();
        let encoded = affine.to_encoded_point(true);
        let bytes = encoded.as_bytes();
        bytes[1..33].try_into().unwrap()
    }

    #[test]
    fn test_threshold_ecdh_2_of_3() {
        // Generate 2-of-3 threshold shares
        let shares = generate_key_shares(2, 3).expect("keygen should succeed");
        assert_eq!(shares.len(), 3);

        // Generate a peer keypair
        let peer_secret = [
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0xa1, 0xa2, 0xa3, 0xa4,
            0xa5, 0xa6, 0xa7, 0xa8,
        ];
        let peer_pubkey_bytes = pubkey_from_scalar(&peer_secret);

        // Test with shares 0 and 1
        let result1 = threshold_ecdh(&shares[0..2], &peer_pubkey_bytes).expect("ECDH should work");

        // Test with shares 1 and 2
        let result2 = threshold_ecdh(&shares[1..3], &peer_pubkey_bytes).expect("ECDH should work");

        // Test with shares 0 and 2
        let result3 = threshold_ecdh(&[shares[0].clone(), shares[2].clone()], &peer_pubkey_bytes)
            .expect("ECDH should work");

        // All combinations should produce the same shared secret
        assert_eq!(
            result1, result2,
            "Different share combinations should produce same secret"
        );
        assert_eq!(
            result2, result3,
            "Different share combinations should produce same secret"
        );
    }

    #[test]
    fn test_threshold_ecdh_3_of_5() {
        // Generate 3-of-5 threshold shares
        let shares = generate_key_shares(3, 5).expect("keygen should succeed");
        assert_eq!(shares.len(), 5);

        // Generate a peer public key
        let peer_secret = [0x42u8; 32];
        let peer_pubkey_bytes = pubkey_from_scalar(&peer_secret);

        // Test with different combinations
        let result1 = threshold_ecdh(&shares[0..3], &peer_pubkey_bytes).expect("ECDH should work");
        let result2 = threshold_ecdh(&shares[2..5], &peer_pubkey_bytes).expect("ECDH should work");
        let result3 = threshold_ecdh(
            &[shares[0].clone(), shares[2].clone(), shares[4].clone()],
            &peer_pubkey_bytes,
        )
        .expect("ECDH should work");

        // All should be equal
        assert_eq!(result1, result2);
        assert_eq!(result2, result3);
    }

    #[test]
    fn test_lagrange_coefficient_basic() {
        // Simple test: 2-of-3 with indices 1, 2
        // For index 1 with members [1, 2], evaluating at 0:
        // λ_1(0) = (0-2)/(1-2) = -2/-1 = 2
        let coeff = compute_lagrange_coefficient(1, &[1, 2]).expect("should work");
        assert_eq!(coeff, Scalar::from(2u64));
    }

    #[test]
    fn test_lagrange_coefficient_three_members() {
        // For index 1 with members [1, 2, 3], evaluating at 0:
        // λ_1(0) = ((0-2)*(0-3))/((1-2)*(1-3)) = ((-2)*(-3))/((-1)*(-2)) = 6/2 = 3
        let coeff = compute_lagrange_coefficient(1, &[1, 2, 3]).expect("should work");
        assert_eq!(coeff, Scalar::from(3u64));
    }

    #[test]
    fn test_empty_shares() {
        let peer_pubkey = [0x42u8; 32];
        let result = threshold_ecdh(&[], &peer_pubkey);
        assert!(result.is_err());
    }

    #[test]
    fn test_ecdh_share_creation() {
        let shares = generate_key_shares(2, 3).expect("keygen should succeed");

        // Use a valid peer public key
        let peer_secret = [0x42u8; 32];
        let peer_pubkey_bytes = pubkey_from_scalar(&peer_secret);

        let members = vec![1u16, 2u16];
        let ecdh_share = create_ecdh_share(&shares[0], &members, &peer_pubkey_bytes);
        assert!(ecdh_share.is_ok());
    }

    #[test]
    fn test_threshold_ecdh_matches_regular_ecdh() {
        let shares = generate_key_shares(2, 3).expect("keygen should succeed");

        let peer_secret = [0x24u8; 32];
        let peer_pubkey_bytes = pubkey_from_scalar(&peer_secret);

        let key_packages: Vec<frost_secp256k1::keys::KeyPackage> = shares[0..2]
            .iter()
            .map(|share| share.key_package.clone())
            .collect();
        let signing_key = frost_secp256k1::keys::reconstruct(&key_packages)
            .expect("should reconstruct signing key");
        let secret_scalar = signing_key.to_scalar();

        let mut full_pubkey_bytes = [0u8; 33];
        full_pubkey_bytes[0] = 0x02;
        full_pubkey_bytes[1..33].copy_from_slice(&peer_pubkey_bytes);
        let peer_point = PublicKey::from_sec1_bytes(&full_pubkey_bytes).unwrap();
        let shared_point = ProjectivePoint::from(*peer_point.as_affine()) * secret_scalar;
        let encoded = shared_point.to_affine().to_encoded_point(false);
        let x_bytes = encoded.x().expect("x coordinate");
        let mut expected = [0u8; 32];
        expected.copy_from_slice(x_bytes);

        let threshold =
            threshold_ecdh(&shares[0..2], &peer_pubkey_bytes).expect("threshold ECDH should work");
        assert_eq!(threshold, expected);
    }
}
