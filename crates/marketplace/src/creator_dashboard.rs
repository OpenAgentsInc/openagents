//! Creator dashboard types for skill analytics and earnings
//!
//! Provides types for skill creators to track their published skills,
//! earnings, analytics, and payout history.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

use crate::types::SkillSubmissionStatus;

/// Errors that can occur when working with creator dashboards
#[derive(Debug, Error)]
pub enum DashboardError {
    #[error("Creator not found: {0}")]
    CreatorNotFound(String),

    #[error("Skill not found: {0}")]
    SkillNotFound(String),

    #[error("Invalid payout amount: {0}")]
    InvalidPayoutAmount(String),

    #[error("Insufficient balance for payout: available {available}, requested {requested}")]
    InsufficientBalance { available: u64, requested: u64 },

    #[error("Invalid period format: {0}")]
    InvalidPeriod(String),
}

/// Payout status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayoutStatus {
    /// Payout requested but not yet processed
    Pending,
    /// Payout is being processed
    Processing,
    /// Payout completed successfully
    Completed,
    /// Payout failed
    Failed,
    /// Payout was cancelled
    Cancelled,
}

impl PayoutStatus {
    /// Check if this is a terminal status
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            PayoutStatus::Completed | PayoutStatus::Failed | PayoutStatus::Cancelled
        )
    }

    /// Check if this payout is still pending
    pub fn is_pending(&self) -> bool {
        matches!(self, PayoutStatus::Pending | PayoutStatus::Processing)
    }
}

/// Record of a payout to a creator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutRecord {
    /// Unique payout ID
    pub id: String,

    /// Amount in satoshis
    pub amount_sats: u64,

    /// Lightning address or payment destination
    pub destination: String,

    /// Current status
    pub status: PayoutStatus,

    /// When the payout was requested
    pub requested_at: DateTime<Utc>,

    /// When the payout was completed (if completed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,

    /// Transaction ID or payment proof (if completed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,

    /// Error message (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl PayoutRecord {
    /// Create a new pending payout
    pub fn new(id: impl Into<String>, amount_sats: u64, destination: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            amount_sats,
            destination: destination.into(),
            status: PayoutStatus::Pending,
            requested_at: Utc::now(),
            completed_at: None,
            transaction_id: None,
            error: None,
        }
    }

    /// Mark the payout as processing
    pub fn mark_processing(&mut self) {
        self.status = PayoutStatus::Processing;
    }

    /// Mark the payout as completed
    pub fn mark_completed(&mut self, transaction_id: impl Into<String>) {
        self.status = PayoutStatus::Completed;
        self.completed_at = Some(Utc::now());
        self.transaction_id = Some(transaction_id.into());
    }

    /// Mark the payout as failed
    pub fn mark_failed(&mut self, error: impl Into<String>) {
        self.status = PayoutStatus::Failed;
        self.completed_at = Some(Utc::now());
        self.error = Some(error.into());
    }

    /// Mark the payout as cancelled
    pub fn mark_cancelled(&mut self) {
        self.status = PayoutStatus::Cancelled;
        self.completed_at = Some(Utc::now());
    }
}

/// Earnings for a specific time period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodEarnings {
    /// Period identifier (e.g., "2025-01" for monthly, "2025-W03" for weekly)
    pub period: String,

    /// Total earned in this period (satoshis)
    pub earned_sats: u64,

    /// Number of jobs completed in this period
    pub jobs_completed: u64,
}

impl PeriodEarnings {
    /// Create new period earnings
    pub fn new(period: impl Into<String>, earned_sats: u64, jobs_completed: u64) -> Self {
        Self {
            period: period.into(),
            earned_sats,
            jobs_completed,
        }
    }

    /// Get average earnings per job
    pub fn avg_per_job(&self) -> u64 {
        if self.jobs_completed == 0 {
            0
        } else {
            self.earned_sats / self.jobs_completed
        }
    }
}

/// Summary of creator's earnings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarningsSummary {
    /// Total earned across all skills (satoshis)
    pub total_earned_sats: u64,

    /// Pending payout amount (satoshis)
    pub pending_payout_sats: u64,

    /// Most recent payout record
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_payout: Option<PayoutRecord>,

    /// Earnings broken down by skill ID
    pub earnings_by_skill: HashMap<String, u64>,

    /// Earnings over time periods
    pub earnings_by_period: Vec<PeriodEarnings>,
}

impl EarningsSummary {
    /// Create a new earnings summary
    pub fn new() -> Self {
        Self {
            total_earned_sats: 0,
            pending_payout_sats: 0,
            last_payout: None,
            earnings_by_skill: HashMap::new(),
            earnings_by_period: Vec::new(),
        }
    }

    /// Get available balance (total earned - pending payout)
    pub fn available_balance(&self) -> u64 {
        self.total_earned_sats
            .saturating_sub(self.pending_payout_sats)
    }

    /// Add earnings from a skill
    pub fn record_earning(&mut self, skill_id: impl Into<String>, amount_sats: u64) {
        let skill_id = skill_id.into();
        self.total_earned_sats += amount_sats;
        *self.earnings_by_skill.entry(skill_id).or_insert(0) += amount_sats;
    }

    /// Request a payout
    pub fn request_payout(&mut self, amount_sats: u64) -> Result<(), DashboardError> {
        if amount_sats == 0 {
            return Err(DashboardError::InvalidPayoutAmount(
                "Amount must be greater than 0".to_string(),
            ));
        }

        let available = self.available_balance();
        if amount_sats > available {
            return Err(DashboardError::InsufficientBalance {
                available,
                requested: amount_sats,
            });
        }

        self.pending_payout_sats += amount_sats;
        Ok(())
    }

    /// Complete a payout
    pub fn complete_payout(&mut self, amount_sats: u64) {
        self.pending_payout_sats = self.pending_payout_sats.saturating_sub(amount_sats);
    }
}

impl Default for EarningsSummary {
    fn default() -> Self {
        Self::new()
    }
}

/// Analytics data for a creator's skills
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatorAnalytics {
    /// Total number of installs across all skills
    pub total_installs: u64,

    /// Number of active users in the last 30 days
    pub active_users_30d: u64,

    /// Average rating across all skills (0.0 to 5.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_rating: Option<f32>,

    /// Total number of skill invocations
    pub total_invocations: u64,

    /// Invocations by day (date string, count)
    pub invocations_by_day: Vec<(String, u64)>,
}

impl CreatorAnalytics {
    /// Create new analytics
    pub fn new() -> Self {
        Self {
            total_installs: 0,
            active_users_30d: 0,
            avg_rating: None,
            total_invocations: 0,
            invocations_by_day: Vec::new(),
        }
    }

    /// Get total invocations from the by-day data
    pub fn calculate_total_invocations(&self) -> u64 {
        self.invocations_by_day.iter().map(|(_, count)| count).sum()
    }

    /// Get invocations for the last N days
    pub fn recent_invocations(&self, days: usize) -> Vec<(String, u64)> {
        let len = self.invocations_by_day.len();
        if len <= days {
            self.invocations_by_day.clone()
        } else {
            self.invocations_by_day[len - days..].to_vec()
        }
    }
}

impl Default for CreatorAnalytics {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of a skill from a creator's perspective
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatorSkillSummary {
    /// Skill ID
    pub skill_id: String,

    /// Skill name
    pub name: String,

    /// Current submission/publication status
    pub status: SkillSubmissionStatus,

    /// Number of installs
    pub installs: u64,

    /// Total revenue generated (satoshis)
    pub revenue_sats: u64,

    /// Average rating (0.0 to 5.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<f32>,

    /// Number of ratings received
    #[serde(default)]
    pub rating_count: u64,

    /// When the skill was published
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<DateTime<Utc>>,

    /// When the skill was last updated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<DateTime<Utc>>,
}

impl CreatorSkillSummary {
    /// Create a new skill summary
    pub fn new(skill_id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            skill_id: skill_id.into(),
            name: name.into(),
            status: SkillSubmissionStatus::Draft,
            installs: 0,
            revenue_sats: 0,
            rating: None,
            rating_count: 0,
            published_at: None,
            last_updated: None,
        }
    }

    /// Check if the skill is published
    pub fn is_published(&self) -> bool {
        matches!(
            self.status,
            SkillSubmissionStatus::Published | SkillSubmissionStatus::Deprecated
        )
    }

    /// Get average earnings per install
    pub fn avg_revenue_per_install(&self) -> u64 {
        if self.installs == 0 {
            0
        } else {
            self.revenue_sats / self.installs
        }
    }
}

/// Complete dashboard for a skill creator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatorDashboard {
    /// Creator's Nostr public key (hex format)
    pub creator: String,

    /// All skills by this creator
    pub skills: Vec<CreatorSkillSummary>,

    /// Earnings summary
    pub earnings: EarningsSummary,

    /// Analytics summary
    pub analytics: CreatorAnalytics,

    /// When the dashboard was last updated
    pub last_updated: DateTime<Utc>,
}

impl CreatorDashboard {
    /// Create a new creator dashboard
    pub fn new(creator: impl Into<String>) -> Self {
        Self {
            creator: creator.into(),
            skills: Vec::new(),
            earnings: EarningsSummary::new(),
            analytics: CreatorAnalytics::new(),
            last_updated: Utc::now(),
        }
    }

    /// Add a skill to the dashboard
    pub fn add_skill(&mut self, skill: CreatorSkillSummary) {
        self.skills.push(skill);
        self.last_updated = Utc::now();
    }

    /// Get a skill by ID
    pub fn get_skill(&self, skill_id: &str) -> Option<&CreatorSkillSummary> {
        self.skills.iter().find(|s| s.skill_id == skill_id)
    }

    /// Get total number of published skills
    pub fn published_skills_count(&self) -> usize {
        self.skills.iter().filter(|s| s.is_published()).count()
    }

    /// Get total revenue across all skills
    pub fn total_revenue(&self) -> u64 {
        self.skills.iter().map(|s| s.revenue_sats).sum()
    }

    /// Refresh the dashboard timestamp
    pub fn refresh(&mut self) {
        self.last_updated = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payout_status_checks() {
        assert!(PayoutStatus::Completed.is_terminal());
        assert!(PayoutStatus::Failed.is_terminal());
        assert!(PayoutStatus::Cancelled.is_terminal());
        assert!(!PayoutStatus::Pending.is_terminal());

        assert!(PayoutStatus::Pending.is_pending());
        assert!(PayoutStatus::Processing.is_pending());
        assert!(!PayoutStatus::Completed.is_pending());
    }

    #[test]
    fn test_payout_record_lifecycle() {
        let mut payout = PayoutRecord::new("payout1", 100000, "user@domain.com");

        assert_eq!(payout.status, PayoutStatus::Pending);
        assert!(payout.completed_at.is_none());

        payout.mark_processing();
        assert_eq!(payout.status, PayoutStatus::Processing);

        payout.mark_completed("tx123");
        assert_eq!(payout.status, PayoutStatus::Completed);
        assert!(payout.completed_at.is_some());
        assert_eq!(payout.transaction_id.as_deref(), Some("tx123"));
    }

    #[test]
    fn test_payout_record_failure() {
        let mut payout = PayoutRecord::new("payout2", 50000, "user@domain.com");

        payout.mark_failed("Insufficient balance");
        assert_eq!(payout.status, PayoutStatus::Failed);
        assert!(payout.completed_at.is_some());
        assert_eq!(payout.error.as_deref(), Some("Insufficient balance"));
    }

    #[test]
    fn test_period_earnings() {
        let period = PeriodEarnings::new("2025-01", 1000000, 50);

        assert_eq!(period.period, "2025-01");
        assert_eq!(period.earned_sats, 1000000);
        assert_eq!(period.jobs_completed, 50);
        assert_eq!(period.avg_per_job(), 20000);

        let zero_jobs = PeriodEarnings::new("2025-02", 0, 0);
        assert_eq!(zero_jobs.avg_per_job(), 0);
    }

    #[test]
    fn test_earnings_summary_basic() {
        let mut earnings = EarningsSummary::new();

        assert_eq!(earnings.total_earned_sats, 0);
        assert_eq!(earnings.available_balance(), 0);

        earnings.record_earning("skill1", 50000);
        earnings.record_earning("skill2", 30000);
        earnings.record_earning("skill1", 20000);

        assert_eq!(earnings.total_earned_sats, 100000);
        assert_eq!(earnings.earnings_by_skill.get("skill1"), Some(&70000));
        assert_eq!(earnings.earnings_by_skill.get("skill2"), Some(&30000));
    }

    #[test]
    fn test_earnings_payout_request() {
        let mut earnings = EarningsSummary::new();
        earnings.total_earned_sats = 100000;

        // Valid payout request
        assert!(earnings.request_payout(50000).is_ok());
        assert_eq!(earnings.pending_payout_sats, 50000);
        assert_eq!(earnings.available_balance(), 50000);

        // Another valid request
        assert!(earnings.request_payout(30000).is_ok());
        assert_eq!(earnings.pending_payout_sats, 80000);
        assert_eq!(earnings.available_balance(), 20000);

        // Invalid: exceeds available balance
        assert!(earnings.request_payout(30000).is_err());

        // Invalid: zero amount
        assert!(earnings.request_payout(0).is_err());
    }

    #[test]
    fn test_earnings_payout_completion() {
        let mut earnings = EarningsSummary::new();
        earnings.total_earned_sats = 100000;
        earnings.request_payout(50000).unwrap();

        assert_eq!(earnings.pending_payout_sats, 50000);

        earnings.complete_payout(50000);
        assert_eq!(earnings.pending_payout_sats, 0);
        assert_eq!(earnings.available_balance(), 100000);
    }

    #[test]
    fn test_creator_analytics() {
        let mut analytics = CreatorAnalytics::new();

        analytics.total_installs = 100;
        analytics.active_users_30d = 75;
        analytics.avg_rating = Some(4.5);
        analytics.invocations_by_day = vec![
            ("2025-01-01".to_string(), 10),
            ("2025-01-02".to_string(), 20),
            ("2025-01-03".to_string(), 15),
        ];

        assert_eq!(analytics.calculate_total_invocations(), 45);

        let recent = analytics.recent_invocations(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].0, "2025-01-02");
        assert_eq!(recent[1].0, "2025-01-03");
    }

    #[test]
    fn test_creator_skill_summary() {
        let mut skill = CreatorSkillSummary::new("skill1", "My Skill");

        assert_eq!(skill.skill_id, "skill1");
        assert_eq!(skill.status, SkillSubmissionStatus::Draft);
        assert!(!skill.is_published());

        skill.status = SkillSubmissionStatus::Published;
        assert!(skill.is_published());

        skill.installs = 100;
        skill.revenue_sats = 500000;
        assert_eq!(skill.avg_revenue_per_install(), 5000);

        let no_installs = CreatorSkillSummary::new("skill2", "Other");
        assert_eq!(no_installs.avg_revenue_per_install(), 0);
    }

    #[test]
    fn test_creator_dashboard() {
        let mut dashboard = CreatorDashboard::new("creator123");

        assert_eq!(dashboard.creator, "creator123");
        assert_eq!(dashboard.skills.len(), 0);
        assert_eq!(dashboard.published_skills_count(), 0);
        assert_eq!(dashboard.total_revenue(), 0);

        let mut skill1 = CreatorSkillSummary::new("skill1", "Skill 1");
        skill1.status = SkillSubmissionStatus::Published;
        skill1.revenue_sats = 100000;

        let mut skill2 = CreatorSkillSummary::new("skill2", "Skill 2");
        skill2.status = SkillSubmissionStatus::Draft;
        skill2.revenue_sats = 50000;

        dashboard.add_skill(skill1);
        dashboard.add_skill(skill2);

        assert_eq!(dashboard.skills.len(), 2);
        assert_eq!(dashboard.published_skills_count(), 1);
        assert_eq!(dashboard.total_revenue(), 150000);

        assert!(dashboard.get_skill("skill1").is_some());
        assert!(dashboard.get_skill("skill3").is_none());
    }

    #[test]
    fn test_payout_record_serde() {
        let payout = PayoutRecord::new("payout1", 100000, "user@domain.com");
        let json = serde_json::to_string(&payout).unwrap();
        let deserialized: PayoutRecord = serde_json::from_str(&json).unwrap();

        assert_eq!(payout.id, deserialized.id);
        assert_eq!(payout.amount_sats, deserialized.amount_sats);
        assert_eq!(payout.destination, deserialized.destination);
    }

    #[test]
    fn test_earnings_summary_serde() {
        let mut earnings = EarningsSummary::new();
        earnings.record_earning("skill1", 100000);

        let json = serde_json::to_string(&earnings).unwrap();
        let deserialized: EarningsSummary = serde_json::from_str(&json).unwrap();

        assert_eq!(earnings.total_earned_sats, deserialized.total_earned_sats);
    }

    #[test]
    fn test_creator_dashboard_serde() {
        let dashboard = CreatorDashboard::new("creator123");
        let json = serde_json::to_string(&dashboard).unwrap();
        let deserialized: CreatorDashboard = serde_json::from_str(&json).unwrap();

        assert_eq!(dashboard.creator, deserialized.creator);
    }
}
