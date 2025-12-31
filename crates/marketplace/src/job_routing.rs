//! Job routing and failover system for compute marketplace
//!
//! Implements provider selection algorithms, scoring, failover chains,
//! and retry logic for compute job routing.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during job routing
#[derive(Debug, Error)]
pub enum RoutingError {
    #[error("No providers available")]
    NoProvidersAvailable,

    #[error("No suitable provider found for request")]
    NoSuitableProvider,

    #[error("Selection failed: {0}")]
    SelectionFailed(String),

    #[error("Failover exhausted: {0}")]
    FailoverExhausted(String),

    #[error("Invalid criteria: {0}")]
    InvalidCriteria(String),
}

/// Provider selection criteria with weights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionCriteria {
    /// Weight for price consideration (0.0 to 1.0)
    pub price_weight: f32,

    /// Weight for latency consideration (0.0 to 1.0)
    pub latency_weight: f32,

    /// Weight for reliability consideration (0.0 to 1.0)
    pub reliability_weight: f32,

    /// Weight for reputation consideration (0.0 to 1.0)
    pub reputation_weight: f32,
}

impl Default for SelectionCriteria {
    fn default() -> Self {
        Self {
            price_weight: 0.30,
            latency_weight: 0.25,
            reliability_weight: 0.25,
            reputation_weight: 0.20,
        }
    }
}

impl SelectionCriteria {
    /// Create balanced criteria (default)
    pub fn balanced() -> Self {
        Self::default()
    }

    /// Create price-optimized criteria
    pub fn price_optimized() -> Self {
        Self {
            price_weight: 0.50,
            latency_weight: 0.20,
            reliability_weight: 0.20,
            reputation_weight: 0.10,
        }
    }

    /// Create performance-optimized criteria
    pub fn performance_optimized() -> Self {
        Self {
            price_weight: 0.10,
            latency_weight: 0.40,
            reliability_weight: 0.30,
            reputation_weight: 0.20,
        }
    }

    /// Create reliability-optimized criteria
    pub fn reliability_optimized() -> Self {
        Self {
            price_weight: 0.15,
            latency_weight: 0.20,
            reliability_weight: 0.40,
            reputation_weight: 0.25,
        }
    }

    /// Validate that weights sum to approximately 1.0
    pub fn validate(&self) -> Result<(), RoutingError> {
        let total = self.price_weight
            + self.latency_weight
            + self.reliability_weight
            + self.reputation_weight;

        if (total - 1.0).abs() > 0.01 {
            return Err(RoutingError::InvalidCriteria(format!(
                "Weights must sum to 1.0, got {}",
                total
            )));
        }

        Ok(())
    }
}

/// Provider score with component breakdowns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderScore {
    /// Provider ID
    pub provider_id: String,

    /// Total weighted score (0.0 to 1.0)
    pub total_score: f32,

    /// Price component score (0.0 to 1.0, higher = better value)
    pub price_score: f32,

    /// Latency component score (0.0 to 1.0, higher = faster)
    pub latency_score: f32,

    /// Reliability component score (0.0 to 1.0, higher = more reliable)
    pub reliability_score: f32,

    /// Reputation component score (0.0 to 1.0, higher = better reputation)
    pub reputation_score: f32,

    /// Estimated cost for the job in satoshis
    pub estimated_cost_sats: u64,
}

impl ProviderScore {
    /// Create a new provider score
    pub fn new(
        provider_id: impl Into<String>,
        criteria: &SelectionCriteria,
        price_score: f32,
        latency_score: f32,
        reliability_score: f32,
        reputation_score: f32,
        estimated_cost_sats: u64,
    ) -> Self {
        let total_score = price_score * criteria.price_weight
            + latency_score * criteria.latency_weight
            + reliability_score * criteria.reliability_weight
            + reputation_score * criteria.reputation_weight;

        Self {
            provider_id: provider_id.into(),
            total_score: total_score.clamp(0.0, 1.0),
            price_score,
            latency_score,
            reliability_score,
            reputation_score,
            estimated_cost_sats,
        }
    }

    /// Check if this provider meets a minimum score threshold
    pub fn meets_threshold(&self, threshold: f32) -> bool {
        self.total_score >= threshold
    }
}

/// Failover chain configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverChain {
    /// Primary provider ID
    pub primary: String,

    /// Fallback provider IDs in order of preference
    pub fallbacks: Vec<String>,

    /// Use platform provider as last resort
    #[serde(default)]
    pub platform_fallback: bool,
}

impl FailoverChain {
    /// Create a new failover chain
    pub fn new(primary: impl Into<String>) -> Self {
        Self {
            primary: primary.into(),
            fallbacks: Vec::new(),
            platform_fallback: false,
        }
    }

    /// Add a fallback provider
    pub fn with_fallback(mut self, provider_id: impl Into<String>) -> Self {
        self.fallbacks.push(provider_id.into());
        self
    }

    /// Add multiple fallback providers
    pub fn with_fallbacks(mut self, provider_ids: Vec<String>) -> Self {
        self.fallbacks = provider_ids;
        self
    }

    /// Enable platform fallback
    pub fn with_platform_fallback(mut self) -> Self {
        self.platform_fallback = true;
        self
    }

    /// Get all providers in order (primary, fallbacks, platform)
    pub fn all_providers(&self) -> Vec<String> {
        let mut providers = vec![self.primary.clone()];
        providers.extend(self.fallbacks.clone());
        if self.platform_fallback {
            providers.push("platform".to_string());
        }
        providers
    }

    /// Get the next provider after a given provider ID
    pub fn next_provider(&self, current: &str) -> Option<String> {
        let all = self.all_providers();
        all.iter()
            .position(|p| p == current)
            .and_then(|idx| all.get(idx + 1).cloned())
    }
}

/// Failover policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverPolicy {
    /// Maximum retry attempts per provider
    pub max_retries: u32,

    /// Base retry delay in milliseconds
    pub retry_delay_ms: u64,

    /// Escalate to next provider on failure
    pub escalate_on_failure: bool,

    /// Use exponential backoff for retries
    #[serde(default = "default_true")]
    pub exponential_backoff: bool,

    /// Maximum total execution time in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_total_time_ms: Option<u64>,
}

fn default_true() -> bool {
    true
}

impl Default for FailoverPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_ms: 1000,
            escalate_on_failure: true,
            exponential_backoff: true,
            max_total_time_ms: None,
        }
    }
}

impl FailoverPolicy {
    /// Create a new policy with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate retry delay for a given attempt
    pub fn calculate_delay(&self, attempt: u32) -> u64 {
        if self.exponential_backoff {
            self.retry_delay_ms * 2_u64.pow(attempt.saturating_sub(1))
        } else {
            self.retry_delay_ms
        }
    }

    /// Check if retries are exhausted
    pub fn retries_exhausted(&self, attempt: u32) -> bool {
        attempt >= self.max_retries
    }
}

/// Retry decision
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum RetryDecision {
    /// Retry with the same provider
    Retry {
        /// Delay before retry in milliseconds
        delay_ms: u64,
    },

    /// Fail over to next provider
    Failover {
        /// Next provider ID to try
        next_provider: String,
    },

    /// Abort the job
    Abort {
        /// Reason for aborting
        reason: String,
    },
}

impl RetryDecision {
    /// Create a retry decision
    pub fn retry(delay_ms: u64) -> Self {
        Self::Retry { delay_ms }
    }

    /// Create a failover decision
    pub fn failover(next_provider: impl Into<String>) -> Self {
        Self::Failover {
            next_provider: next_provider.into(),
        }
    }

    /// Create an abort decision
    pub fn abort(reason: impl Into<String>) -> Self {
        Self::Abort {
            reason: reason.into(),
        }
    }

    /// Check if this is a retry decision
    pub fn is_retry(&self) -> bool {
        matches!(self, Self::Retry { .. })
    }

    /// Check if this is a failover decision
    pub fn is_failover(&self) -> bool {
        matches!(self, Self::Failover { .. })
    }

    /// Check if this is an abort decision
    pub fn is_abort(&self) -> bool {
        matches!(self, Self::Abort { .. })
    }
}

/// Job error classification for retry logic
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobErrorType {
    /// Temporary network error (retryable)
    NetworkError,

    /// Provider timeout (retryable)
    Timeout,

    /// Provider capacity exceeded (failover)
    CapacityExceeded,

    /// Provider offline (failover)
    ProviderOffline,

    /// Invalid request (abort)
    InvalidRequest,

    /// Payment failed (abort)
    PaymentFailed,

    /// Unknown error (retry once)
    Unknown,
}

impl JobErrorType {
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::NetworkError | Self::Timeout | Self::Unknown)
    }

    /// Check if this error should trigger failover
    pub fn should_failover(&self) -> bool {
        matches!(self, Self::CapacityExceeded | Self::ProviderOffline)
    }

    /// Check if this error should abort the job
    pub fn should_abort(&self) -> bool {
        matches!(self, Self::InvalidRequest | Self::PaymentFailed)
    }
}

/// Determine retry decision based on error and policy
pub fn should_retry(
    error_type: JobErrorType,
    attempt: u32,
    policy: &FailoverPolicy,
    chain: &FailoverChain,
    current_provider: &str,
) -> RetryDecision {
    // Always abort for non-retryable errors
    if error_type.should_abort() {
        return RetryDecision::abort(format!("Non-retryable error: {:?}", error_type));
    }

    // Failover if error type suggests it
    if error_type.should_failover() && policy.escalate_on_failure {
        if let Some(next) = chain.next_provider(current_provider) {
            return RetryDecision::failover(next);
        } else {
            return RetryDecision::abort("No more failover providers available".to_string());
        }
    }

    // Check retry limit
    if policy.retries_exhausted(attempt) {
        // Try failover if enabled
        if policy.escalate_on_failure {
            if let Some(next) = chain.next_provider(current_provider) {
                return RetryDecision::failover(next);
            }
        }
        return RetryDecision::abort(format!("Max retries ({}) exceeded", policy.max_retries));
    }

    // Retry with delay
    let delay = policy.calculate_delay(attempt);
    RetryDecision::retry(delay)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_criteria_presets() {
        let balanced = SelectionCriteria::balanced();
        assert!(balanced.validate().is_ok());

        let price = SelectionCriteria::price_optimized();
        assert_eq!(price.price_weight, 0.50);

        let performance = SelectionCriteria::performance_optimized();
        assert_eq!(performance.latency_weight, 0.40);

        let reliability = SelectionCriteria::reliability_optimized();
        assert_eq!(reliability.reliability_weight, 0.40);
    }

    #[test]
    fn test_selection_criteria_validation() {
        let invalid = SelectionCriteria {
            price_weight: 0.50,
            latency_weight: 0.30,
            reliability_weight: 0.30,
            reputation_weight: 0.20,
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_provider_score_calculation() {
        let criteria = SelectionCriteria::balanced();
        let score = ProviderScore::new(
            "provider1",
            &criteria,
            0.8,  // price
            0.9,  // latency
            0.95, // reliability
            0.85, // reputation
            10000,
        );

        // Should be weighted average
        let expected = 0.8 * 0.30 + 0.9 * 0.25 + 0.95 * 0.25 + 0.85 * 0.20;
        assert!((score.total_score - expected).abs() < 0.01);
    }

    #[test]
    fn test_provider_score_threshold() {
        let criteria = SelectionCriteria::balanced();
        let score = ProviderScore::new("provider1", &criteria, 0.9, 0.9, 0.9, 0.9, 10000);

        assert!(score.meets_threshold(0.8));
        assert!(score.meets_threshold(0.9));
        assert!(!score.meets_threshold(0.95));
    }

    #[test]
    fn test_failover_chain() {
        let chain = FailoverChain::new("primary")
            .with_fallback("fallback1")
            .with_fallback("fallback2")
            .with_platform_fallback();

        let all = chain.all_providers();
        assert_eq!(all.len(), 4);
        assert_eq!(all[0], "primary");
        assert_eq!(all[3], "platform");

        assert_eq!(
            chain.next_provider("primary"),
            Some("fallback1".to_string())
        );
        assert_eq!(
            chain.next_provider("fallback2"),
            Some("platform".to_string())
        );
        assert_eq!(chain.next_provider("platform"), None);
    }

    #[test]
    fn test_failover_policy_delay() {
        let policy = FailoverPolicy::default();

        assert_eq!(policy.calculate_delay(1), 1000);
        assert_eq!(policy.calculate_delay(2), 2000);
        assert_eq!(policy.calculate_delay(3), 4000);

        let no_backoff = FailoverPolicy {
            exponential_backoff: false,
            ..Default::default()
        };
        assert_eq!(no_backoff.calculate_delay(1), 1000);
        assert_eq!(no_backoff.calculate_delay(3), 1000);
    }

    #[test]
    fn test_failover_policy_retries() {
        let policy = FailoverPolicy {
            max_retries: 3,
            ..Default::default()
        };

        assert!(!policy.retries_exhausted(0));
        assert!(!policy.retries_exhausted(2));
        assert!(policy.retries_exhausted(3));
        assert!(policy.retries_exhausted(5));
    }

    #[test]
    fn test_retry_decision_types() {
        let retry = RetryDecision::retry(1000);
        assert!(retry.is_retry());
        assert!(!retry.is_failover());
        assert!(!retry.is_abort());

        let failover = RetryDecision::failover("provider2");
        assert!(failover.is_failover());

        let abort = RetryDecision::abort("Failed");
        assert!(abort.is_abort());
    }

    #[test]
    fn test_job_error_type_classification() {
        assert!(JobErrorType::NetworkError.is_retryable());
        assert!(JobErrorType::Timeout.is_retryable());
        assert!(!JobErrorType::InvalidRequest.is_retryable());

        assert!(JobErrorType::CapacityExceeded.should_failover());
        assert!(JobErrorType::ProviderOffline.should_failover());

        assert!(JobErrorType::InvalidRequest.should_abort());
        assert!(JobErrorType::PaymentFailed.should_abort());
    }

    #[test]
    fn test_should_retry_abort_cases() {
        let policy = FailoverPolicy::default();
        let chain = FailoverChain::new("primary");

        let decision = should_retry(JobErrorType::InvalidRequest, 1, &policy, &chain, "primary");
        assert!(decision.is_abort());

        let decision = should_retry(JobErrorType::PaymentFailed, 1, &policy, &chain, "primary");
        assert!(decision.is_abort());
    }

    #[test]
    fn test_should_retry_failover_cases() {
        let policy = FailoverPolicy::default();
        let chain = FailoverChain::new("primary").with_fallback("fallback1");

        let decision = should_retry(
            JobErrorType::CapacityExceeded,
            1,
            &policy,
            &chain,
            "primary",
        );
        assert!(decision.is_failover());

        let decision = should_retry(JobErrorType::ProviderOffline, 1, &policy, &chain, "primary");
        assert!(decision.is_failover());
    }

    #[test]
    fn test_should_retry_retry_cases() {
        let policy = FailoverPolicy::default();
        let chain = FailoverChain::new("primary");

        let decision = should_retry(JobErrorType::NetworkError, 1, &policy, &chain, "primary");
        assert!(decision.is_retry());

        let decision = should_retry(JobErrorType::Timeout, 2, &policy, &chain, "primary");
        assert!(decision.is_retry());
    }

    #[test]
    fn test_should_retry_exhausted() {
        let policy = FailoverPolicy {
            max_retries: 2,
            ..Default::default()
        };
        let chain = FailoverChain::new("primary");

        let decision = should_retry(JobErrorType::NetworkError, 2, &policy, &chain, "primary");
        assert!(decision.is_abort());
    }

    #[test]
    fn test_should_retry_exhausted_with_failover() {
        let policy = FailoverPolicy {
            max_retries: 2,
            escalate_on_failure: true,
            ..Default::default()
        };
        let chain = FailoverChain::new("primary").with_fallback("fallback1");

        let decision = should_retry(JobErrorType::NetworkError, 2, &policy, &chain, "primary");
        assert!(decision.is_failover());
    }

    #[test]
    fn test_selection_criteria_serde() {
        let criteria = SelectionCriteria::performance_optimized();
        let json = serde_json::to_string(&criteria).unwrap();
        let deserialized: SelectionCriteria = serde_json::from_str(&json).unwrap();

        assert_eq!(criteria.latency_weight, deserialized.latency_weight);
    }

    #[test]
    fn test_provider_score_serde() {
        let criteria = SelectionCriteria::balanced();
        let score = ProviderScore::new("provider1", &criteria, 0.8, 0.9, 0.95, 0.85, 10000);

        let json = serde_json::to_string(&score).unwrap();
        let deserialized: ProviderScore = serde_json::from_str(&json).unwrap();

        assert_eq!(score.provider_id, deserialized.provider_id);
        assert_eq!(score.total_score, deserialized.total_score);
    }
}
