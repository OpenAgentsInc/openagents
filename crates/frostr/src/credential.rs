//! Credential encoding for FROSTR
//!
//! This module handles bech32 encoding of group and share credentials.

use crate::{Error, Result};
use bech32::{Bech32, Hrp};
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
        // Encode: [threshold:4][total:4][group_pk:32]
        let mut data = Vec::with_capacity(40);
        data.extend_from_slice(&self.threshold.to_be_bytes());
        data.extend_from_slice(&self.total.to_be_bytes());
        data.extend_from_slice(&self.group_pk);

        let hrp =
            Hrp::parse("bfgroup").map_err(|e| Error::Encoding(format!("invalid HRP: {}", e)))?;

        bech32::encode::<Bech32>(hrp, &data)
            .map_err(|e| Error::Encoding(format!("bech32 encoding failed: {}", e)))
    }

    /// Decode from bech32 string
    pub fn from_bech32(s: &str) -> Result<Self> {
        let (hrp, data) = bech32::decode(s)
            .map_err(|e| Error::Encoding(format!("bech32 decoding failed: {}", e)))?;

        if hrp.to_string() != "bfgroup" {
            return Err(Error::Encoding(format!(
                "invalid HRP: expected 'bfgroup', got '{}'",
                hrp
            )));
        }

        if data.len() != 40 {
            return Err(Error::Encoding(format!(
                "invalid data length: expected 40 bytes, got {}",
                data.len()
            )));
        }

        let threshold = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let total = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let mut group_pk = [0u8; 32];
        group_pk.copy_from_slice(&data[8..40]);

        // Validate threshold
        if threshold == 0 || threshold > total {
            return Err(Error::InvalidThreshold(threshold, total));
        }

        Ok(GroupCredential {
            threshold,
            total,
            group_pk,
        })
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
        // Encode: [index:4][secret:32][group_pk:32]
        let mut data = Vec::with_capacity(68);
        data.extend_from_slice(&self.index.to_be_bytes());
        data.extend_from_slice(&self.secret);
        data.extend_from_slice(&self.group_pk);

        let hrp =
            Hrp::parse("bfshare").map_err(|e| Error::Encoding(format!("invalid HRP: {}", e)))?;

        bech32::encode::<Bech32>(hrp, &data)
            .map_err(|e| Error::Encoding(format!("bech32 encoding failed: {}", e)))
    }

    /// Decode from bech32 string
    pub fn from_bech32(s: &str) -> Result<Self> {
        let (hrp, data) = bech32::decode(s)
            .map_err(|e| Error::Encoding(format!("bech32 decoding failed: {}", e)))?;

        if hrp.to_string() != "bfshare" {
            return Err(Error::Encoding(format!(
                "invalid HRP: expected 'bfshare', got '{}'",
                hrp
            )));
        }

        if data.len() != 68 {
            return Err(Error::Encoding(format!(
                "invalid data length: expected 68 bytes, got {}",
                data.len()
            )));
        }

        let index = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&data[4..36]);
        let mut group_pk = [0u8; 32];
        group_pk.copy_from_slice(&data[36..68]);

        // Validate index
        if index == 0 {
            return Err(Error::Encoding("share index must be >= 1".to_string()));
        }

        Ok(ShareCredential {
            index,
            secret,
            group_pk,
        })
    }

    /// Validate that this share matches a group credential
    pub fn matches_group(&self, group: &GroupCredential) -> bool {
        self.group_pk == group.group_pk && self.index >= 1 && self.index <= group.total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_credential_round_trip() {
        let group = GroupCredential {
            threshold: 2,
            total: 3,
            group_pk: [0x42; 32],
        };

        let encoded = group.to_bech32().unwrap();
        assert!(encoded.starts_with("bfgroup1"));

        let decoded = GroupCredential::from_bech32(&encoded).unwrap();
        assert_eq!(decoded.threshold, group.threshold);
        assert_eq!(decoded.total, group.total);
        assert_eq!(decoded.group_pk, group.group_pk);
    }

    #[test]
    fn test_share_credential_round_trip() {
        let share = ShareCredential {
            index: 1,
            secret: [0x99; 32],
            group_pk: [0x42; 32],
        };

        let encoded = share.to_bech32().unwrap();
        assert!(encoded.starts_with("bfshare1"));

        let decoded = ShareCredential::from_bech32(&encoded).unwrap();
        assert_eq!(decoded.index, share.index);
        assert_eq!(decoded.secret, share.secret);
        assert_eq!(decoded.group_pk, share.group_pk);
    }

    #[test]
    fn test_invalid_bech32_strings() {
        // Invalid HRP for group
        let result = GroupCredential::from_bech32(
            "bfshare1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqy9ggjn",
        );
        assert!(result.is_err());

        // Invalid HRP for share
        let result = ShareCredential::from_bech32(
            "bfgroup1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqy9ggjn",
        );
        assert!(result.is_err());

        // Completely invalid bech32
        let result = GroupCredential::from_bech32("not-a-bech32-string");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_checksum() {
        // Valid structure but corrupted checksum
        let group = GroupCredential {
            threshold: 2,
            total: 3,
            group_pk: [0x42; 32],
        };
        let encoded = group.to_bech32().unwrap();

        // Corrupt the last character
        let mut chars: Vec<char> = encoded.chars().collect();
        let last_idx = chars.len() - 1;
        chars[last_idx] = if chars[last_idx] == 'a' { 'b' } else { 'a' };
        let corrupted: String = chars.into_iter().collect();

        let result = GroupCredential::from_bech32(&corrupted);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_threshold() {
        // Threshold > total
        let mut data = Vec::with_capacity(40);
        data.extend_from_slice(&5u32.to_be_bytes()); // threshold = 5
        data.extend_from_slice(&3u32.to_be_bytes()); // total = 3
        data.extend_from_slice(&[0x42; 32]);

        let hrp = Hrp::parse("bfgroup").unwrap();
        let encoded = bech32::encode::<Bech32>(hrp, &data).unwrap();
        let result = GroupCredential::from_bech32(&encoded);
        assert!(result.is_err());

        // Threshold = 0
        let mut data = Vec::with_capacity(40);
        data.extend_from_slice(&0u32.to_be_bytes()); // threshold = 0
        data.extend_from_slice(&3u32.to_be_bytes()); // total = 3
        data.extend_from_slice(&[0x42; 32]);

        let hrp = Hrp::parse("bfgroup").unwrap();
        let encoded = bech32::encode::<Bech32>(hrp, &data).unwrap();
        let result = GroupCredential::from_bech32(&encoded);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_share_index() {
        // Share index = 0
        let mut data = Vec::with_capacity(68);
        data.extend_from_slice(&0u32.to_be_bytes()); // index = 0
        data.extend_from_slice(&[0x99; 32]);
        data.extend_from_slice(&[0x42; 32]);

        let hrp = Hrp::parse("bfshare").unwrap();
        let encoded = bech32::encode::<Bech32>(hrp, &data).unwrap();
        let result = ShareCredential::from_bech32(&encoded);
        assert!(result.is_err());
    }

    #[test]
    fn test_share_group_mismatch() {
        let group = GroupCredential {
            threshold: 2,
            total: 3,
            group_pk: [0x42; 32],
        };

        // Share with different group_pk
        let share_wrong_pk = ShareCredential {
            index: 1,
            secret: [0x99; 32],
            group_pk: [0x43; 32], // Different from group
        };
        assert!(!share_wrong_pk.matches_group(&group));

        // Share with correct group_pk
        let share_correct = ShareCredential {
            index: 1,
            secret: [0x99; 32],
            group_pk: [0x42; 32], // Matches group
        };
        assert!(share_correct.matches_group(&group));
    }

    #[test]
    fn test_share_index_out_of_range() {
        let group = GroupCredential {
            threshold: 2,
            total: 3,
            group_pk: [0x42; 32],
        };

        // Share index > total
        let share_high = ShareCredential {
            index: 4, // > total (3)
            secret: [0x99; 32],
            group_pk: [0x42; 32],
        };
        assert!(!share_high.matches_group(&group));

        // Share index = 0
        let share_zero = ShareCredential {
            index: 0,
            secret: [0x99; 32],
            group_pk: [0x42; 32],
        };
        assert!(!share_zero.matches_group(&group));

        // Valid share index
        let share_valid = ShareCredential {
            index: 2, // Within 1..3
            secret: [0x99; 32],
            group_pk: [0x42; 32],
        };
        assert!(share_valid.matches_group(&group));
    }

    #[test]
    fn test_different_threshold_values() {
        // Test 1-of-1
        let group_1_1 = GroupCredential {
            threshold: 1,
            total: 1,
            group_pk: [0x01; 32],
        };
        let encoded = group_1_1.to_bech32().unwrap();
        let decoded = GroupCredential::from_bech32(&encoded).unwrap();
        assert_eq!(decoded.threshold, 1);
        assert_eq!(decoded.total, 1);

        // Test 5-of-7
        let group_5_7 = GroupCredential {
            threshold: 5,
            total: 7,
            group_pk: [0x05; 32],
        };
        let encoded = group_5_7.to_bech32().unwrap();
        let decoded = GroupCredential::from_bech32(&encoded).unwrap();
        assert_eq!(decoded.threshold, 5);
        assert_eq!(decoded.total, 7);
    }

    #[test]
    fn test_different_public_keys() {
        // All zeros
        let group_zeros = GroupCredential {
            threshold: 2,
            total: 3,
            group_pk: [0x00; 32],
        };
        let encoded = group_zeros.to_bech32().unwrap();
        let decoded = GroupCredential::from_bech32(&encoded).unwrap();
        assert_eq!(decoded.group_pk, [0x00; 32]);

        // All ones
        let group_ones = GroupCredential {
            threshold: 2,
            total: 3,
            group_pk: [0xFF; 32],
        };
        let encoded = group_ones.to_bech32().unwrap();
        let decoded = GroupCredential::from_bech32(&encoded).unwrap();
        assert_eq!(decoded.group_pk, [0xFF; 32]);

        // Random pattern
        let mut pk = [0u8; 32];
        for (i, byte) in pk.iter_mut().enumerate() {
            *byte = i as u8;
        }
        let group_pattern = GroupCredential {
            threshold: 2,
            total: 3,
            group_pk: pk,
        };
        let encoded = group_pattern.to_bech32().unwrap();
        let decoded = GroupCredential::from_bech32(&encoded).unwrap();
        assert_eq!(decoded.group_pk, pk);
    }
}
