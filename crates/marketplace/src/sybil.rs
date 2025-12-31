//! Sybil resistance types for marketplace anti-abuse
//!
//! This module provides stake-based and proof-of-work sybil resistance
//! mechanisms to prevent identity spoofing and abuse in the marketplace.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::trust::EntityType;

/// Errors that can occur during sybil resistance operations
#[derive(Debug, Clone, Error, PartialEq, Serialize, Deserialize)]
pub enum SybilError {
    #[error("Insufficient stake: required {required} sats, have {available} sats")]
    InsufficientStake { required: u64, available: u64 },

    #[error("Stake is locked until {0}")]
    StakeLocked(DateTime<Utc>),

    #[error("Invalid proof of work: {0}")]
    InvalidProofOfWork(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Stake already slashed")]
    AlreadySlashed,

    #[error("Cannot release stake: {0}")]
    CannotRelease(String),
}

/// Conditions that must be met to release a stake
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReleaseCondition {
    /// Must have minimum success rate
    MinSuccessRate(u8), // Percentage, e.g., 95 for 95%

    /// Must have no active disputes
    NoActiveDisputes,

    /// Must be at least N days old
    MinAgeDays(u32),
}

impl ReleaseCondition {
    /// Check if condition is met
    pub fn is_met(&self, success_rate: f32, has_disputes: bool, age_days: u32) -> bool {
        match self {
            ReleaseCondition::MinSuccessRate(min) => (success_rate * 100.0) as u8 >= *min,
            ReleaseCondition::NoActiveDisputes => !has_disputes,
            ReleaseCondition::MinAgeDays(min) => age_days >= *min,
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> String {
        match self {
            ReleaseCondition::MinSuccessRate(rate) => format!("Minimum {}% success rate", rate),
            ReleaseCondition::NoActiveDisputes => "No active disputes".to_string(),
            ReleaseCondition::MinAgeDays(days) => format!("Account at least {} days old", days),
        }
    }
}

/// Conditions that trigger stake slashing
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlashCondition {
    /// Fraud was detected
    FraudDetected,

    /// Too many repeated failures
    RepeatedFailures(u32),

    /// Lost a dispute
    DisputeLoss,

    /// Violated marketplace policy
    PolicyViolation,
}

impl SlashCondition {
    /// Get human-readable description
    pub fn description(&self) -> String {
        match self {
            SlashCondition::FraudDetected => "Fraud detected".to_string(),
            SlashCondition::RepeatedFailures(n) => format!("{} repeated failures", n),
            SlashCondition::DisputeLoss => "Lost dispute resolution".to_string(),
            SlashCondition::PolicyViolation => "Policy violation".to_string(),
        }
    }

    /// Get default slash percentage for this condition
    pub fn default_slash_percent(&self) -> u8 {
        match self {
            SlashCondition::FraudDetected => 100,
            SlashCondition::RepeatedFailures(_) => 25,
            SlashCondition::DisputeLoss => 50,
            SlashCondition::PolicyViolation => 75,
        }
    }
}

/// Stake requirement for an entity type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StakeRequirement {
    /// Type of entity this requirement applies to
    pub entity_type: EntityType,

    /// Minimum stake amount in satoshis
    pub min_stake_sats: u64,

    /// Number of days stake must be held
    pub hold_period_days: u32,

    /// Conditions that must be met to release stake
    pub release_conditions: Vec<ReleaseCondition>,

    /// Conditions that trigger slashing
    pub slash_conditions: Vec<SlashCondition>,
}

impl StakeRequirement {
    /// Create a new stake requirement
    pub fn new(entity_type: EntityType, min_stake_sats: u64, hold_period_days: u32) -> Self {
        Self {
            entity_type,
            min_stake_sats,
            hold_period_days,
            release_conditions: vec![
                ReleaseCondition::MinSuccessRate(95),
                ReleaseCondition::NoActiveDisputes,
            ],
            slash_conditions: vec![SlashCondition::FraudDetected, SlashCondition::DisputeLoss],
        }
    }

    /// Create default requirements for providers
    pub fn provider_default() -> Self {
        Self::new(EntityType::Provider, 100_000, 30) // 100k sats, 30 days
    }

    /// Create default requirements for creators
    pub fn creator_default() -> Self {
        Self::new(EntityType::Creator, 50_000, 14) // 50k sats, 14 days
    }

    /// Create default requirements for agents
    pub fn agent_default() -> Self {
        Self::new(EntityType::Agent, 25_000, 7) // 25k sats, 7 days
    }

    /// Add a release condition
    pub fn with_release_condition(mut self, condition: ReleaseCondition) -> Self {
        self.release_conditions.push(condition);
        self
    }

    /// Add a slash condition
    pub fn with_slash_condition(mut self, condition: SlashCondition) -> Self {
        self.slash_conditions.push(condition);
        self
    }
}

/// Current status of a stake
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StakeStatus {
    /// Stake is active and locked
    Active,

    /// Stake release has been requested, pending conditions
    PendingRelease,

    /// Stake has been released back to entity
    Released,

    /// Stake was slashed
    Slashed(SlashRecord),
}

impl StakeStatus {
    /// Check if stake is currently locked
    pub fn is_locked(&self) -> bool {
        matches!(self, StakeStatus::Active | StakeStatus::PendingRelease)
    }

    /// Check if stake can be used as collateral
    pub fn is_active(&self) -> bool {
        matches!(self, StakeStatus::Active)
    }
}

/// Record of a stake being slashed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlashRecord {
    /// Amount that was slashed in satoshis
    pub amount_sats: u64,

    /// Reason for slashing
    pub reason: SlashCondition,

    /// When the slash occurred
    pub slashed_at: DateTime<Utc>,

    /// Reference to evidence or dispute
    pub reference_id: Option<String>,
}

impl SlashRecord {
    /// Create a new slash record
    pub fn new(amount_sats: u64, reason: SlashCondition) -> Self {
        Self {
            amount_sats,
            reason,
            slashed_at: Utc::now(),
            reference_id: None,
        }
    }

    /// Add a reference ID (dispute, evidence, etc.)
    pub fn with_reference(mut self, reference_id: impl Into<String>) -> Self {
        self.reference_id = Some(reference_id.into());
        self
    }
}

/// A stake held by an entity
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stake {
    /// Unique stake identifier
    pub id: String,

    /// Entity that owns this stake
    pub entity_id: String,

    /// Amount staked in satoshis
    pub amount_sats: u64,

    /// When the stake was created
    pub staked_at: DateTime<Utc>,

    /// When the stake can be released (earliest)
    pub locked_until: DateTime<Utc>,

    /// Current status
    pub status: StakeStatus,
}

impl Stake {
    /// Create a new stake
    pub fn new(entity_id: impl Into<String>, amount_sats: u64, hold_days: u32) -> Self {
        let now = Utc::now();
        let locked_until = now + chrono::Duration::days(hold_days as i64);

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            entity_id: entity_id.into(),
            amount_sats,
            staked_at: now,
            locked_until,
            status: StakeStatus::Active,
        }
    }

    /// Check if stake meets minimum requirement
    pub fn meets_requirement(&self, requirement: &StakeRequirement) -> bool {
        self.status.is_active() && self.amount_sats >= requirement.min_stake_sats
    }

    /// Check if stake can be released
    pub fn can_release(&self, success_rate: f32, has_disputes: bool, age_days: u32) -> bool {
        if !matches!(
            self.status,
            StakeStatus::Active | StakeStatus::PendingRelease
        ) {
            return false;
        }

        if Utc::now() < self.locked_until {
            return false;
        }

        // All conditions must be met (we don't have access to the original requirements here,
        // so we use default conditions)
        let default_conditions = vec![
            ReleaseCondition::MinSuccessRate(95),
            ReleaseCondition::NoActiveDisputes,
        ];

        default_conditions
            .iter()
            .all(|c| c.is_met(success_rate, has_disputes, age_days))
    }

    /// Request release of stake
    pub fn request_release(&mut self) -> Result<(), SybilError> {
        match &self.status {
            StakeStatus::Active => {
                if Utc::now() < self.locked_until {
                    return Err(SybilError::StakeLocked(self.locked_until));
                }
                self.status = StakeStatus::PendingRelease;
                Ok(())
            }
            StakeStatus::PendingRelease => Ok(()),
            StakeStatus::Released => Err(SybilError::CannotRelease("Already released".into())),
            StakeStatus::Slashed(_) => Err(SybilError::AlreadySlashed),
        }
    }

    /// Complete the release
    pub fn complete_release(&mut self) -> Result<u64, SybilError> {
        match &self.status {
            StakeStatus::PendingRelease => {
                let amount = self.amount_sats;
                self.status = StakeStatus::Released;
                Ok(amount)
            }
            StakeStatus::Active => Err(SybilError::CannotRelease(
                "Must request release first".into(),
            )),
            StakeStatus::Released => Err(SybilError::CannotRelease("Already released".into())),
            StakeStatus::Slashed(_) => Err(SybilError::AlreadySlashed),
        }
    }

    /// Slash the stake
    pub fn slash(&mut self, reason: SlashCondition, slash_percent: u8) -> Result<u64, SybilError> {
        if matches!(self.status, StakeStatus::Slashed(_)) {
            return Err(SybilError::AlreadySlashed);
        }

        let slash_amount = (self.amount_sats as f64 * (slash_percent as f64 / 100.0)) as u64;
        let record = SlashRecord::new(slash_amount, reason);
        self.status = StakeStatus::Slashed(record);

        Ok(slash_amount)
    }
}

/// Proof of work challenge and solution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofOfWork {
    /// Entity that completed the proof
    pub entity_id: String,

    /// Challenge string (random data)
    pub challenge: String,

    /// Solution (nonce that produces valid hash)
    pub solution: String,

    /// Required difficulty (number of leading zero bits)
    pub difficulty: u32,

    /// Whether the solution has been verified
    pub verified: bool,

    /// When the proof was completed
    pub completed_at: DateTime<Utc>,
}

impl ProofOfWork {
    /// Create a new proof of work with a generated challenge
    pub fn new(entity_id: impl Into<String>, difficulty: u32) -> Self {
        Self {
            entity_id: entity_id.into(),
            challenge: generate_challenge(),
            solution: String::new(),
            difficulty,
            verified: false,
            completed_at: Utc::now(),
        }
    }

    /// Submit a solution
    pub fn submit_solution(&mut self, solution: impl Into<String>) -> bool {
        self.solution = solution.into();
        self.verified = verify_solution(&self.challenge, &self.solution, self.difficulty);
        if self.verified {
            self.completed_at = Utc::now();
        }
        self.verified
    }

    /// Check if verified
    pub fn is_verified(&self) -> bool {
        self.verified
    }
}

/// Generate a random challenge string
pub fn generate_challenge() -> String {
    // Use UUID v4 (which is random) combined with timestamp for entropy
    let uuid1 = uuid::Uuid::new_v4();
    let uuid2 = uuid::Uuid::new_v4();
    let mut hasher = Sha256::new();
    hasher.update(uuid1.as_bytes());
    hasher.update(uuid2.as_bytes());
    hasher.update(Utc::now().timestamp_nanos_opt().unwrap_or(0).to_le_bytes());
    hex::encode(hasher.finalize())
}

/// Verify a proof of work solution
pub fn verify_solution(challenge: &str, solution: &str, difficulty: u32) -> bool {
    let mut hasher = Sha256::new();
    hasher.update(challenge.as_bytes());
    hasher.update(solution.as_bytes());
    let result = hasher.finalize();

    // Check for leading zero bits
    let required_zero_bytes = (difficulty / 8) as usize;
    let remaining_bits = difficulty % 8;

    // Check full zero bytes
    if result.iter().take(required_zero_bytes).any(|&b| b != 0) {
        return false;
    }

    // Check remaining bits
    if remaining_bits > 0 && required_zero_bytes < result.len() {
        let mask = 0xFF << (8 - remaining_bits);
        if result[required_zero_bytes] & mask != 0 {
            return false;
        }
    }

    true
}

/// Find a valid solution for a challenge (for testing/demonstration)
pub fn solve_challenge(challenge: &str, difficulty: u32) -> String {
    let mut nonce: u64 = 0;
    loop {
        let solution = format!("{:016x}", nonce);
        if verify_solution(challenge, &solution, difficulty) {
            return solution;
        }
        nonce += 1;
    }
}

/// Rate-limited actions in the marketplace
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RateLimitedAction {
    /// Creating a new identity
    CreateIdentity,

    /// Creating a new listing
    CreateListing,

    /// Submitting a job
    SubmitJob,

    /// Joining a coalition
    JoinCoalition,

    /// Sending a payment
    SendPayment,

    /// Installing a skill
    InstallSkill,
}

impl RateLimitedAction {
    /// Get human-readable description
    pub fn description(&self) -> &str {
        match self {
            RateLimitedAction::CreateIdentity => "Create identity",
            RateLimitedAction::CreateListing => "Create listing",
            RateLimitedAction::SubmitJob => "Submit job",
            RateLimitedAction::JoinCoalition => "Join coalition",
            RateLimitedAction::SendPayment => "Send payment",
            RateLimitedAction::InstallSkill => "Install skill",
        }
    }
}

/// Rate limits for marketplace actions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimits {
    /// New identities per day
    pub new_identities_per_day: u32,

    /// Listings per identity per day
    pub listings_per_identity_per_day: u32,

    /// Jobs per new provider per hour
    pub jobs_per_new_provider_per_hour: u32,

    /// Coalitions an entity can join per day
    pub coalition_joins_per_day: u32,

    /// Payments per hour
    pub payments_per_hour: u32,

    /// Skill installs per day
    pub skill_installs_per_day: u32,
}

impl Default for RateLimits {
    fn default() -> Self {
        Self {
            new_identities_per_day: 3,
            listings_per_identity_per_day: 10,
            jobs_per_new_provider_per_hour: 5,
            coalition_joins_per_day: 3,
            payments_per_hour: 60,
            skill_installs_per_day: 20,
        }
    }
}

impl RateLimits {
    /// Get the limit for a specific action
    pub fn get_limit(&self, action: RateLimitedAction) -> u32 {
        match action {
            RateLimitedAction::CreateIdentity => self.new_identities_per_day,
            RateLimitedAction::CreateListing => self.listings_per_identity_per_day,
            RateLimitedAction::SubmitJob => self.jobs_per_new_provider_per_hour,
            RateLimitedAction::JoinCoalition => self.coalition_joins_per_day,
            RateLimitedAction::SendPayment => self.payments_per_hour,
            RateLimitedAction::InstallSkill => self.skill_installs_per_day,
        }
    }

    /// Check if an action would exceed rate limits
    pub fn check_rate_limit(&self, action: RateLimitedAction, current_count: u32) -> bool {
        current_count < self.get_limit(action)
    }
}

/// Tracker for rate-limited actions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitTracker {
    /// Entity being tracked
    pub entity_id: String,

    /// Action type
    pub action: RateLimitedAction,

    /// Count in current window
    pub count: u32,

    /// Window start time
    pub window_start: DateTime<Utc>,

    /// Window duration in seconds
    pub window_seconds: u32,
}

impl RateLimitTracker {
    /// Create a new tracker
    pub fn new(
        entity_id: impl Into<String>,
        action: RateLimitedAction,
        window_seconds: u32,
    ) -> Self {
        Self {
            entity_id: entity_id.into(),
            action,
            count: 0,
            window_start: Utc::now(),
            window_seconds,
        }
    }

    /// Check if the window has expired and reset if needed
    pub fn check_and_reset_window(&mut self) {
        let window_duration = chrono::Duration::seconds(self.window_seconds as i64);
        if Utc::now() >= self.window_start + window_duration {
            self.count = 0;
            self.window_start = Utc::now();
        }
    }

    /// Try to increment the count, returns false if limit exceeded
    pub fn try_increment(&mut self, limits: &RateLimits) -> Result<(), SybilError> {
        self.check_and_reset_window();

        if !limits.check_rate_limit(self.action, self.count) {
            return Err(SybilError::RateLimitExceeded(format!(
                "{} limit exceeded ({}/{})",
                self.action.description(),
                self.count,
                limits.get_limit(self.action)
            )));
        }

        self.count += 1;
        Ok(())
    }

    /// Get remaining actions in current window
    pub fn remaining(&self, limits: &RateLimits) -> u32 {
        let limit = limits.get_limit(self.action);
        limit.saturating_sub(self.count)
    }
}

/// Check if a rate limit would be exceeded
pub fn check_rate_limit(
    entity_id: &str,
    action: RateLimitedAction,
    current_count: u32,
    limits: &RateLimits,
) -> Result<(), SybilError> {
    if limits.check_rate_limit(action, current_count) {
        Ok(())
    } else {
        Err(SybilError::RateLimitExceeded(format!(
            "{} limit exceeded for entity {}: {}/{}",
            action.description(),
            entity_id,
            current_count,
            limits.get_limit(action)
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_release_condition_success_rate() {
        let condition = ReleaseCondition::MinSuccessRate(95);
        assert!(condition.is_met(0.96, false, 30));
        assert!(!condition.is_met(0.90, false, 30));
    }

    #[test]
    fn test_release_condition_no_disputes() {
        let condition = ReleaseCondition::NoActiveDisputes;
        assert!(condition.is_met(0.95, false, 30));
        assert!(!condition.is_met(0.95, true, 30));
    }

    #[test]
    fn test_release_condition_min_age() {
        let condition = ReleaseCondition::MinAgeDays(30);
        assert!(condition.is_met(0.95, false, 45));
        assert!(!condition.is_met(0.95, false, 15));
    }

    #[test]
    fn test_slash_condition_descriptions() {
        assert_eq!(SlashCondition::FraudDetected.default_slash_percent(), 100);
        assert_eq!(SlashCondition::DisputeLoss.default_slash_percent(), 50);
        assert_eq!(
            SlashCondition::RepeatedFailures(5).default_slash_percent(),
            25
        );
    }

    #[test]
    fn test_stake_requirement_defaults() {
        let provider = StakeRequirement::provider_default();
        assert_eq!(provider.min_stake_sats, 100_000);
        assert_eq!(provider.hold_period_days, 30);

        let creator = StakeRequirement::creator_default();
        assert_eq!(creator.min_stake_sats, 50_000);

        let agent = StakeRequirement::agent_default();
        assert_eq!(agent.min_stake_sats, 25_000);
    }

    #[test]
    fn test_stake_creation() {
        let stake = Stake::new("entity1", 100_000, 30);
        assert_eq!(stake.amount_sats, 100_000);
        assert!(stake.status.is_active());
        assert!(stake.locked_until > Utc::now());
    }

    #[test]
    fn test_stake_meets_requirement() {
        let stake = Stake::new("entity1", 100_000, 30);
        let requirement = StakeRequirement::provider_default();
        assert!(stake.meets_requirement(&requirement));

        let small_stake = Stake::new("entity2", 50_000, 30);
        assert!(!small_stake.meets_requirement(&requirement));
    }

    #[test]
    fn test_stake_slash() {
        let mut stake = Stake::new("entity1", 100_000, 30);
        let slashed = stake.slash(SlashCondition::FraudDetected, 50).unwrap();
        assert_eq!(slashed, 50_000);
        assert!(matches!(stake.status, StakeStatus::Slashed(_)));

        // Cannot slash again
        assert!(stake.slash(SlashCondition::DisputeLoss, 25).is_err());
    }

    #[test]
    fn test_proof_of_work_verification() {
        // Test with difficulty 0 (any solution works)
        assert!(verify_solution("test", "anything", 0));

        // Test with difficulty 8 (first byte must be 0)
        let challenge = "test_challenge";
        let solution = solve_challenge(challenge, 8);
        assert!(verify_solution(challenge, &solution, 8));
    }

    #[test]
    fn test_generate_challenge() {
        let challenge1 = generate_challenge();
        let challenge2 = generate_challenge();
        assert_ne!(challenge1, challenge2);
        assert_eq!(challenge1.len(), 64); // 32 bytes = 64 hex chars
    }

    #[test]
    fn test_rate_limits_default() {
        let limits = RateLimits::default();
        assert_eq!(limits.new_identities_per_day, 3);
        assert_eq!(limits.listings_per_identity_per_day, 10);
    }

    #[test]
    fn test_rate_limit_check() {
        let limits = RateLimits::default();

        // Within limits
        assert!(check_rate_limit("entity1", RateLimitedAction::CreateIdentity, 2, &limits).is_ok());

        // At limit
        assert!(
            check_rate_limit("entity1", RateLimitedAction::CreateIdentity, 3, &limits).is_err()
        );

        // Over limit
        assert!(
            check_rate_limit("entity1", RateLimitedAction::CreateIdentity, 5, &limits).is_err()
        );
    }

    #[test]
    fn test_rate_limit_tracker() {
        let limits = RateLimits::default();
        let mut tracker = RateLimitTracker::new("entity1", RateLimitedAction::CreateListing, 86400);

        // Should succeed up to the limit
        for _ in 0..10 {
            assert!(tracker.try_increment(&limits).is_ok());
        }

        // Should fail when limit exceeded
        assert!(tracker.try_increment(&limits).is_err());
    }

    #[test]
    fn test_rate_limit_tracker_remaining() {
        let limits = RateLimits::default();
        let mut tracker = RateLimitTracker::new("entity1", RateLimitedAction::CreateListing, 86400);

        assert_eq!(tracker.remaining(&limits), 10);
        tracker.try_increment(&limits).unwrap();
        assert_eq!(tracker.remaining(&limits), 9);
    }

    #[test]
    fn test_stake_status_checks() {
        assert!(StakeStatus::Active.is_locked());
        assert!(StakeStatus::Active.is_active());
        assert!(StakeStatus::PendingRelease.is_locked());
        assert!(!StakeStatus::PendingRelease.is_active());
        assert!(!StakeStatus::Released.is_locked());
        assert!(
            !StakeStatus::Slashed(SlashRecord::new(1000, SlashCondition::FraudDetected))
                .is_active()
        );
    }

    #[test]
    fn test_slash_record() {
        let record =
            SlashRecord::new(50_000, SlashCondition::DisputeLoss).with_reference("dispute-123");

        assert_eq!(record.amount_sats, 50_000);
        assert_eq!(record.reference_id, Some("dispute-123".to_string()));
    }

    #[test]
    fn test_proof_of_work_submit() {
        let mut pow = ProofOfWork::new("entity1", 0); // difficulty 0 for fast test
        assert!(!pow.is_verified());

        pow.submit_solution("any_solution");
        assert!(pow.is_verified());
    }

    #[test]
    fn test_sybil_error_display() {
        let err = SybilError::InsufficientStake {
            required: 100_000,
            available: 50_000,
        };
        assert!(err.to_string().contains("100000"));
        assert!(err.to_string().contains("50000"));
    }
}
