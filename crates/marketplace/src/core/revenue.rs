//! Flow of Funds revenue splitting for marketplace transactions
//!
//! Implements transparent revenue distribution across all marketplace participants:
//! - Creators (skill/data authors)
//! - Compute providers
//! - Platform (OpenAgents)
//! - Referrers (optional)
//!
//! # Revenue Split Model
//!
//! Default splits (configurable):
//! - Creator: 55%
//! - Compute: 25%
//! - Platform: 12%
//! - Referrer: 8%
//!
//! Total: 100%

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Revenue split configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueSplitConfig {
    /// Creator's share (0-100)
    pub creator_share: u8,
    /// Compute provider's share (0-100)
    pub compute_share: u8,
    /// Platform's share (0-100)
    pub platform_share: u8,
    /// Referrer's share (0-100)
    pub referrer_share: u8,
}

impl Default for RevenueSplitConfig {
    fn default() -> Self {
        Self {
            creator_share: 55,
            compute_share: 25,
            platform_share: 12,
            referrer_share: 8,
        }
    }
}

impl RevenueSplitConfig {
    /// Create a new revenue split configuration
    ///
    /// # Arguments
    /// * `creator` - Creator share percentage (0-100)
    /// * `compute` - Compute share percentage (0-100)
    /// * `platform` - Platform share percentage (0-100)
    /// * `referrer` - Referrer share percentage (0-100)
    ///
    /// # Returns
    /// Configuration if shares sum to 100, error otherwise
    pub fn new(creator: u8, compute: u8, platform: u8, referrer: u8) -> Result<Self> {
        let total = creator as u16 + compute as u16 + platform as u16 + referrer as u16;
        if total != 100 {
            return Err(anyhow::anyhow!(
                "Revenue shares must sum to 100, got {}",
                total
            ));
        }

        Ok(Self {
            creator_share: creator,
            compute_share: compute,
            platform_share: platform,
            referrer_share: referrer,
        })
    }

    /// Validate that shares sum to 100%
    pub fn validate(&self) -> Result<()> {
        let total = self.creator_share as u16
            + self.compute_share as u16
            + self.platform_share as u16
            + self.referrer_share as u16;

        if total != 100 {
            return Err(anyhow::anyhow!(
                "Revenue shares must sum to 100, got {}",
                total
            ));
        }

        Ok(())
    }
}

/// Revenue split result showing distribution in satoshis
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RevenueSplit {
    /// Total gross amount
    pub gross_sats: u64,
    /// Creator's amount
    pub creator_sats: u64,
    /// Compute provider's amount
    pub compute_sats: u64,
    /// Platform's amount
    pub platform_sats: u64,
    /// Referrer's amount (0 if no referrer)
    pub referrer_sats: u64,
}

impl RevenueSplit {
    /// Calculate revenue split from gross amount
    ///
    /// # Arguments
    /// * `gross_sats` - Total payment amount in satoshis
    /// * `config` - Revenue split configuration
    /// * `has_referrer` - Whether there is a referrer to pay
    ///
    /// # Returns
    /// Revenue split with amounts for each participant
    ///
    /// # Note
    /// If no referrer, referrer's share is added to creator's share
    pub fn calculate(gross_sats: u64, config: &RevenueSplitConfig, has_referrer: bool) -> Self {
        let creator_base = (gross_sats * config.creator_share as u64) / 100;
        let compute = (gross_sats * config.compute_share as u64) / 100;
        let platform = (gross_sats * config.platform_share as u64) / 100;
        let referrer = if has_referrer {
            (gross_sats * config.referrer_share as u64) / 100
        } else {
            0
        };

        // If no referrer, add referrer share to creator
        let creator = if has_referrer {
            creator_base
        } else {
            creator_base + (gross_sats * config.referrer_share as u64) / 100
        };

        // Handle rounding - any remainder goes to creator
        let total = creator + compute + platform + referrer;
        let creator = if total < gross_sats {
            creator + (gross_sats - total)
        } else {
            creator
        };

        Self {
            gross_sats,
            creator_sats: creator,
            compute_sats: compute,
            platform_sats: platform,
            referrer_sats: referrer,
        }
    }

    /// Verify the split adds up to the gross amount
    pub fn verify(&self) -> Result<()> {
        let total = self.creator_sats + self.compute_sats + self.platform_sats + self.referrer_sats;

        if total != self.gross_sats {
            return Err(anyhow::anyhow!(
                "Split total ({}) does not match gross amount ({})",
                total,
                self.gross_sats
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validation() {
        let config = RevenueSplitConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.creator_share, 55);
        assert_eq!(config.compute_share, 25);
        assert_eq!(config.platform_share, 12);
        assert_eq!(config.referrer_share, 8);
    }

    #[test]
    fn test_custom_config_valid() {
        let config = RevenueSplitConfig::new(60, 20, 15, 5).unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_custom_config_invalid() {
        let result = RevenueSplitConfig::new(60, 20, 15, 10);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must sum to 100"));
    }

    #[test]
    fn test_revenue_split_with_referrer() {
        let config = RevenueSplitConfig::default();
        let split = RevenueSplit::calculate(100_000, &config, true);

        assert_eq!(split.gross_sats, 100_000);
        assert_eq!(split.creator_sats, 55_000);
        assert_eq!(split.compute_sats, 25_000);
        assert_eq!(split.platform_sats, 12_000);
        assert_eq!(split.referrer_sats, 8_000);

        assert!(split.verify().is_ok());
    }

    #[test]
    fn test_revenue_split_without_referrer() {
        let config = RevenueSplitConfig::default();
        let split = RevenueSplit::calculate(100_000, &config, false);

        assert_eq!(split.gross_sats, 100_000);
        // Creator gets their share + referrer share
        assert_eq!(split.creator_sats, 63_000);
        assert_eq!(split.compute_sats, 25_000);
        assert_eq!(split.platform_sats, 12_000);
        assert_eq!(split.referrer_sats, 0);

        assert!(split.verify().is_ok());
    }

    #[test]
    fn test_revenue_split_rounding() {
        let config = RevenueSplitConfig::default();
        // Amount that doesn't divide evenly
        let split = RevenueSplit::calculate(1_000, &config, true);

        assert_eq!(split.gross_sats, 1_000);
        // Verify total equals gross (rounding handled)
        assert!(split.verify().is_ok());

        let total =
            split.creator_sats + split.compute_sats + split.platform_sats + split.referrer_sats;
        assert_eq!(total, 1_000);
    }

    #[test]
    fn test_revenue_split_large_amount() {
        let config = RevenueSplitConfig::default();
        let split = RevenueSplit::calculate(10_000_000, &config, true);

        assert_eq!(split.gross_sats, 10_000_000);
        assert_eq!(split.creator_sats, 5_500_000);
        assert_eq!(split.compute_sats, 2_500_000);
        assert_eq!(split.platform_sats, 1_200_000);
        assert_eq!(split.referrer_sats, 800_000);

        assert!(split.verify().is_ok());
    }

    #[test]
    fn test_revenue_split_serialization() {
        let config = RevenueSplitConfig::default();
        let split = RevenueSplit::calculate(100_000, &config, true);

        // Serialize and deserialize
        let json = serde_json::to_string(&split).unwrap();
        let deserialized: RevenueSplit = serde_json::from_str(&json).unwrap();

        assert_eq!(split, deserialized);
    }
}
