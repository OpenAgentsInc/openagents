//! Core types for the marketplace

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
            SkillPricing::PerToken {
                per_1k_input,
                per_1k_output,
            } => {
                let input_cost = (input_tokens * per_1k_input).div_ceil(1000); // Round up
                let output_cost = (output_tokens * per_1k_output).div_ceil(1000); // Round up
                input_cost + output_cost
            }
            SkillPricing::Hybrid {
                per_call,
                per_1k_input,
                per_1k_output,
            } => {
                let input_cost = (input_tokens * per_1k_input).div_ceil(1000); // Round up
                let output_cost = (output_tokens * per_1k_output).div_ceil(1000); // Round up
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

/// Status of a skill submission in the publishing workflow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSubmissionStatus {
    /// Still being edited by creator
    Draft,
    /// Submitted and awaiting review assignment
    PendingReview,
    /// Currently being reviewed
    InReview,
    /// Reviewer requested changes
    ChangesRequested,
    /// Approved by reviewer
    Approved,
    /// Published to marketplace
    Published,
    /// No longer recommended for use
    Deprecated,
    /// Rejected and will not be published
    Rejected,
}

impl SkillSubmissionStatus {
    /// Get the status as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            SkillSubmissionStatus::Draft => "draft",
            SkillSubmissionStatus::PendingReview => "pending_review",
            SkillSubmissionStatus::InReview => "in_review",
            SkillSubmissionStatus::ChangesRequested => "changes_requested",
            SkillSubmissionStatus::Approved => "approved",
            SkillSubmissionStatus::Published => "published",
            SkillSubmissionStatus::Deprecated => "deprecated",
            SkillSubmissionStatus::Rejected => "rejected",
        }
    }

    /// Check if the submission is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            SkillSubmissionStatus::Published
                | SkillSubmissionStatus::Deprecated
                | SkillSubmissionStatus::Rejected
        )
    }

    /// Check if the submission can be edited
    pub fn is_editable(&self) -> bool {
        matches!(
            self,
            SkillSubmissionStatus::Draft | SkillSubmissionStatus::ChangesRequested
        )
    }
}

/// A skill submission for review and publishing
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSubmission {
    /// Unique submission ID
    pub id: String,
    /// Path to local skill directory
    pub skill_path: PathBuf,
    /// Creator's Nostr public key (hex format)
    pub creator: String,
    /// Current status
    pub status: SkillSubmissionStatus,
    /// When submitted
    pub submitted_at: DateTime<Utc>,
    /// When last updated
    pub updated_at: DateTime<Utc>,
    /// Review feedback if any
    pub feedback: Option<String>,
}

/// Marketplace item installation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    /// Available to install
    Available,
    /// Currently installed
    Installed,
    /// Has an update available
    UpdateAvailable,
    /// Deprecated, should be replaced
    Deprecated,
}

impl ItemStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemStatus::Available => "available",
            ItemStatus::Installed => "installed",
            ItemStatus::UpdateAvailable => "update_available",
            ItemStatus::Deprecated => "deprecated",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pricing_free() {
        let pricing = SkillPricing::Free;
        assert_eq!(pricing.calculate_cost(0, 0), 0);
        assert_eq!(pricing.calculate_cost(1000, 500), 0);
        assert_eq!(pricing.calculate_cost(999999, 999999), 0);
    }

    #[test]
    fn test_pricing_per_call() {
        let pricing = SkillPricing::PerCall { credits: 100 };

        // Cost is always the same regardless of tokens
        assert_eq!(pricing.calculate_cost(0, 0), 100);
        assert_eq!(pricing.calculate_cost(1000, 500), 100);
        assert_eq!(pricing.calculate_cost(999999, 999999), 100);
    }

    #[test]
    fn test_pricing_per_token_exact_thousands() {
        let pricing = SkillPricing::PerToken {
            per_1k_input: 10,
            per_1k_output: 20,
        };

        // Exact multiples of 1000
        assert_eq!(pricing.calculate_cost(1000, 1000), 30); // 10 + 20
        assert_eq!(pricing.calculate_cost(2000, 3000), 80); // 20 + 60
        assert_eq!(pricing.calculate_cost(5000, 2000), 90); // 50 + 40
    }

    #[test]
    fn test_pricing_per_token_rounds_up() {
        let pricing = SkillPricing::PerToken {
            per_1k_input: 10,
            per_1k_output: 20,
        };

        // Should round up (div_ceil)
        // (1 * 10).div_ceil(1000) = 1, (1 * 20).div_ceil(1000) = 1
        assert_eq!(pricing.calculate_cost(1, 1), 2); // Min cost: 1 + 1
        assert_eq!(pricing.calculate_cost(500, 500), 15); // 5 + 10 (rounds up)
        assert_eq!(pricing.calculate_cost(1001, 1001), 32); // 11 + 21 (rounds up)
        assert_eq!(pricing.calculate_cost(1500, 2500), 65); // 15 + 50
    }

    #[test]
    fn test_pricing_per_token_zero_tokens() {
        let pricing = SkillPricing::PerToken {
            per_1k_input: 10,
            per_1k_output: 20,
        };

        assert_eq!(pricing.calculate_cost(0, 0), 0);
        assert_eq!(pricing.calculate_cost(0, 1000), 20);
        assert_eq!(pricing.calculate_cost(1000, 0), 10);
    }

    #[test]
    fn test_pricing_hybrid() {
        let pricing = SkillPricing::Hybrid {
            per_call: 50,
            per_1k_input: 5,
            per_1k_output: 10,
        };

        // Base cost + token costs
        assert_eq!(pricing.calculate_cost(0, 0), 50);
        assert_eq!(pricing.calculate_cost(1000, 1000), 65); // 50 + 5 + 10
        assert_eq!(pricing.calculate_cost(2000, 3000), 90); // 50 + 10 + 30
    }

    #[test]
    fn test_pricing_hybrid_rounds_up() {
        let pricing = SkillPricing::Hybrid {
            per_call: 100,
            per_1k_input: 10,
            per_1k_output: 20,
        };

        // Rounds up like PerToken
        assert_eq!(pricing.calculate_cost(1, 1), 102); // 100 + 1 + 1
        assert_eq!(pricing.calculate_cost(500, 500), 115); // 100 + 5 + 10
        assert_eq!(pricing.calculate_cost(1500, 2500), 165); // 100 + 15 + 50
    }

    #[test]
    fn test_revenue_split_default_valid() {
        let split = RevenueSplit::DEFAULT;
        assert!(split.is_valid());
        assert_eq!(split.creator_pct, 60);
        assert_eq!(split.compute_pct, 25);
        assert_eq!(split.platform_pct, 10);
        assert_eq!(split.referrer_pct, 5);
    }

    #[test]
    fn test_revenue_split_validation() {
        // Valid splits
        assert!(
            RevenueSplit {
                creator_pct: 50,
                compute_pct: 30,
                platform_pct: 15,
                referrer_pct: 5,
            }
            .is_valid()
        );

        assert!(
            RevenueSplit {
                creator_pct: 70,
                compute_pct: 20,
                platform_pct: 5,
                referrer_pct: 5,
            }
            .is_valid()
        );

        // Invalid splits (don't sum to 100)
        assert!(
            !RevenueSplit {
                creator_pct: 60,
                compute_pct: 25,
                platform_pct: 10,
                referrer_pct: 10, // Total is 105
            }
            .is_valid()
        );

        assert!(
            !RevenueSplit {
                creator_pct: 50,
                compute_pct: 25,
                platform_pct: 10,
                referrer_pct: 5, // Total is 90
            }
            .is_valid()
        );

        assert!(
            !RevenueSplit {
                creator_pct: 0,
                compute_pct: 0,
                platform_pct: 0,
                referrer_pct: 0, // Total is 0
            }
            .is_valid()
        );
    }

    #[test]
    fn test_revenue_split_calculation() {
        let split = RevenueSplit::DEFAULT; // 60/25/10/5

        let (creator, compute, platform, referrer) = split.split(1000);
        assert_eq!(creator, 600);
        assert_eq!(compute, 250);
        assert_eq!(platform, 100);
        assert_eq!(referrer, 50);

        // Verify sum equals original
        assert_eq!(creator + compute + platform + referrer, 1000);
    }

    #[test]
    fn test_revenue_split_rounding() {
        let split = RevenueSplit::DEFAULT; // 60/25/10/5

        // Test with amount that doesn't divide evenly
        let (creator, compute, platform, referrer) = split.split(1003);

        // Integer division rounds down, remainder goes to referrer
        assert_eq!(creator, 601); // 1003 * 60 / 100 = 601
        assert_eq!(compute, 250); // 1003 * 25 / 100 = 250
        assert_eq!(platform, 100); // 1003 * 10 / 100 = 100

        // Referrer gets remainder
        assert_eq!(creator + compute + platform + referrer, 1003);
    }

    #[test]
    fn test_revenue_split_zero_amount() {
        let split = RevenueSplit::DEFAULT;
        let (creator, compute, platform, referrer) = split.split(0);

        assert_eq!(creator, 0);
        assert_eq!(compute, 0);
        assert_eq!(platform, 0);
        assert_eq!(referrer, 0);
    }

    #[test]
    fn test_revenue_split_small_amount() {
        let split = RevenueSplit::DEFAULT;

        // Amount smaller than percentages
        let (creator, compute, platform, referrer) = split.split(10);
        assert_eq!(creator, 6); // 60%
        assert_eq!(compute, 2); // 25%
        assert_eq!(platform, 1); // 10%
        assert_eq!(referrer, 1); // Remainder

        assert_eq!(creator + compute + platform + referrer, 10);
    }

    #[test]
    fn test_revenue_split_custom() {
        let split = RevenueSplit {
            creator_pct: 70,
            compute_pct: 20,
            platform_pct: 5,
            referrer_pct: 5,
        };

        let (creator, compute, platform, referrer) = split.split(1000);
        assert_eq!(creator, 700);
        assert_eq!(compute, 200);
        assert_eq!(platform, 50);
        assert_eq!(referrer, 50);
        assert_eq!(creator + compute + platform + referrer, 1000);
    }

    #[test]
    fn test_submission_status_as_str() {
        assert_eq!(SkillSubmissionStatus::Draft.as_str(), "draft");
        assert_eq!(
            SkillSubmissionStatus::PendingReview.as_str(),
            "pending_review"
        );
        assert_eq!(SkillSubmissionStatus::InReview.as_str(), "in_review");
        assert_eq!(
            SkillSubmissionStatus::ChangesRequested.as_str(),
            "changes_requested"
        );
        assert_eq!(SkillSubmissionStatus::Approved.as_str(), "approved");
        assert_eq!(SkillSubmissionStatus::Published.as_str(), "published");
        assert_eq!(SkillSubmissionStatus::Deprecated.as_str(), "deprecated");
        assert_eq!(SkillSubmissionStatus::Rejected.as_str(), "rejected");
    }

    #[test]
    fn test_submission_status_is_terminal() {
        // Terminal states
        assert!(SkillSubmissionStatus::Published.is_terminal());
        assert!(SkillSubmissionStatus::Deprecated.is_terminal());
        assert!(SkillSubmissionStatus::Rejected.is_terminal());

        // Non-terminal states
        assert!(!SkillSubmissionStatus::Draft.is_terminal());
        assert!(!SkillSubmissionStatus::PendingReview.is_terminal());
        assert!(!SkillSubmissionStatus::InReview.is_terminal());
        assert!(!SkillSubmissionStatus::ChangesRequested.is_terminal());
        assert!(!SkillSubmissionStatus::Approved.is_terminal());
    }

    #[test]
    fn test_submission_status_is_editable() {
        // Editable states
        assert!(SkillSubmissionStatus::Draft.is_editable());
        assert!(SkillSubmissionStatus::ChangesRequested.is_editable());

        // Non-editable states
        assert!(!SkillSubmissionStatus::PendingReview.is_editable());
        assert!(!SkillSubmissionStatus::InReview.is_editable());
        assert!(!SkillSubmissionStatus::Approved.is_editable());
        assert!(!SkillSubmissionStatus::Published.is_editable());
        assert!(!SkillSubmissionStatus::Deprecated.is_editable());
        assert!(!SkillSubmissionStatus::Rejected.is_editable());
    }

    #[test]
    fn test_item_status_as_str() {
        assert_eq!(ItemStatus::Available.as_str(), "available");
        assert_eq!(ItemStatus::Installed.as_str(), "installed");
        assert_eq!(ItemStatus::UpdateAvailable.as_str(), "update_available");
        assert_eq!(ItemStatus::Deprecated.as_str(), "deprecated");
    }
}
