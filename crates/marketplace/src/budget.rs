//! Budget control and spending tracking types
//!
//! This module provides comprehensive budget management for marketplace spending,
//! including daily/monthly caps, spending tracking, cost estimation, and overage handling.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during budget operations
#[derive(Debug, Clone, Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetError {
    #[error("Budget exceeded: {0}")]
    BudgetExceeded(String),

    #[error("Invalid budget configuration: {0}")]
    InvalidConfig(String),

    #[error("Invalid threshold: {0}")]
    InvalidThreshold(String),

    #[error("Requires approval: {0}")]
    RequiresApproval(String),

    #[error("Budget blocked: {0}")]
    Blocked(String),
}

/// Action to take when a budget alert threshold is reached
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertAction {
    /// Send notification but allow spending
    Notify,

    /// Require manual approval before proceeding
    RequireApproval,

    /// Pause all spending
    Pause,
}

/// Alert threshold configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertThreshold {
    /// Percentage of budget (0-100)
    pub percentage: u8,

    /// Action to take at this threshold
    pub action: AlertAction,
}

impl AlertThreshold {
    /// Create a new alert threshold
    pub fn new(percentage: u8, action: AlertAction) -> Result<Self, BudgetError> {
        if percentage > 100 {
            return Err(BudgetError::InvalidThreshold(
                "Percentage must be 0-100".to_string(),
            ));
        }

        Ok(Self { percentage, action })
    }

    /// Check if this threshold has been reached
    pub fn is_reached(&self, spent: u64, limit: u64) -> bool {
        if limit == 0 {
            return false;
        }
        let spent_pct = (spent as f64 / limit as f64 * 100.0) as u8;
        spent_pct >= self.percentage
    }
}

/// Budget configuration for an account
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetConfig {
    /// Account or organization ID
    pub account_id: String,

    /// Daily spending cap in satoshis
    pub daily_cap_sats: Option<u64>,

    /// Per-job spending cap in satoshis
    pub per_job_cap_sats: Option<u64>,

    /// Monthly spending cap in satoshis
    pub monthly_cap_sats: Option<u64>,

    /// Automatically stop spending when budget exhausted
    pub auto_stop_on_exhaust: bool,

    /// Alert thresholds and actions
    pub alert_thresholds: Vec<AlertThreshold>,
}

impl BudgetConfig {
    /// Create a new budget configuration
    pub fn new(account_id: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            daily_cap_sats: None,
            per_job_cap_sats: None,
            monthly_cap_sats: None,
            auto_stop_on_exhaust: true,
            alert_thresholds: Vec::new(),
        }
    }

    /// Set daily cap
    pub fn with_daily_cap(mut self, cap_sats: u64) -> Self {
        self.daily_cap_sats = Some(cap_sats);
        self
    }

    /// Set per-job cap
    pub fn with_per_job_cap(mut self, cap_sats: u64) -> Self {
        self.per_job_cap_sats = Some(cap_sats);
        self
    }

    /// Set monthly cap
    pub fn with_monthly_cap(mut self, cap_sats: u64) -> Self {
        self.monthly_cap_sats = Some(cap_sats);
        self
    }

    /// Set auto-stop behavior
    pub fn with_auto_stop(mut self, enabled: bool) -> Self {
        self.auto_stop_on_exhaust = enabled;
        self
    }

    /// Add an alert threshold
    pub fn add_threshold(mut self, threshold: AlertThreshold) -> Self {
        self.alert_thresholds.push(threshold);
        self
    }

    /// Validate the budget configuration
    pub fn validate(&self) -> Result<(), BudgetError> {
        // Check that at least one cap is set
        if self.daily_cap_sats.is_none()
            && self.per_job_cap_sats.is_none()
            && self.monthly_cap_sats.is_none()
        {
            return Err(BudgetError::InvalidConfig(
                "At least one budget cap must be set".to_string(),
            ));
        }

        // Check that thresholds are valid
        for threshold in &self.alert_thresholds {
            if threshold.percentage > 100 {
                return Err(BudgetError::InvalidThreshold(
                    "Percentage must be 0-100".to_string(),
                ));
            }
        }

        Ok(())
    }
}

/// Time period for budget tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetPeriod {
    /// Daily budget period
    Daily,

    /// Weekly budget period
    Weekly,

    /// Monthly budget period
    Monthly,
}

impl BudgetPeriod {
    /// Get human-readable description
    pub fn description(&self) -> &str {
        match self {
            BudgetPeriod::Daily => "Daily",
            BudgetPeriod::Weekly => "Weekly",
            BudgetPeriod::Monthly => "Monthly",
        }
    }
}

/// Spending tracker for an account
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpendingTracker {
    /// Account ID
    pub account_id: String,

    /// Budget period being tracked
    pub period: BudgetPeriod,

    /// Amount spent in this period
    pub spent_sats: u64,

    /// Budget limit for this period
    pub limit_sats: u64,

    /// Remaining budget
    pub remaining_sats: u64,

    /// Number of jobs executed
    pub jobs_count: u32,

    /// Period start time
    pub period_start: DateTime<Utc>,

    /// Period end time
    pub period_end: DateTime<Utc>,
}

impl SpendingTracker {
    /// Create a new spending tracker
    pub fn new(
        account_id: impl Into<String>,
        period: BudgetPeriod,
        limit_sats: u64,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> Self {
        Self {
            account_id: account_id.into(),
            period,
            spent_sats: 0,
            limit_sats,
            remaining_sats: limit_sats,
            jobs_count: 0,
            period_start,
            period_end,
        }
    }

    /// Record spending
    pub fn record_spend(&mut self, amount_sats: u64) -> Result<(), BudgetError> {
        if amount_sats > self.remaining_sats {
            return Err(BudgetError::BudgetExceeded(format!(
                "Spend {} exceeds remaining budget {}",
                amount_sats, self.remaining_sats
            )));
        }

        self.spent_sats += amount_sats;
        self.remaining_sats = self.limit_sats.saturating_sub(self.spent_sats);
        self.jobs_count += 1;
        Ok(())
    }

    /// Get spending percentage
    pub fn spent_percentage(&self) -> f64 {
        if self.limit_sats == 0 {
            return 0.0;
        }
        (self.spent_sats as f64 / self.limit_sats as f64) * 100.0
    }

    /// Check if budget is exhausted
    pub fn is_exhausted(&self) -> bool {
        self.remaining_sats == 0
    }

    /// Check if period is active
    pub fn is_active(&self) -> bool {
        let now = Utc::now();
        now >= self.period_start && now <= self.period_end
    }
}

/// Result of a budget check
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetCheckResult {
    /// Spending is allowed
    Allowed,

    /// Requires manual approval
    RequiresApproval { reason: String },

    /// Spending is blocked
    Blocked { reason: String },
}

impl BudgetCheckResult {
    /// Check if spending is allowed
    pub fn is_allowed(&self) -> bool {
        matches!(self, BudgetCheckResult::Allowed)
    }

    /// Check if approval is required
    pub fn requires_approval(&self) -> bool {
        matches!(self, BudgetCheckResult::RequiresApproval { .. })
    }

    /// Check if spending is blocked
    pub fn is_blocked(&self) -> bool {
        matches!(self, BudgetCheckResult::Blocked { .. })
    }
}

/// Check if proposed spending is within budget
pub fn check_budget(
    config: &BudgetConfig,
    tracker: &SpendingTracker,
    proposed_spend: u64,
) -> BudgetCheckResult {
    // Check per-job cap
    if let Some(per_job_cap) = config.per_job_cap_sats {
        if proposed_spend > per_job_cap {
            return BudgetCheckResult::Blocked {
                reason: format!(
                    "Job cost {} exceeds per-job cap {}",
                    proposed_spend, per_job_cap
                ),
            };
        }
    }

    // Check if proposed spend exceeds remaining budget
    if proposed_spend > tracker.remaining_sats {
        if config.auto_stop_on_exhaust {
            return BudgetCheckResult::Blocked {
                reason: format!(
                    "Insufficient budget: {} needed, {} remaining",
                    proposed_spend, tracker.remaining_sats
                ),
            };
        } else {
            return BudgetCheckResult::RequiresApproval {
                reason: format!(
                    "Exceeds budget by {} sats",
                    proposed_spend - tracker.remaining_sats
                ),
            };
        }
    }

    // Check alert thresholds
    let projected_spent = tracker.spent_sats + proposed_spend;
    for threshold in &config.alert_thresholds {
        if threshold.is_reached(projected_spent, tracker.limit_sats) {
            match threshold.action {
                AlertAction::Notify => continue,
                AlertAction::RequireApproval => {
                    return BudgetCheckResult::RequiresApproval {
                        reason: format!("Reached {}% of budget threshold", threshold.percentage),
                    };
                }
                AlertAction::Pause => {
                    return BudgetCheckResult::Blocked {
                        reason: format!("Budget paused at {}% threshold", threshold.percentage),
                    };
                }
            }
        }
    }

    BudgetCheckResult::Allowed
}

/// Impact of a job on the budget
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetImpact {
    /// Current spent amount
    pub current_spent: u64,

    /// Projected spent after job
    pub after_job: u64,

    /// Remaining budget after job
    pub remaining_after: u64,

    /// Whether this exceeds the limit
    pub exceeds_limit: bool,
}

impl BudgetImpact {
    /// Calculate budget impact
    pub fn calculate(tracker: &SpendingTracker, estimated_cost: u64) -> Self {
        let after_job = tracker.spent_sats + estimated_cost;
        let remaining_after = tracker.limit_sats.saturating_sub(after_job);
        let exceeds_limit = after_job > tracker.limit_sats;

        Self {
            current_spent: tracker.spent_sats,
            after_job,
            remaining_after,
            exceeds_limit,
        }
    }
}

/// Cost estimate for a job
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostEstimate {
    /// Job identifier
    pub job_id: String,

    /// Estimated input tokens
    pub estimated_input_tokens: u64,

    /// Estimated output tokens
    pub estimated_output_tokens: u64,

    /// Estimated cost in satoshis
    pub estimated_cost_sats: u64,

    /// Confidence in estimate (0.0-1.0)
    pub confidence: u8,

    /// Budget impact
    pub budget_impact: BudgetImpact,
}

impl CostEstimate {
    /// Create a new cost estimate
    pub fn new(
        job_id: impl Into<String>,
        estimated_input_tokens: u64,
        estimated_output_tokens: u64,
        estimated_cost_sats: u64,
        confidence: u8,
        budget_impact: BudgetImpact,
    ) -> Self {
        Self {
            job_id: job_id.into(),
            estimated_input_tokens,
            estimated_output_tokens,
            estimated_cost_sats,
            confidence: confidence.min(100),
            budget_impact,
        }
    }

    /// Check if estimate is high confidence
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 80
    }

    /// Get total estimated tokens
    pub fn total_tokens(&self) -> u64 {
        self.estimated_input_tokens + self.estimated_output_tokens
    }
}

/// Action to take when actual cost exceeds estimate
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverageAction {
    /// Proceed if within tolerance
    Proceed,

    /// Pause and ask user
    PauseAndPrompt,

    /// Never exceed estimate
    Abort,
}

/// Policy for handling cost overages
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OveragePolicy {
    /// Tolerance percentage (e.g., 10 for 10%)
    pub tolerance_pct: u8,

    /// Action to take when exceeded
    pub action_on_exceed: OverageAction,
}

impl OveragePolicy {
    /// Create a new overage policy
    pub fn new(tolerance_pct: u8, action_on_exceed: OverageAction) -> Self {
        Self {
            tolerance_pct,
            action_on_exceed,
        }
    }

    /// Create a strict policy (no overage allowed)
    pub fn strict() -> Self {
        Self {
            tolerance_pct: 0,
            action_on_exceed: OverageAction::Abort,
        }
    }

    /// Create a lenient policy (10% tolerance, proceed)
    pub fn lenient() -> Self {
        Self {
            tolerance_pct: 10,
            action_on_exceed: OverageAction::Proceed,
        }
    }

    /// Check if actual cost is within policy
    pub fn is_within_policy(&self, estimated: u64, actual: u64) -> bool {
        if actual <= estimated {
            return true;
        }

        let overage = actual - estimated;
        let overage_pct = if estimated == 0 {
            100
        } else {
            ((overage as f64 / estimated as f64) * 100.0) as u8
        };

        overage_pct <= self.tolerance_pct
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_alert_threshold_creation() {
        let threshold = AlertThreshold::new(75, AlertAction::Notify).unwrap();
        assert_eq!(threshold.percentage, 75);

        let invalid = AlertThreshold::new(150, AlertAction::Notify);
        assert!(invalid.is_err());
    }

    #[test]
    fn test_alert_threshold_reached() {
        let threshold = AlertThreshold::new(75, AlertAction::Notify).unwrap();
        assert!(threshold.is_reached(750, 1000));
        assert!(!threshold.is_reached(500, 1000));
    }

    #[test]
    fn test_budget_config_builder() {
        let config = BudgetConfig::new("account1")
            .with_daily_cap(10000)
            .with_per_job_cap(1000)
            .with_auto_stop(true);

        assert_eq!(config.daily_cap_sats, Some(10000));
        assert_eq!(config.per_job_cap_sats, Some(1000));
        assert!(config.auto_stop_on_exhaust);
    }

    #[test]
    fn test_budget_config_validation() {
        let valid = BudgetConfig::new("account1").with_daily_cap(10000);
        assert!(valid.validate().is_ok());

        let invalid = BudgetConfig::new("account1");
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_budget_period_description() {
        assert_eq!(BudgetPeriod::Daily.description(), "Daily");
        assert_eq!(BudgetPeriod::Monthly.description(), "Monthly");
    }

    #[test]
    fn test_spending_tracker_record() {
        let start = Utc::now();
        let end = start + Duration::days(1);
        let mut tracker = SpendingTracker::new("account1", BudgetPeriod::Daily, 10000, start, end);

        assert!(tracker.record_spend(3000).is_ok());
        assert_eq!(tracker.spent_sats, 3000);
        assert_eq!(tracker.remaining_sats, 7000);
        assert_eq!(tracker.jobs_count, 1);
    }

    #[test]
    fn test_spending_tracker_exceed() {
        let start = Utc::now();
        let end = start + Duration::days(1);
        let mut tracker = SpendingTracker::new("account1", BudgetPeriod::Daily, 10000, start, end);

        tracker.record_spend(9000).unwrap();
        let result = tracker.record_spend(2000);
        assert!(result.is_err());
    }

    #[test]
    fn test_spending_tracker_percentage() {
        let start = Utc::now();
        let end = start + Duration::days(1);
        let mut tracker = SpendingTracker::new("account1", BudgetPeriod::Daily, 10000, start, end);

        tracker.record_spend(7500).unwrap();
        assert_eq!(tracker.spent_percentage(), 75.0);
    }

    #[test]
    fn test_spending_tracker_exhausted() {
        let start = Utc::now();
        let end = start + Duration::days(1);
        let mut tracker = SpendingTracker::new("account1", BudgetPeriod::Daily, 10000, start, end);

        assert!(!tracker.is_exhausted());
        tracker.record_spend(10000).unwrap();
        assert!(tracker.is_exhausted());
    }

    #[test]
    fn test_budget_check_allowed() {
        let start = Utc::now();
        let end = start + Duration::days(1);
        let config = BudgetConfig::new("account1").with_daily_cap(10000);
        let tracker = SpendingTracker::new("account1", BudgetPeriod::Daily, 10000, start, end);

        let result = check_budget(&config, &tracker, 5000);
        assert!(result.is_allowed());
    }

    #[test]
    fn test_budget_check_per_job_cap() {
        let start = Utc::now();
        let end = start + Duration::days(1);
        let config = BudgetConfig::new("account1")
            .with_daily_cap(10000)
            .with_per_job_cap(1000);
        let tracker = SpendingTracker::new("account1", BudgetPeriod::Daily, 10000, start, end);

        let result = check_budget(&config, &tracker, 2000);
        assert!(result.is_blocked());
    }

    #[test]
    fn test_budget_check_exceeded_auto_stop() {
        let start = Utc::now();
        let end = start + Duration::days(1);
        let config = BudgetConfig::new("account1")
            .with_daily_cap(10000)
            .with_auto_stop(true);
        let tracker = SpendingTracker::new("account1", BudgetPeriod::Daily, 10000, start, end);

        let result = check_budget(&config, &tracker, 15000);
        assert!(result.is_blocked());
    }

    #[test]
    fn test_budget_check_exceeded_no_auto_stop() {
        let start = Utc::now();
        let end = start + Duration::days(1);
        let config = BudgetConfig::new("account1")
            .with_daily_cap(10000)
            .with_auto_stop(false);
        let tracker = SpendingTracker::new("account1", BudgetPeriod::Daily, 10000, start, end);

        let result = check_budget(&config, &tracker, 15000);
        assert!(result.requires_approval());
    }

    #[test]
    fn test_budget_check_threshold_notify() {
        let start = Utc::now();
        let end = start + Duration::days(1);
        let threshold = AlertThreshold::new(50, AlertAction::Notify).unwrap();
        let config = BudgetConfig::new("account1")
            .with_daily_cap(10000)
            .add_threshold(threshold);
        let tracker = SpendingTracker::new("account1", BudgetPeriod::Daily, 10000, start, end);

        let result = check_budget(&config, &tracker, 6000);
        assert!(result.is_allowed());
    }

    #[test]
    fn test_budget_check_threshold_approval() {
        let start = Utc::now();
        let end = start + Duration::days(1);
        let threshold = AlertThreshold::new(75, AlertAction::RequireApproval).unwrap();
        let config = BudgetConfig::new("account1")
            .with_daily_cap(10000)
            .add_threshold(threshold);
        let tracker = SpendingTracker::new("account1", BudgetPeriod::Daily, 10000, start, end);

        let result = check_budget(&config, &tracker, 8000);
        assert!(result.requires_approval());
    }

    #[test]
    fn test_budget_check_threshold_pause() {
        let start = Utc::now();
        let end = start + Duration::days(1);
        let threshold = AlertThreshold::new(90, AlertAction::Pause).unwrap();
        let config = BudgetConfig::new("account1")
            .with_daily_cap(10000)
            .add_threshold(threshold);
        let tracker = SpendingTracker::new("account1", BudgetPeriod::Daily, 10000, start, end);

        let result = check_budget(&config, &tracker, 9500);
        assert!(result.is_blocked());
    }

    #[test]
    fn test_budget_check_result_checks() {
        assert!(BudgetCheckResult::Allowed.is_allowed());
        assert!(!BudgetCheckResult::Allowed.requires_approval());

        let approval = BudgetCheckResult::RequiresApproval {
            reason: "test".to_string(),
        };
        assert!(approval.requires_approval());

        let blocked = BudgetCheckResult::Blocked {
            reason: "test".to_string(),
        };
        assert!(blocked.is_blocked());
    }

    #[test]
    fn test_budget_impact_calculation() {
        let start = Utc::now();
        let end = start + Duration::days(1);
        let tracker = SpendingTracker::new("account1", BudgetPeriod::Daily, 10000, start, end);

        let impact = BudgetImpact::calculate(&tracker, 3000);
        assert_eq!(impact.current_spent, 0);
        assert_eq!(impact.after_job, 3000);
        assert_eq!(impact.remaining_after, 7000);
        assert!(!impact.exceeds_limit);
    }

    #[test]
    fn test_budget_impact_exceeds() {
        let start = Utc::now();
        let end = start + Duration::days(1);
        let tracker = SpendingTracker::new("account1", BudgetPeriod::Daily, 10000, start, end);

        let impact = BudgetImpact::calculate(&tracker, 15000);
        assert!(impact.exceeds_limit);
    }

    #[test]
    fn test_cost_estimate_creation() {
        let start = Utc::now();
        let end = start + Duration::days(1);
        let tracker = SpendingTracker::new("account1", BudgetPeriod::Daily, 10000, start, end);
        let impact = BudgetImpact::calculate(&tracker, 1000);

        let estimate = CostEstimate::new("job1", 5000, 3000, 1000, 85, impact);
        assert_eq!(estimate.estimated_cost_sats, 1000);
        assert!(estimate.is_high_confidence());
        assert_eq!(estimate.total_tokens(), 8000);
    }

    #[test]
    fn test_overage_policy_strict() {
        let policy = OveragePolicy::strict();
        assert_eq!(policy.tolerance_pct, 0);
        assert!(!policy.is_within_policy(1000, 1100));
        assert!(policy.is_within_policy(1000, 1000));
    }

    #[test]
    fn test_overage_policy_lenient() {
        let policy = OveragePolicy::lenient();
        assert_eq!(policy.tolerance_pct, 10);
        assert!(policy.is_within_policy(1000, 1100));
        assert!(!policy.is_within_policy(1000, 1200));
    }

    #[test]
    fn test_overage_policy_custom() {
        let policy = OveragePolicy::new(5, OverageAction::PauseAndPrompt);
        assert!(policy.is_within_policy(1000, 1050));
        assert!(!policy.is_within_policy(1000, 1100));
    }

    #[test]
    fn test_budget_config_serde() {
        let config = BudgetConfig::new("account1")
            .with_daily_cap(10000)
            .with_per_job_cap(1000);

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: BudgetConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_spending_tracker_serde() {
        let start = Utc::now();
        let end = start + Duration::days(1);
        let tracker = SpendingTracker::new("account1", BudgetPeriod::Daily, 10000, start, end);

        let json = serde_json::to_string(&tracker).unwrap();
        let deserialized: SpendingTracker = serde_json::from_str(&json).unwrap();
        assert_eq!(tracker, deserialized);
    }

    #[test]
    fn test_cost_estimate_serde() {
        let start = Utc::now();
        let end = start + Duration::days(1);
        let tracker = SpendingTracker::new("account1", BudgetPeriod::Daily, 10000, start, end);
        let impact = BudgetImpact::calculate(&tracker, 1000);
        let estimate = CostEstimate::new("job1", 5000, 3000, 1000, 85, impact);

        let json = serde_json::to_string(&estimate).unwrap();
        let deserialized: CostEstimate = serde_json::from_str(&json).unwrap();
        assert_eq!(estimate, deserialized);
    }
}
