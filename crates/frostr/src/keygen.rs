//! Key generation and Shamir Secret Sharing
//!
//! This module provides functions for generating threshold key shares using
//! Shamir Secret Sharing and FROST key generation.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// A secret share in a threshold scheme
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Share {
    /// Share index (1..n)
    pub index: u32,
    /// Secret share value (32 bytes)
    pub secret: [u8; 32],
    /// Group public key this share belongs to
    pub group_pk: [u8; 32],
}

/// Generate threshold key shares
///
/// Creates a k-of-n threshold scheme where any k shares can reconstruct
/// the signing capability, but k-1 shares reveal nothing.
///
/// # Arguments
/// * `threshold` - Minimum shares needed (k)
/// * `total` - Total shares to create (n)
///
/// # Returns
/// * Group public key (32 bytes x-only)
/// * Vector of secret shares
///
/// # Example
/// ```no_run
/// use frostr::keygen::generate_key_shares;
///
/// // Create 2-of-3 threshold
/// let (group_pk, shares) = generate_key_shares(2, 3)?;
/// assert_eq!(shares.len(), 3);
/// ```
pub fn generate_key_shares(threshold: u32, total: u32) -> Result<([u8; 32], Vec<Share>)> {
    if threshold > total {
        return Err(Error::InvalidThreshold(threshold, total));
    }
    if threshold == 0 || total == 0 {
        return Err(Error::InvalidThreshold(threshold, total));
    }

    // TODO: Implement FROST key generation
    // For now, return placeholder
    todo!("Implement FROST key generation")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "not yet implemented")]
    fn test_generate_key_shares_placeholder() {
        let _ = generate_key_shares(2, 3);
    }

    #[test]
    fn test_invalid_threshold() {
        let result = generate_key_shares(4, 3);
        assert!(result.is_err());
    }

    #[test]
    fn test_zero_threshold() {
        let result = generate_key_shares(0, 3);
        assert!(result.is_err());
    }
}
