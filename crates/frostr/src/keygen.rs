//! Key generation and Shamir Secret Sharing
//!
//! This module provides functions for generating threshold key shares using
//! Shamir Secret Sharing and FROST key generation.
//!
//! # Examples
//!
//! ## Generating FROST threshold shares
//!
//! ```
//! use frostr::keygen::generate_key_shares;
//!
//! // Generate 2-of-3 threshold shares
//! let shares = generate_key_shares(2, 3).expect("keygen failed");
//! assert_eq!(shares.len(), 3);
//! assert_eq!(shares[0].threshold, 2);
//! assert_eq!(shares[0].total, 3);
//!
//! // All shares have the same group public key
//! let group_pk = shares[0].public_key_package.verifying_key();
//! assert_eq!(
//!     shares[1].public_key_package.verifying_key(),
//!     group_pk
//! );
//! ```
//!
//! ## Shamir Secret Sharing
//!
//! ```
//! use frostr::keygen::{split_secret, reconstruct_secret};
//!
//! let secret = [42u8; 32];
//!
//! // Split into 3-of-5 shares
//! let shares = split_secret(&secret, 3, 5).expect("split failed");
//! assert_eq!(shares.len(), 5);
//!
//! // Reconstruct with any 3 shares
//! let reconstructed = reconstruct_secret(&shares[0..3]).expect("reconstruct failed");
//! assert_eq!(reconstructed, secret);
//!
//! // Can use different combination
//! let reconstructed2 = reconstruct_secret(&[shares[1].clone(), shares[3].clone(), shares[4].clone()])
//!     .expect("reconstruct failed");
//! assert_eq!(reconstructed2, secret);
//! ```

use crate::{Error, Result};
use frost_secp256k1::keys::{KeyPackage, PublicKeyPackage};
use rand::Rng;
use serde::{Deserialize, Serialize};

/// A secret share in a threshold scheme (for SSS)
///
/// # Examples
///
/// ```
/// use frostr::keygen::{split_secret, Share};
///
/// let secret = [42u8; 32];
///
/// // Split into 2-of-3 shares
/// let shares = split_secret(&secret, 2, 3).expect("split failed");
///
/// assert_eq!(shares.len(), 3);
/// assert_eq!(shares[0].index, 1);
/// assert_eq!(shares[1].index, 2);
/// assert_eq!(shares[2].index, 3);
///
/// // Each share has 32 bytes
/// assert_eq!(shares[0].secret.len(), 32);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Share {
    /// Share index (1..n)
    pub index: u32,
    /// Secret share value (32 bytes)
    pub secret: [u8; 32],
    /// Group public key this share belongs to
    pub group_pk: [u8; 32],
}

/// A FROST key package containing both secret and verification data
///
/// # Examples
///
/// ```
/// use frostr::keygen::generate_key_shares;
///
/// // Generate 2-of-3 threshold FROST shares
/// let shares = generate_key_shares(2, 3).expect("keygen failed");
///
/// assert_eq!(shares.len(), 3);
/// assert_eq!(shares[0].threshold, 2);
/// assert_eq!(shares[0].total, 3);
///
/// // All shares have the same group public key
/// let group_pk = shares[0].public_key_package.verifying_key();
/// assert_eq!(shares[1].public_key_package.verifying_key(), group_pk);
/// assert_eq!(shares[2].public_key_package.verifying_key(), group_pk);
/// ```
#[derive(Debug, Clone)]
pub struct FrostShare {
    /// The FROST key package with signing share
    pub key_package: KeyPackage,
    /// Group public key package for verification
    pub public_key_package: PublicKeyPackage,
    /// Minimum signers required (threshold k)
    pub threshold: u16,
    /// Total number of shares (n)
    pub total: u16,
    /// Participant ID (1-based index)
    pub participant_id: u8,
}

/// Galois Field GF(256) multiplication
/// Uses the AES polynomial x^8 + x^4 + x^3 + x + 1
pub(crate) fn gf256_mul(mut a: u8, mut b: u8) -> u8 {
    let mut p: u8 = 0;
    for _ in 0..8 {
        if b & 1 != 0 {
            p ^= a;
        }
        let hi_bit_set = a & 0x80 != 0;
        a <<= 1;
        if hi_bit_set {
            a ^= 0x1b; // x^8 + x^4 + x^3 + x + 1
        }
        b >>= 1;
    }
    p
}

/// Galois Field GF(256) division
pub(crate) fn gf256_div(a: u8, b: u8) -> u8 {
    if b == 0 {
        panic!("Division by zero in GF(256)");
    }
    if a == 0 {
        return 0;
    }
    // Use logarithm tables for division
    gf256_mul(a, gf256_inv(b))
}

/// Galois Field GF(256) multiplicative inverse
fn gf256_inv(a: u8) -> u8 {
    if a == 0 {
        panic!("Cannot invert zero in GF(256)");
    }
    // Use Fermat's little theorem: a^(p-1) = 1, so a^(-1) = a^(p-2)
    // For GF(256), we compute a^254
    let mut result = a;
    for _ in 0..6 {
        result = gf256_mul(result, result);
        result = gf256_mul(result, a);
    }
    result = gf256_mul(result, result);
    result
}

/// Split a 32-byte secret into n shares using Shamir Secret Sharing
///
/// Any k shares can reconstruct the secret, but k-1 shares reveal nothing.
///
/// # Arguments
/// * `secret` - The 32-byte secret to split
/// * `threshold` - Minimum shares needed to reconstruct (k)
/// * `total` - Total number of shares to create (n)
///
/// # Returns
/// Vector of secret shares, each containing an index and share value
pub fn split_secret(secret: &[u8; 32], threshold: u32, total: u32) -> Result<Vec<Share>> {
    if threshold > total {
        return Err(Error::InvalidThreshold(threshold, total));
    }
    if threshold == 0 || total == 0 {
        return Err(Error::InvalidThreshold(threshold, total));
    }
    if threshold == 1 {
        // Trivial case: just copy the secret to all shares
        return Ok((1..=total)
            .map(|i| Share {
                index: i,
                secret: *secret,
                group_pk: [0u8; 32], // Not used in SSS-only mode
            })
            .collect());
    }

    let mut rng = rand::thread_rng();
    let mut shares = Vec::with_capacity(total as usize);

    // For each byte of the secret, create a polynomial with random coefficients
    // Then evaluate it at each x value to generate shares
    let mut polynomials: Vec<Vec<u8>> = Vec::with_capacity(32);

    for &secret_byte in secret.iter() {
        // Create polynomial: f(x) = a0 + a1*x + a2*x^2 + ... + a(k-1)*x^(k-1)
        // where a0 = secret_byte
        let mut coeffs = vec![secret_byte];
        for _ in 1..threshold {
            coeffs.push(rng.r#gen::<u8>());
        }
        polynomials.push(coeffs);
    }

    // Now evaluate each polynomial at points x=1, 2, ..., n to create shares
    for x in 1..=total {
        let mut share_bytes = [0u8; 32];

        for byte_idx in 0..32 {
            let coeffs = &polynomials[byte_idx];

            // Evaluate polynomial at point x using Horner's method in GF(256)
            let mut y = coeffs[coeffs.len() - 1];
            for i in (0..coeffs.len() - 1).rev() {
                y = gf256_mul(y, x as u8) ^ coeffs[i];
            }
            share_bytes[byte_idx] = y;
        }

        shares.push(Share {
            index: x,
            secret: share_bytes,
            group_pk: [0u8; 32], // Not used in SSS-only mode
        });
    }

    Ok(shares)
}

/// Reconstruct secret from k shares using Lagrange interpolation
///
/// WARNING: This function is for testing only. In production FROSTR,
/// the secret is NEVER reconstructed - shares are used directly for
/// threshold signing.
///
/// # Arguments
/// * `shares` - At least k shares to reconstruct from
///
/// # Returns
/// The reconstructed 32-byte secret
pub fn reconstruct_secret(shares: &[Share]) -> Result<[u8; 32]> {
    if shares.is_empty() {
        return Err(Error::InvalidThreshold(0, 0));
    }

    // Validate no duplicate indices (would cause division by zero)
    for i in 0..shares.len() {
        for j in (i + 1)..shares.len() {
            if shares[i].index == shares[j].index {
                return Err(Error::FrostError(format!(
                    "Duplicate share index: {} appears multiple times",
                    shares[i].index
                )));
            }
        }
    }

    let mut secret = [0u8; 32];

    // For each byte position, perform Lagrange interpolation in GF(256)
    for (byte_idx, secret_byte) in secret.iter_mut().enumerate() {
        let mut result: u8 = 0;

        // Lagrange interpolation: f(0) = sum(y_i * L_i(0))
        // where L_i(0) = product((0 - x_j) / (x_i - x_j)) for j != i
        for i in 0..shares.len() {
            let xi = shares[i].index;
            let yi = shares[i].secret[byte_idx];

            // Compute Lagrange basis polynomial L_i(0)
            // L_i(0) = product((0 - xj) / (xi - xj)) for all j != i
            let mut li = 1u8;

            for (j, share_j) in shares.iter().enumerate() {
                if i != j {
                    let xj = share_j.index;

                    // In GF(256): (0 - xj) / (xi - xj) = xj / (xi XOR xj)
                    // since subtraction is XOR in characteristic 2
                    let numerator = xj as u8;
                    let denominator = (xi ^ xj) as u8;

                    li = gf256_mul(li, gf256_div(numerator, denominator));
                }
            }

            // Add yi * L_i(0) to result (addition is XOR in GF(256))
            result ^= gf256_mul(yi, li);
        }

        *secret_byte = result;
    }

    Ok(secret)
}

/// Generate threshold key shares using FROST
///
/// Creates a k-of-n threshold scheme where any k shares can sign,
/// but k-1 shares reveal nothing. Uses the FROST protocol for
/// verifiable secret sharing.
///
/// # Arguments
/// * `threshold` - Minimum shares needed (k)
/// * `total` - Total shares to create (n)
///
/// # Returns
/// * Group public key (32 bytes x-only)
/// * Vector of FROST key packages
///
/// # Example
/// ```
/// use frostr::keygen::generate_key_shares;
///
/// // Create 2-of-3 threshold
/// let shares = generate_key_shares(2, 3).unwrap();
/// assert_eq!(shares.len(), 3);
/// ```
pub fn generate_key_shares(threshold: u32, total: u32) -> Result<Vec<FrostShare>> {
    if threshold > total {
        return Err(Error::InvalidThreshold(threshold, total));
    }
    if threshold == 0 || total == 0 {
        return Err(Error::InvalidThreshold(threshold, total));
    }
    if threshold > u16::MAX as u32 || total > u16::MAX as u32 {
        return Err(Error::InvalidThreshold(threshold, total));
    }

    let mut rng = rand::thread_rng();

    // Create identifiers for each participant (1..=total)
    let identifiers: Vec<_> = (1..=total)
        .map(|i| {
            frost_secp256k1::Identifier::try_from(i as u16)
                .expect("Identifier creation should succeed for valid indices")
        })
        .collect();

    // Generate key shares using FROST dealer
    let (secret_shares, public_key_package) = frost_secp256k1::keys::generate_with_dealer(
        total as u16,
        threshold as u16,
        frost_secp256k1::keys::IdentifierList::Custom(&identifiers),
        &mut rng,
    )
    .map_err(|e| Error::FrostError(format!("FROST keygen failed: {:?}", e)))?;

    // Convert BTreeMap to Vec of FrostShare
    // We enumerate with 1-based indices since identifiers are created as 1..=n
    let frost_shares: Vec<FrostShare> = secret_shares
        .into_iter()
        .enumerate()
        .map(|(idx, (_identifier, secret_share))| {
            // Convert SecretShare to KeyPackage (performs validation)
            let key_package: KeyPackage = secret_share.try_into().expect("Valid secret share");
            // Participant ID is 1-based (1..=n)
            let participant_id = (idx + 1) as u8;

            FrostShare {
                key_package,
                public_key_package: public_key_package.clone(),
                threshold: threshold as u16,
                total: total as u16,
                participant_id,
            }
        })
        .collect();

    Ok(frost_shares)
}

/// Reshare an existing group key to a new participant set.
///
/// Uses FROST's `reconstruct` + `split` to rotate holders while keeping
/// the same group public key.
pub fn reshare_frost_shares(
    existing_shares: &[FrostShare],
    new_threshold: u32,
    new_total: u32,
) -> Result<Vec<FrostShare>> {
    if existing_shares.is_empty() {
        return Err(Error::InvalidShareCount { need: 1, got: 0 });
    }
    if new_threshold == 0 || new_total == 0 || new_threshold > new_total {
        return Err(Error::InvalidThreshold(new_threshold, new_total));
    }
    if new_threshold > u16::MAX as u32 || new_total > u16::MAX as u32 {
        return Err(Error::InvalidThreshold(new_threshold, new_total));
    }

    let required = existing_shares[0].threshold as u32;
    if existing_shares.len() < required as usize {
        return Err(Error::InvalidShareCount {
            need: required,
            got: existing_shares.len() as u32,
        });
    }

    let key_packages: Vec<KeyPackage> = existing_shares
        .iter()
        .map(|share| share.key_package.clone())
        .collect();

    let signing_key = frost_secp256k1::keys::reconstruct(&key_packages)
        .map_err(|e| Error::FrostError(format!("FROST reconstruct failed: {:?}", e)))?;

    let mut rng = rand::thread_rng();
    let identifiers: Vec<_> = (1..=new_total)
        .map(|i| {
            frost_secp256k1::Identifier::try_from(i as u16)
                .expect("Identifier creation should succeed for valid indices")
        })
        .collect();

    let (secret_shares, public_key_package) = frost_secp256k1::keys::split(
        &signing_key,
        new_total as u16,
        new_threshold as u16,
        frost_secp256k1::keys::IdentifierList::Custom(&identifiers),
        &mut rng,
    )
    .map_err(|e| Error::FrostError(format!("FROST reshare failed: {:?}", e)))?;

    let frost_shares: Vec<FrostShare> = secret_shares
        .into_iter()
        .enumerate()
        .map(|(idx, (_identifier, secret_share))| {
            let key_package: KeyPackage = secret_share.try_into().expect("Valid secret share");
            let participant_id = (idx + 1) as u8;

            FrostShare {
                key_package,
                public_key_package: public_key_package.clone(),
                threshold: new_threshold as u16,
                total: new_total as u16,
                participant_id,
            }
        })
        .collect();

    Ok(frost_shares)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gf256_mul() {
        // Test basic multiplication
        assert_eq!(gf256_mul(0, 5), 0);
        assert_eq!(gf256_mul(5, 0), 0);
        assert_eq!(gf256_mul(1, 5), 5);
        assert_eq!(gf256_mul(5, 1), 5);

        // Test field properties
        assert_eq!(gf256_mul(2, 3), gf256_mul(3, 2)); // Commutativity
    }

    #[test]
    fn test_gf256_inv() {
        // Test that a * a^(-1) = 1 for various values
        for a in 1..=255u8 {
            let inv = gf256_inv(a);
            assert_eq!(gf256_mul(a, inv), 1, "Failed for a={}", a);
        }
    }

    #[test]
    #[should_panic(expected = "Cannot invert zero")]
    fn test_gf256_inv_zero() {
        gf256_inv(0);
    }

    #[test]
    fn test_split_secret_2_of_3() {
        let secret = [42u8; 32];
        let shares = split_secret(&secret, 2, 3).unwrap();

        assert_eq!(shares.len(), 3);
        assert_eq!(shares[0].index, 1);
        assert_eq!(shares[1].index, 2);
        assert_eq!(shares[2].index, 3);

        // Verify each share is different from the secret
        assert_ne!(shares[0].secret, secret);
        assert_ne!(shares[1].secret, secret);
        assert_ne!(shares[2].secret, secret);
    }

    #[test]
    fn test_split_secret_3_of_5() {
        let secret = [
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0xa1, 0xa2, 0xa3, 0xa4,
            0xa5, 0xa6, 0xa7, 0xa8,
        ];
        let shares = split_secret(&secret, 3, 5).unwrap();

        assert_eq!(shares.len(), 5);
        for i in 0..5 {
            assert_eq!(shares[i].index, (i + 1) as u32);
        }
    }

    #[test]
    fn test_split_secret_5_of_7() {
        let secret = [0xffu8; 32];
        let shares = split_secret(&secret, 5, 7).unwrap();

        assert_eq!(shares.len(), 7);
        for i in 0..7 {
            assert_eq!(shares[i].index, (i + 1) as u32);
        }
    }

    #[test]
    fn test_reconstruct_exact_threshold_2_of_3() {
        let secret = [42u8; 32];
        let shares = split_secret(&secret, 2, 3).unwrap();

        // Test all combinations of 2 shares
        let reconstructed1 = reconstruct_secret(&shares[0..2]).unwrap();
        assert_eq!(reconstructed1, secret);

        let reconstructed2 = reconstruct_secret(&[shares[0].clone(), shares[2].clone()]).unwrap();
        assert_eq!(reconstructed2, secret);

        let reconstructed3 = reconstruct_secret(&[shares[1].clone(), shares[2].clone()]).unwrap();
        assert_eq!(reconstructed3, secret);
    }

    #[test]
    fn test_reconstruct_exact_threshold_3_of_5() {
        let secret = [
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0xa1, 0xa2, 0xa3, 0xa4,
            0xa5, 0xa6, 0xa7, 0xa8,
        ];
        let shares = split_secret(&secret, 3, 5).unwrap();

        // Test one combination of 3 shares
        let reconstructed = reconstruct_secret(&shares[0..3]).unwrap();
        assert_eq!(reconstructed, secret);

        // Test another combination
        let reconstructed =
            reconstruct_secret(&[shares[0].clone(), shares[2].clone(), shares[4].clone()]).unwrap();
        assert_eq!(reconstructed, secret);
    }

    #[test]
    fn test_reconstruct_exact_threshold_5_of_7() {
        let secret = [0xffu8; 32];
        let shares = split_secret(&secret, 5, 7).unwrap();

        // Test with exactly 5 shares
        let reconstructed = reconstruct_secret(&shares[0..5]).unwrap();
        assert_eq!(reconstructed, secret);

        // Test with different combination
        let reconstructed = reconstruct_secret(&[
            shares[0].clone(),
            shares[2].clone(),
            shares[3].clone(),
            shares[4].clone(),
            shares[6].clone(),
        ])
        .unwrap();
        assert_eq!(reconstructed, secret);
    }

    #[test]
    fn test_reconstruct_more_than_threshold() {
        let secret = [99u8; 32];
        let shares = split_secret(&secret, 2, 5).unwrap();

        // Reconstruct with 3 shares (more than threshold)
        let reconstructed = reconstruct_secret(&shares[0..3]).unwrap();
        assert_eq!(reconstructed, secret);

        // Reconstruct with all 5 shares
        let reconstructed = reconstruct_secret(&shares).unwrap();
        assert_eq!(reconstructed, secret);
    }

    #[test]
    fn test_reconstruct_random_secret() {
        // Test with random-looking secret
        let secret = [
            0x6b, 0x3a, 0x72, 0x91, 0xef, 0x2c, 0x84, 0x15, 0xa3, 0x5d, 0x9e, 0x47, 0xc6, 0x18,
            0xf0, 0x2b, 0x7d, 0x4e, 0x86, 0x39, 0xb1, 0x50, 0xe2, 0x6c, 0x94, 0x1f, 0xa7, 0x35,
            0xd8, 0x4a, 0xbe, 0x63,
        ];
        let shares = split_secret(&secret, 3, 5).unwrap();

        let reconstructed = reconstruct_secret(&shares[0..3]).unwrap();
        assert_eq!(reconstructed, secret);
    }

    #[test]
    fn test_split_secret_threshold_1() {
        // Trivial case: 1-of-n means all shares are identical to secret
        let secret = [123u8; 32];
        let shares = split_secret(&secret, 1, 3).unwrap();

        assert_eq!(shares.len(), 3);
        assert_eq!(shares[0].secret, secret);
        assert_eq!(shares[1].secret, secret);
        assert_eq!(shares[2].secret, secret);

        // Reconstruction should work with any single share
        let reconstructed = reconstruct_secret(&shares[0..1]).unwrap();
        assert_eq!(reconstructed, secret);
    }

    #[test]
    fn test_split_secret_invalid_threshold() {
        let secret = [42u8; 32];

        // threshold > total
        assert!(split_secret(&secret, 4, 3).is_err());

        // threshold = 0
        assert!(split_secret(&secret, 0, 3).is_err());

        // total = 0
        assert!(split_secret(&secret, 2, 0).is_err());
    }

    #[test]
    fn test_reconstruct_empty_shares() {
        let result = reconstruct_secret(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_frost_generate_key_shares_2_of_3() {
        let shares = generate_key_shares(2, 3).unwrap();
        assert_eq!(shares.len(), 3);

        // Verify all shares have the same group public key
        let group_pk = shares[0].public_key_package.verifying_key();
        for share in &shares {
            assert_eq!(share.public_key_package.verifying_key(), group_pk);
        }
    }

    #[test]
    fn test_frost_generate_key_shares_3_of_5() {
        let shares = generate_key_shares(3, 5).unwrap();
        assert_eq!(shares.len(), 5);

        // Verify all shares have the same group public key
        let group_pk = shares[0].public_key_package.verifying_key();
        for share in &shares {
            assert_eq!(share.public_key_package.verifying_key(), group_pk);
        }
    }

    #[test]
    fn test_frost_generate_key_shares_5_of_7() {
        let shares = generate_key_shares(5, 7).unwrap();
        assert_eq!(shares.len(), 7);

        // Verify all shares have the same group public key
        let group_pk = shares[0].public_key_package.verifying_key();
        for share in &shares {
            assert_eq!(share.public_key_package.verifying_key(), group_pk);
        }
    }

    #[test]
    fn test_frost_invalid_threshold() {
        let result = generate_key_shares(4, 3);
        assert!(result.is_err());
    }

    #[test]
    fn test_frost_zero_threshold() {
        let result = generate_key_shares(0, 3);
        assert!(result.is_err());
    }

    #[test]
    fn test_frost_max_participants() {
        // Test boundary conditions
        let result = generate_key_shares(u16::MAX as u32 + 1, u16::MAX as u32 + 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_indices_error() {
        let secret = [42u8; 32];
        let shares = split_secret(&secret, 2, 3).unwrap();

        // Create a vector with duplicate indices (same share twice)
        let duplicate_shares = vec![shares[0].clone(), shares[0].clone()];

        // Should return error for duplicate indices
        let result = reconstruct_secret(&duplicate_shares);
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("Duplicate share index"));
    }

    #[test]
    fn test_multiple_duplicate_indices() {
        let secret = [42u8; 32];
        let shares = split_secret(&secret, 3, 5).unwrap();

        // Create a vector with multiple duplicates
        let duplicate_shares = vec![
            shares[0].clone(),
            shares[1].clone(),
            shares[1].clone(), // Duplicate of index 2
        ];

        let result = reconstruct_secret(&duplicate_shares);
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("Duplicate share index"));
    }

    #[test]
    fn test_reshare_frost_shares_rotates_participants() {
        let shares = generate_key_shares(2, 3).unwrap();
        let group_pk = shares[0].public_key_package.verifying_key();

        let reshared = reshare_frost_shares(&shares[..2], 2, 4).unwrap();
        assert_eq!(reshared.len(), 4);
        assert_eq!(reshared[0].threshold, 2);
        assert_eq!(reshared[0].total, 4);

        for share in &reshared {
            assert_eq!(share.public_key_package.verifying_key(), group_pk);
        }
    }

    #[test]
    fn test_reshare_requires_threshold_shares() {
        let shares = generate_key_shares(3, 5).unwrap();
        let result = reshare_frost_shares(&shares[..2], 2, 4);
        assert!(result.is_err());
    }
}
