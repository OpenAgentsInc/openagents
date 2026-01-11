//! Payment and reward distribution for trajectory contributions
//!
//! This module handles the complete payment flow for trajectory contributions:
//! - Quality-based reward calculation
//! - Flow of Funds revenue splitting
//! - Revenue bucket tracking at minute granularity
//! - Lightning payment integration (via Spark SDK)
//! - Automatic withdrawal support

use super::{TrajectorySession, validate::QualityScore, rewards::RewardInfo};
use crate::{
    types::RevenueSplit,
    ledger::{
        Balance, LedgerEntry, LedgerEntryType, Direction, LedgerOperation,
        LedgerAmounts, LedgerParties, LedgerReferences, LedgerError,
    },
    creator_dashboard::{EarningsSummary, PayoutRecord, PayoutStatus},
};
use chrono::{DateTime, Utc, Timelike};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during payment processing
#[derive(Debug, Error)]
pub enum PaymentError {
    #[error("Invalid reward amount: {0}")]
    InvalidReward(String),

    #[error("Invalid split configuration: {0}")]
    InvalidSplit(String),

    #[error("Ledger error: {0}")]
    Ledger(#[from] LedgerError),

    #[error("Payment failed: {0}")]
    PaymentFailed(String),

    #[error("Contributor not found: {0}")]
    ContributorNotFound(String),

    #[error("Insufficient balance: {0}")]
    InsufficientBalance(String),
}

/// Revenue split for trajectory contributions
///
/// Unlike skills (creator/compute/platform/referrer), trajectory contributions
/// have a simpler split: creator gets the full reward minus platform fee
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TrajectoryRevenueSplit {
    /// Percentage for contributor (0-100)
    pub contributor_pct: u8,

    /// Percentage for platform (0-100)
    pub platform_pct: u8,
}

impl Default for TrajectoryRevenueSplit {
    fn default() -> Self {
        Self {
            contributor_pct: 90,  // 90% to contributor
            platform_pct: 10,      // 10% to platform
        }
    }
}

impl TrajectoryRevenueSplit {
    /// Validate that percentages sum to 100
    pub fn is_valid(&self) -> bool {
        self.contributor_pct as u16 + self.platform_pct as u16 == 100
    }

    /// Split a total reward amount
    pub fn split(&self, total_sats: u64) -> (u64, u64) {
        let contributor = (total_sats * self.contributor_pct as u64) / 100;
        let platform = total_sats.saturating_sub(contributor); // Remainder to platform
        (contributor, platform)
    }
}

/// Revenue bucket for tracking earnings at minute granularity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueBucket {
    /// Bucket timestamp (truncated to minute)
    pub bucket_minute: DateTime<Utc>,

    /// Contribution ID
    pub contribution_id: String,

    /// Total reward in sats
    pub gross_sats: u64,

    /// Contributor's share
    pub contributor_sats: u64,

    /// Platform's share
    pub platform_sats: u64,

    /// Split version (for tracking split rule changes)
    pub split_version: u32,

    /// When the bucket was created
    pub created_at: DateTime<Utc>,
}

impl RevenueBucket {
    /// Create a new revenue bucket
    pub fn new(
        contribution_id: impl Into<String>,
        gross_sats: u64,
        split: &TrajectoryRevenueSplit,
    ) -> Self {
        let now = Utc::now();
        let bucket_minute = truncate_to_minute(now);
        let (contributor_sats, platform_sats) = split.split(gross_sats);

        Self {
            bucket_minute,
            contribution_id: contribution_id.into(),
            gross_sats,
            contributor_sats,
            platform_sats,
            split_version: 1, // Increment when split rules change
            created_at: now,
        }
    }

    /// Get the bucket key (minute timestamp + contribution ID)
    pub fn key(&self) -> String {
        format!(
            "{}-{}",
            self.bucket_minute.timestamp(),
            self.contribution_id
        )
    }
}

/// Truncate a timestamp to the minute (zero out seconds and subseconds)
fn truncate_to_minute(dt: DateTime<Utc>) -> DateTime<Utc> {
    dt.with_second(0)
        .and_then(|d| d.with_nanosecond(0))
        .unwrap_or(dt)
}

/// Contribution payment record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributionPayment {
    /// Unique payment ID
    pub id: String,

    /// Contribution/session ID
    pub contribution_id: String,

    /// Contributor's account ID (Nostr pubkey)
    pub contributor: String,

    /// Reward calculation details
    pub reward: RewardInfo,

    /// Revenue split used
    pub split: TrajectoryRevenueSplit,

    /// Gross amount (before split)
    pub gross_sats: u64,

    /// Contributor's net amount
    pub net_sats: u64,

    /// Platform fee
    pub platform_fee_sats: u64,

    /// Payment status
    pub status: PaymentStatus,

    /// Lightning transaction hash (if paid)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_hash: Option<String>,

    /// When the payment was created
    pub created_at: DateTime<Utc>,

    /// When the payment was completed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,

    /// Error message (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Payment status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    /// Payment pending
    Pending,

    /// Being processed
    Processing,

    /// Completed successfully
    Completed,

    /// Failed
    Failed,
}

impl ContributionPayment {
    /// Create a new contribution payment
    pub fn new(
        id: impl Into<String>,
        contribution_id: impl Into<String>,
        contributor: impl Into<String>,
        reward: RewardInfo,
        split: TrajectoryRevenueSplit,
    ) -> Self {
        let gross_sats = reward.total_sats;
        let (net_sats, platform_fee_sats) = split.split(gross_sats);

        Self {
            id: id.into(),
            contribution_id: contribution_id.into(),
            contributor: contributor.into(),
            reward,
            split,
            gross_sats,
            net_sats,
            platform_fee_sats,
            status: PaymentStatus::Pending,
            tx_hash: None,
            created_at: Utc::now(),
            completed_at: None,
            error: None,
        }
    }

    /// Mark as processing
    pub fn mark_processing(&mut self) {
        self.status = PaymentStatus::Processing;
    }

    /// Mark as completed
    pub fn mark_completed(&mut self, tx_hash: impl Into<String>) {
        self.status = PaymentStatus::Completed;
        self.tx_hash = Some(tx_hash.into());
        self.completed_at = Some(Utc::now());
    }

    /// Mark as failed
    pub fn mark_failed(&mut self, error: impl Into<String>) {
        self.status = PaymentStatus::Failed;
        self.error = Some(error.into());
        self.completed_at = Some(Utc::now());
    }

    /// Check if payment is complete
    pub fn is_complete(&self) -> bool {
        self.status == PaymentStatus::Completed
    }

    /// Check if payment can be retried
    pub fn can_retry(&self) -> bool {
        matches!(self.status, PaymentStatus::Failed)
    }
}

/// Payment processor for trajectory contributions
pub struct PaymentProcessor {
    /// Revenue split configuration
    split: TrajectoryRevenueSplit,

    /// Minimum payout threshold (sats)
    min_payout_sats: u64,

    /// Auto-withdraw threshold (sats)
    auto_withdraw_threshold: Option<u64>,
}

impl Default for PaymentProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl PaymentProcessor {
    /// Create a new payment processor with default configuration
    pub fn new() -> Self {
        Self {
            split: TrajectoryRevenueSplit::default(),
            min_payout_sats: 1000, // 1k sats minimum
            auto_withdraw_threshold: None, // Disabled by default
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        split: TrajectoryRevenueSplit,
        min_payout_sats: u64,
        auto_withdraw_threshold: Option<u64>,
    ) -> Self {
        Self {
            split,
            min_payout_sats,
            auto_withdraw_threshold,
        }
    }

    /// Create a payment for a trajectory contribution
    pub fn create_payment(
        &self,
        payment_id: impl Into<String>,
        session: &TrajectorySession,
        contributor: impl Into<String>,
        reward: RewardInfo,
    ) -> Result<ContributionPayment, PaymentError> {
        // Validate split
        if !self.split.is_valid() {
            return Err(PaymentError::InvalidSplit(
                "Split percentages must sum to 100".to_string()
            ));
        }

        // Validate reward
        if reward.total_sats == 0 {
            return Err(PaymentError::InvalidReward(
                "Total reward must be greater than 0".to_string()
            ));
        }

        Ok(ContributionPayment::new(
            payment_id,
            &session.session_id,
            contributor,
            reward,
            self.split,
        ))
    }

    /// Create a revenue bucket for tracking
    pub fn create_revenue_bucket(
        &self,
        payment: &ContributionPayment,
    ) -> RevenueBucket {
        RevenueBucket::new(
            &payment.contribution_id,
            payment.gross_sats,
            &self.split,
        )
    }

    /// Create a ledger entry for a payment
    pub fn create_ledger_entry(
        &self,
        payment: &ContributionPayment,
        previous_hash: impl Into<String>,
    ) -> Result<LedgerEntry, PaymentError> {
        let amounts = LedgerAmounts::new(
            payment.gross_sats,
            payment.platform_fee_sats,
        );

        let parties = LedgerParties::new(
            "platform", // Platform pays contributors
            &payment.contributor,
        );

        let references = LedgerReferences::new()
            .with_job_id(&payment.contribution_id);

        Ok(LedgerEntry::new(
            &payment.id,
            LedgerEntryType::DataPayment, // Trajectory contributions are a type of data
            Direction::Outbound, // From platform to contributor
            LedgerOperation::Credit, // Credit to contributor
            amounts,
            parties,
            references,
            previous_hash,
        )?
        .finalize())
    }

    /// Check if earnings should trigger auto-withdrawal
    pub fn should_auto_withdraw(&self, earnings: &EarningsSummary) -> bool {
        if let Some(threshold) = self.auto_withdraw_threshold {
            earnings.available_balance() >= threshold
        } else {
            false
        }
    }

    /// Process a withdrawal request
    pub fn process_withdrawal(
        &self,
        earnings: &mut EarningsSummary,
        amount_sats: u64,
        destination: impl Into<String>,
    ) -> Result<PayoutRecord, PaymentError> {
        // Validate amount
        if amount_sats < self.min_payout_sats {
            return Err(PaymentError::InvalidReward(
                format!(
                    "Withdrawal amount {} is below minimum {}",
                    amount_sats, self.min_payout_sats
                )
            ));
        }

        // Request payout
        earnings.request_payout(amount_sats)
            .map_err(|e| PaymentError::InsufficientBalance(e.to_string()))?;

        // Create payout record
        let payout_id = format!("payout-{}", Utc::now().timestamp());
        Ok(PayoutRecord::new(payout_id, amount_sats, destination))
    }
}

/// Earnings tracker for trajectory contributions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryEarnings {
    /// Contributor ID (Nostr pubkey)
    pub contributor: String,

    /// Total earned from trajectory contributions
    pub total_earned_sats: u64,

    /// Number of contributions accepted
    pub contributions_accepted: u64,

    /// Revenue buckets by minute
    pub buckets: Vec<RevenueBucket>,

    /// Payments made
    pub payments: Vec<ContributionPayment>,

    /// Last updated
    pub last_updated: DateTime<Utc>,
}

impl TrajectoryEarnings {
    /// Create new earnings tracker
    pub fn new(contributor: impl Into<String>) -> Self {
        Self {
            contributor: contributor.into(),
            total_earned_sats: 0,
            contributions_accepted: 0,
            buckets: Vec::new(),
            payments: Vec::new(),
            last_updated: Utc::now(),
        }
    }

    /// Record a new payment
    pub fn record_payment(&mut self, payment: ContributionPayment, bucket: RevenueBucket) {
        self.total_earned_sats += payment.net_sats;
        self.contributions_accepted += 1;
        self.payments.push(payment);
        self.buckets.push(bucket);
        self.last_updated = Utc::now();
    }

    /// Get earnings for a specific time period
    pub fn earnings_in_period(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> u64 {
        self.buckets
            .iter()
            .filter(|b| b.bucket_minute >= start && b.bucket_minute <= end)
            .map(|b| b.contributor_sats)
            .sum()
    }

    /// Get average reward per contribution
    pub fn avg_reward(&self) -> u64 {
        if self.contributions_accepted == 0 {
            0
        } else {
            self.total_earned_sats / self.contributions_accepted
        }
    }

    /// Get total pending payments
    pub fn pending_sats(&self) -> u64 {
        self.payments
            .iter()
            .filter(|p| p.status == PaymentStatus::Pending)
            .map(|p| p.net_sats)
            .sum()
    }

    /// Get total completed payments
    pub fn completed_sats(&self) -> u64 {
        self.payments
            .iter()
            .filter(|p| p.status == PaymentStatus::Completed)
            .map(|p| p.net_sats)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_session() -> TrajectorySession {
        TrajectorySession {
            session_id: "test-session".to_string(),
            source: "codex".to_string(),
            path: "/tmp/test.rlog".into(),
            initial_commit: Some("abc".to_string()),
            final_commit: Some("def".to_string()),
            ci_passed: Some(true),
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
            token_count: 2000,
            tool_calls: 15,
            quality_score: 0.8,
        }
    }

    fn create_test_reward() -> RewardInfo {
        RewardInfo {
            base_sats: 100,
            quality_bonus_sats: 40,
            ci_bonus_sats: 200,
            complexity_bonus_sats: 95,
            total_sats: 435,
        }
    }

    #[test]
    fn test_trajectory_revenue_split() {
        let split = TrajectoryRevenueSplit::default();
        assert!(split.is_valid());

        let (contributor, platform) = split.split(1000);
        assert_eq!(contributor, 900);
        assert_eq!(platform, 100);
    }

    #[test]
    fn test_revenue_bucket_creation() {
        let split = TrajectoryRevenueSplit::default();
        let bucket = RevenueBucket::new("contrib-1", 1000, &split);

        assert_eq!(bucket.contribution_id, "contrib-1");
        assert_eq!(bucket.gross_sats, 1000);
        assert_eq!(bucket.contributor_sats, 900);
        assert_eq!(bucket.platform_sats, 100);
        assert_eq!(bucket.split_version, 1);
    }

    #[test]
    fn test_contribution_payment_creation() {
        let split = TrajectoryRevenueSplit::default();
        let reward = create_test_reward();

        let payment = ContributionPayment::new(
            "payment-1",
            "contrib-1",
            "contributor-pubkey",
            reward,
            split,
        );

        assert_eq!(payment.id, "payment-1");
        assert_eq!(payment.gross_sats, 435);
        assert_eq!(payment.net_sats, 391); // 90% of 435
        assert_eq!(payment.platform_fee_sats, 44); // Remainder
        assert_eq!(payment.status, PaymentStatus::Pending);
    }

    #[test]
    fn test_payment_lifecycle() {
        let split = TrajectoryRevenueSplit::default();
        let reward = create_test_reward();

        let mut payment = ContributionPayment::new(
            "payment-1",
            "contrib-1",
            "contributor",
            reward,
            split,
        );

        assert_eq!(payment.status, PaymentStatus::Pending);
        assert!(!payment.is_complete());

        payment.mark_processing();
        assert_eq!(payment.status, PaymentStatus::Processing);

        payment.mark_completed("tx123");
        assert_eq!(payment.status, PaymentStatus::Completed);
        assert!(payment.is_complete());
        assert_eq!(payment.tx_hash.as_deref(), Some("tx123"));
    }

    #[test]
    fn test_payment_failure() {
        let split = TrajectoryRevenueSplit::default();
        let reward = create_test_reward();

        let mut payment = ContributionPayment::new(
            "payment-1",
            "contrib-1",
            "contributor",
            reward,
            split,
        );

        payment.mark_failed("Insufficient balance");
        assert_eq!(payment.status, PaymentStatus::Failed);
        assert!(payment.can_retry());
        assert_eq!(payment.error.as_deref(), Some("Insufficient balance"));
    }

    #[test]
    fn test_payment_processor_create_payment() {
        let processor = PaymentProcessor::new();
        let session = create_test_session();
        let reward = create_test_reward();

        let payment = processor
            .create_payment("payment-1", &session, "contributor", reward)
            .unwrap();

        assert_eq!(payment.contribution_id, "test-session");
        assert_eq!(payment.gross_sats, 435);
    }

    #[test]
    fn test_payment_processor_revenue_bucket() {
        let processor = PaymentProcessor::new();
        let session = create_test_session();
        let reward = create_test_reward();

        let payment = processor
            .create_payment("payment-1", &session, "contributor", reward)
            .unwrap();

        let bucket = processor.create_revenue_bucket(&payment);
        assert_eq!(bucket.contribution_id, "test-session");
        assert_eq!(bucket.gross_sats, 435);
    }

    #[test]
    fn test_payment_processor_auto_withdraw() {
        let processor = PaymentProcessor::with_config(
            TrajectoryRevenueSplit::default(),
            1000,
            Some(10000), // Auto-withdraw at 10k sats
        );

        let mut earnings = EarningsSummary::new();
        earnings.total_earned_sats = 5000;
        assert!(!processor.should_auto_withdraw(&earnings));

        earnings.total_earned_sats = 15000;
        assert!(processor.should_auto_withdraw(&earnings));
    }

    #[test]
    fn test_payment_processor_withdrawal() {
        let processor = PaymentProcessor::new();
        let mut earnings = EarningsSummary::new();
        earnings.total_earned_sats = 50000;

        let payout = processor
            .process_withdrawal(&mut earnings, 10000, "user@domain.com")
            .unwrap();

        assert_eq!(payout.amount_sats, 10000);
        assert_eq!(payout.status, PayoutStatus::Pending);
        assert_eq!(earnings.pending_payout_sats, 10000);
    }

    #[test]
    fn test_payment_processor_withdrawal_below_minimum() {
        let processor = PaymentProcessor::new();
        let mut earnings = EarningsSummary::new();
        earnings.total_earned_sats = 50000;

        let result = processor.process_withdrawal(&mut earnings, 500, "user@domain.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_trajectory_earnings() {
        let mut earnings = TrajectoryEarnings::new("contributor-1");

        assert_eq!(earnings.total_earned_sats, 0);
        assert_eq!(earnings.contributions_accepted, 0);

        let split = TrajectoryRevenueSplit::default();
        let reward = create_test_reward();
        let payment = ContributionPayment::new(
            "payment-1",
            "contrib-1",
            "contributor-1",
            reward.clone(),
            split,
        );
        let bucket = RevenueBucket::new("contrib-1", reward.total_sats, &split);

        earnings.record_payment(payment, bucket);

        assert_eq!(earnings.total_earned_sats, 391); // 90% of 435
        assert_eq!(earnings.contributions_accepted, 1);
        assert_eq!(earnings.avg_reward(), 391);
    }

    #[test]
    fn test_trajectory_earnings_pending_vs_completed() {
        let mut earnings = TrajectoryEarnings::new("contributor-1");

        let split = TrajectoryRevenueSplit::default();
        let reward = create_test_reward();

        // Add pending payment
        let payment1 = ContributionPayment::new(
            "payment-1",
            "contrib-1",
            "contributor-1",
            reward.clone(),
            split,
        );
        let bucket1 = RevenueBucket::new("contrib-1", reward.total_sats, &split);
        earnings.record_payment(payment1, bucket1);

        // Add completed payment
        let mut payment2 = ContributionPayment::new(
            "payment-2",
            "contrib-2",
            "contributor-1",
            reward.clone(),
            split,
        );
        payment2.mark_completed("tx123");
        let bucket2 = RevenueBucket::new("contrib-2", reward.total_sats, &split);
        earnings.record_payment(payment2, bucket2);

        assert_eq!(earnings.pending_sats(), 391);
        assert_eq!(earnings.completed_sats(), 391);
    }

    #[test]
    fn test_truncate_to_minute() {
        let dt = Utc::now();
        let truncated = truncate_to_minute(dt);

        assert_eq!(truncated.second(), 0);
        assert_eq!(truncated.nanosecond(), 0);
        assert_eq!(truncated.minute(), dt.minute());
        assert_eq!(truncated.hour(), dt.hour());
    }
}
