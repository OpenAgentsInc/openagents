//! FRLM policies for budget, timeout, quorum, and verification.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Main FRLM policy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrlmPolicy {
    /// Budget policy.
    pub budget: BudgetPolicy,
    /// Timeout policy.
    pub timeout: TimeoutPolicy,
    /// Quorum policy.
    pub quorum: QuorumPolicy,
    /// Verification tier.
    pub verification: VerificationTier,
    /// Whether to allow local fallback.
    pub allow_local_fallback: bool,
}

impl Default for FrlmPolicy {
    fn default() -> Self {
        Self {
            budget: BudgetPolicy::default(),
            timeout: TimeoutPolicy::default(),
            quorum: QuorumPolicy::default(),
            verification: VerificationTier::None,
            allow_local_fallback: true,
        }
    }
}

impl FrlmPolicy {
    /// Create a new policy with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the budget limit in satoshis.
    pub fn with_budget_sats(mut self, limit_sats: u64) -> Self {
        self.budget.limit_sats = limit_sats;
        self
    }

    /// Set the per-query budget limit in satoshis.
    pub fn with_per_query_budget_sats(mut self, limit_sats: u64) -> Self {
        self.budget.per_query_limit_sats = Some(limit_sats);
        self
    }

    /// Set the timeout duration.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout.total = timeout;
        self
    }

    /// Set the per-query timeout duration.
    pub fn with_per_query_timeout(mut self, timeout: Duration) -> Self {
        self.timeout.per_query = timeout;
        self
    }

    /// Set the quorum policy.
    pub fn with_quorum(mut self, quorum: Quorum) -> Self {
        self.quorum.quorum = quorum;
        self
    }

    /// Set the verification tier.
    pub fn with_verification(mut self, tier: VerificationTier) -> Self {
        self.verification = tier;
        self
    }

    /// Disable local fallback.
    pub fn no_local_fallback(mut self) -> Self {
        self.allow_local_fallback = false;
        self
    }
}

/// Budget policy for FRLM execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetPolicy {
    /// Total budget limit in satoshis.
    pub limit_sats: u64,
    /// Per-query budget limit in satoshis (optional).
    pub per_query_limit_sats: Option<u64>,
    /// Reserve multiplier for cost estimation.
    pub reserve_multiplier: f32,
}

impl Default for BudgetPolicy {
    fn default() -> Self {
        Self {
            limit_sats: 10_000, // 10k sats default
            per_query_limit_sats: None,
            reserve_multiplier: 1.5, // Reserve 50% extra for cost uncertainty
        }
    }
}

impl BudgetPolicy {
    /// Estimate cost for a query in satoshis.
    ///
    /// This is a rough estimate based on prompt length.
    pub fn estimate_cost(&self, prompt_len: usize) -> u64 {
        // Rough estimate: 1 sat per 100 chars
        // Actual cost depends on provider pricing
        let base_cost = (prompt_len / 100).max(1) as u64;
        (base_cost as f32 * self.reserve_multiplier) as u64
    }

    /// Check if budget allows a query with estimated cost.
    pub fn can_afford(&self, estimated_cost: u64, spent: u64) -> bool {
        spent + estimated_cost <= self.limit_sats
    }
}

/// Timeout policy for FRLM execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutPolicy {
    /// Total timeout for the entire run.
    pub total: Duration,
    /// Per-query timeout.
    pub per_query: Duration,
    /// Grace period before marking a query as straggler.
    pub straggler_threshold: Duration,
}

impl Default for TimeoutPolicy {
    fn default() -> Self {
        Self {
            total: Duration::from_secs(300),       // 5 minutes total
            per_query: Duration::from_secs(30),    // 30 seconds per query
            straggler_threshold: Duration::from_secs(10), // 10 second grace
        }
    }
}

/// Quorum policy for result collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuorumPolicy {
    /// The quorum requirement.
    pub quorum: Quorum,
    /// Whether to cancel stragglers after quorum is met.
    pub cancel_stragglers: bool,
}

impl Default for QuorumPolicy {
    fn default() -> Self {
        Self {
            quorum: Quorum::All,
            cancel_stragglers: true,
        }
    }
}

/// Quorum types for result collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Quorum {
    /// Wait for all results.
    All,
    /// Wait for at least N% of results.
    Fraction(f32),
    /// Wait for at least N results.
    MinCount(usize),
    /// Take whatever we have after timeout.
    BestEffort,
}

impl Quorum {
    /// Check if quorum is met given received and total counts.
    pub fn is_met(&self, received: usize, total: usize) -> bool {
        match self {
            Quorum::All => received >= total,
            Quorum::Fraction(f) => {
                if total == 0 {
                    true
                } else {
                    (received as f32 / total as f32) >= *f
                }
            }
            Quorum::MinCount(n) => received >= *n,
            Quorum::BestEffort => true, // Always met (timeout-based)
        }
    }

    /// Get the minimum required count for quorum.
    pub fn min_required(&self, total: usize) -> usize {
        match self {
            Quorum::All => total,
            Quorum::Fraction(f) => ((total as f32) * f).ceil() as usize,
            Quorum::MinCount(n) => *n,
            Quorum::BestEffort => 0,
        }
    }
}

/// Verification tier for sub-query results.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum VerificationTier {
    /// No verification - trust provider.
    #[default]
    None,

    /// Redundancy verification - run on N workers, require M agreement.
    Redundancy {
        /// Number of workers to use.
        n: usize,
        /// Minimum agreement required.
        m: usize,
        /// Similarity threshold for agreement (0.0 - 1.0).
        similarity_threshold: f32,
    },

    /// Objective verification - hash/schema check.
    Objective {
        /// Schema to validate against.
        schema: Option<String>,
    },

    /// Validator attestation required.
    Validated {
        /// Validator public key.
        validator_pubkey: String,
    },
}

impl VerificationTier {
    /// Create redundancy verification with default settings.
    pub fn redundancy(n: usize, m: usize) -> Self {
        Self::Redundancy {
            n,
            m,
            similarity_threshold: 0.8,
        }
    }

    /// Create 3-of-5 redundancy verification.
    pub fn redundancy_3_of_5() -> Self {
        Self::redundancy(5, 3)
    }

    /// Create 2-of-3 redundancy verification.
    pub fn redundancy_2_of_3() -> Self {
        Self::redundancy(3, 2)
    }

    /// Check if this tier requires redundant execution.
    pub fn requires_redundancy(&self) -> bool {
        matches!(self, Self::Redundancy { .. })
    }

    /// Get the number of executions required.
    pub fn execution_count(&self) -> usize {
        match self {
            Self::Redundancy { n, .. } => *n,
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quorum_all() {
        let q = Quorum::All;
        assert!(!q.is_met(4, 5));
        assert!(q.is_met(5, 5));
        assert_eq!(q.min_required(5), 5);
    }

    #[test]
    fn test_quorum_fraction() {
        let q = Quorum::Fraction(0.6);
        assert!(!q.is_met(2, 5));
        assert!(q.is_met(3, 5));
        assert_eq!(q.min_required(5), 3);
    }

    #[test]
    fn test_quorum_min_count() {
        let q = Quorum::MinCount(3);
        assert!(!q.is_met(2, 10));
        assert!(q.is_met(3, 10));
        assert_eq!(q.min_required(10), 3);
    }

    #[test]
    fn test_budget_estimate() {
        let policy = BudgetPolicy::default();
        let cost = policy.estimate_cost(1000);
        assert!(cost > 0);
    }

    #[test]
    fn test_verification_tier() {
        let tier = VerificationTier::redundancy_2_of_3();
        assert!(tier.requires_redundancy());
        assert_eq!(tier.execution_count(), 3);
    }
}
