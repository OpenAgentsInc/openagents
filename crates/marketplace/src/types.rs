//! Core types for the marketplace

use serde::{Deserialize, Serialize};

/// Pricing model for marketplace skills
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SkillPricing {
    /// Free skill - no cost
    Free,
    /// Fixed cost per call
    PerCall {
        /// Credits charged per call
        credits: u64,
    },
    /// Cost based on token usage
    PerToken {
        /// Credits per 1K input tokens
        per_1k_input: u64,
        /// Credits per 1K output tokens
        per_1k_output: u64,
    },
    /// Combined fixed and token-based pricing
    Hybrid {
        /// Fixed credits per call
        per_call: u64,
        /// Credits per 1K input tokens
        per_1k_input: u64,
        /// Credits per 1K output tokens
        per_1k_output: u64,
    },
}

impl SkillPricing {
    /// Calculate the cost for a skill invocation
    ///
    /// # Arguments
    /// * `input_tokens` - Number of input tokens used
    /// * `output_tokens` - Number of output tokens generated
    ///
    /// # Returns
    /// Total cost in credits
    pub fn calculate_cost(&self, input_tokens: u64, output_tokens: u64) -> u64 {
        match self {
            SkillPricing::Free => 0,
            SkillPricing::PerCall { credits } => *credits,
            SkillPricing::PerToken { per_1k_input, per_1k_output } => {
                let input_cost = (input_tokens * per_1k_input + 999) / 1000; // Round up
                let output_cost = (output_tokens * per_1k_output + 999) / 1000; // Round up
                input_cost + output_cost
            }
            SkillPricing::Hybrid { per_call, per_1k_input, per_1k_output } => {
                let input_cost = (input_tokens * per_1k_input + 999) / 1000; // Round up
                let output_cost = (output_tokens * per_1k_output + 999) / 1000; // Round up
                per_call + input_cost + output_cost
            }
        }
    }
}

/// Revenue split configuration for skill payments
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevenueSplit {
    /// Percentage for skill creator (0-100)
    pub creator_pct: u8,
    /// Percentage for compute provider (0-100)
    pub compute_pct: u8,
    /// Percentage for platform (0-100)
    pub platform_pct: u8,
    /// Percentage for referrer (0-100)
    pub referrer_pct: u8,
}

impl RevenueSplit {
    /// Default revenue split
    pub const DEFAULT: Self = Self {
        creator_pct: 60,
        compute_pct: 25,
        platform_pct: 10,
        referrer_pct: 5,
    };

    /// Validate that the percentages sum to 100
    pub fn is_valid(&self) -> bool {
        self.creator_pct as u16
            + self.compute_pct as u16
            + self.platform_pct as u16
            + self.referrer_pct as u16
            == 100
    }

    /// Calculate the split amounts for a given total
    ///
    /// # Arguments
    /// * `total_credits` - Total credits to split
    ///
    /// # Returns
    /// Tuple of (creator, compute, platform, referrer) amounts
    pub fn split(&self, total_credits: u64) -> (u64, u64, u64, u64) {
        let creator = (total_credits * self.creator_pct as u64) / 100;
        let compute = (total_credits * self.compute_pct as u64) / 100;
        let platform = (total_credits * self.platform_pct as u64) / 100;
        let referrer = total_credits.saturating_sub(creator + compute + platform); // Remainder goes to referrer
        (creator, compute, platform, referrer)
    }
}

impl Default for RevenueSplit {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Type of marketplace item
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarketplaceItemType {
    /// A plugin (extends functionality via MCP)
    Plugin,
    /// A skill (predefined task template)
    Skill,
    /// An agent (autonomous worker)
    Agent,
}

impl MarketplaceItemType {
    /// Get the item type as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            MarketplaceItemType::Plugin => "plugin",
            MarketplaceItemType::Skill => "skill",
            MarketplaceItemType::Agent => "agent",
        }
    }
}

/// Status of a marketplace item
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemStatus {
    /// Available for installation
    Available,
    /// Currently being installed
    Installing,
    /// Successfully installed
    Installed,
    /// Installation failed
    Failed,
    /// Update available
    UpdateAvailable,
}

impl ItemStatus {
    /// Get the status as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemStatus::Available => "available",
            ItemStatus::Installing => "installing",
            ItemStatus::Installed => "installed",
            ItemStatus::Failed => "failed",
            ItemStatus::UpdateAvailable => "update_available",
        }
    }

    /// Check if the item is installed
    pub fn is_installed(&self) -> bool {
        matches!(self, ItemStatus::Installed | ItemStatus::UpdateAvailable)
    }

    /// Check if the item is in a transitional state
    pub fn is_transitional(&self) -> bool {
        matches!(self, ItemStatus::Installing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_pricing_free() {
        let pricing = SkillPricing::Free;
        assert_eq!(pricing.calculate_cost(100, 200), 0);
        assert_eq!(pricing.calculate_cost(0, 0), 0);
    }

    #[test]
    fn test_skill_pricing_per_call() {
        let pricing = SkillPricing::PerCall { credits: 100 };
        assert_eq!(pricing.calculate_cost(100, 200), 100);
        assert_eq!(pricing.calculate_cost(0, 0), 100);
        assert_eq!(pricing.calculate_cost(1000, 1000), 100);
    }

    #[test]
    fn test_skill_pricing_per_token() {
        let pricing = SkillPricing::PerToken {
            per_1k_input: 10,
            per_1k_output: 20,
        };

        // 1000 input tokens @ 10 credits/1k = 10 credits
        // 1000 output tokens @ 20 credits/1k = 20 credits
        // Total = 30 credits
        assert_eq!(pricing.calculate_cost(1000, 1000), 30);

        // 500 input tokens @ 10 credits/1k = 5 credits (rounds up to 5)
        // 500 output tokens @ 20 credits/1k = 10 credits (rounds up to 10)
        // Total = 15 credits
        assert_eq!(pricing.calculate_cost(500, 500), 15);

        // 1 input token @ 10 credits/1k = 1 credit (rounds up)
        // 1 output token @ 20 credits/1k = 1 credit (rounds up)
        // Total = 2 credits
        assert_eq!(pricing.calculate_cost(1, 1), 2);

        // 0 tokens = 0 credits
        assert_eq!(pricing.calculate_cost(0, 0), 0);
    }

    #[test]
    fn test_skill_pricing_hybrid() {
        let pricing = SkillPricing::Hybrid {
            per_call: 50,
            per_1k_input: 10,
            per_1k_output: 20,
        };

        // 50 (base) + 10 (1000 input tokens) + 20 (1000 output tokens) = 80 credits
        assert_eq!(pricing.calculate_cost(1000, 1000), 80);

        // 50 (base) + 5 (500 input tokens) + 10 (500 output tokens) = 65 credits
        assert_eq!(pricing.calculate_cost(500, 500), 65);

        // 50 (base) + 0 (0 tokens) = 50 credits
        assert_eq!(pricing.calculate_cost(0, 0), 50);
    }

    #[test]
    fn test_revenue_split_default() {
        let split = RevenueSplit::default();
        assert_eq!(split.creator_pct, 60);
        assert_eq!(split.compute_pct, 25);
        assert_eq!(split.platform_pct, 10);
        assert_eq!(split.referrer_pct, 5);
        assert!(split.is_valid());
    }

    #[test]
    fn test_revenue_split_validation() {
        let valid = RevenueSplit {
            creator_pct: 60,
            compute_pct: 25,
            platform_pct: 10,
            referrer_pct: 5,
        };
        assert!(valid.is_valid());

        let invalid = RevenueSplit {
            creator_pct: 60,
            compute_pct: 25,
            platform_pct: 10,
            referrer_pct: 10, // Sums to 105
        };
        assert!(!invalid.is_valid());
    }

    #[test]
    fn test_revenue_split_calculation() {
        let split = RevenueSplit::DEFAULT;

        // 100 credits total
        let (creator, compute, platform, referrer) = split.split(100);
        assert_eq!(creator, 60);
        assert_eq!(compute, 25);
        assert_eq!(platform, 10);
        assert_eq!(referrer, 5);

        // Verify total adds up
        assert_eq!(creator + compute + platform + referrer, 100);

        // 1000 credits total
        let (creator, compute, platform, referrer) = split.split(1000);
        assert_eq!(creator, 600);
        assert_eq!(compute, 250);
        assert_eq!(platform, 100);
        assert_eq!(referrer, 50);

        // Verify total adds up
        assert_eq!(creator + compute + platform + referrer, 1000);
    }

    #[test]
    fn test_revenue_split_rounding() {
        let split = RevenueSplit::DEFAULT;

        // With 99 credits, rounding might cause issues
        // 60% of 99 = 59.4 -> 59
        // 25% of 99 = 24.75 -> 24
        // 10% of 99 = 9.9 -> 9
        // Referrer gets remainder = 99 - 59 - 24 - 9 = 7
        let (creator, compute, platform, referrer) = split.split(99);
        assert_eq!(creator, 59);
        assert_eq!(compute, 24);
        assert_eq!(platform, 9);
        assert_eq!(referrer, 7); // Gets the rounding remainder

        // Verify total adds up exactly
        assert_eq!(creator + compute + platform + referrer, 99);
    }

    #[test]
    fn test_skill_pricing_serde() {
        let pricing = SkillPricing::Hybrid {
            per_call: 50,
            per_1k_input: 10,
            per_1k_output: 20,
        };

        let json = serde_json::to_string(&pricing).unwrap();
        let deserialized: SkillPricing = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, pricing);
    }

    #[test]
    fn test_revenue_split_serde() {
        let split = RevenueSplit::DEFAULT;
        let json = serde_json::to_string(&split).unwrap();
        let deserialized: RevenueSplit = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, split);
    }

    #[test]
    fn test_item_type_as_str() {
        assert_eq!(MarketplaceItemType::Plugin.as_str(), "plugin");
        assert_eq!(MarketplaceItemType::Skill.as_str(), "skill");
        assert_eq!(MarketplaceItemType::Agent.as_str(), "agent");
    }

    #[test]
    fn test_item_status_as_str() {
        assert_eq!(ItemStatus::Available.as_str(), "available");
        assert_eq!(ItemStatus::Installing.as_str(), "installing");
        assert_eq!(ItemStatus::Installed.as_str(), "installed");
        assert_eq!(ItemStatus::Failed.as_str(), "failed");
        assert_eq!(ItemStatus::UpdateAvailable.as_str(), "update_available");
    }

    #[test]
    fn test_item_status_is_installed() {
        assert!(!ItemStatus::Available.is_installed());
        assert!(!ItemStatus::Installing.is_installed());
        assert!(ItemStatus::Installed.is_installed());
        assert!(!ItemStatus::Failed.is_installed());
        assert!(ItemStatus::UpdateAvailable.is_installed());
    }

    #[test]
    fn test_item_status_is_transitional() {
        assert!(!ItemStatus::Available.is_transitional());
        assert!(ItemStatus::Installing.is_transitional());
        assert!(!ItemStatus::Installed.is_transitional());
        assert!(!ItemStatus::Failed.is_transitional());
        assert!(!ItemStatus::UpdateAvailable.is_transitional());
    }

    #[test]
    fn test_serde_roundtrip() {
        let item_type = MarketplaceItemType::Skill;
        let json = serde_json::to_string(&item_type).unwrap();
        assert_eq!(json, "\"skill\"");
        let deserialized: MarketplaceItemType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, item_type);

        let status = ItemStatus::Installed;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"installed\"");
        let deserialized: ItemStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);
    }
}
