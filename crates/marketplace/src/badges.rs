//! Verification badge types for marketplace trust
//!
//! This module implements trust badges for identity verification,
//! benchmarks, audits, and other trust signals.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Errors that can occur during badge operations
#[derive(Debug, Clone, Error, PartialEq, Serialize, Deserialize)]
pub enum BadgeError {
    #[error("Badge not found: {0}")]
    NotFound(String),

    #[error("Badge already exists for entity: {0}")]
    AlreadyExists(String),

    #[error("Badge has expired")]
    Expired,

    #[error("Requirements not met: {0}")]
    RequirementsNotMet(String),

    #[error("Invalid badge type: {0}")]
    InvalidType(String),

    #[error("Cannot revoke badge: {0}")]
    CannotRevoke(String),
}

/// Types of verification badges
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BadgeType {
    /// KYC or web-of-trust verified identity
    VerifiedIdentity,
    /// Passes conformance/performance tests
    Benchmarked,
    /// Third-party security audit passed
    Audited,
    /// Platform-created official entity
    Official,
    /// Top 1% performance tier
    Elite,
    /// High install/usage count
    Popular,
}

impl BadgeType {
    /// Get the default trust boost for this badge type
    pub fn default_trust_boost(&self) -> f32 {
        match self {
            BadgeType::VerifiedIdentity => 0.10,
            BadgeType::Benchmarked => 0.08,
            BadgeType::Audited => 0.12,
            BadgeType::Official => 0.15,
            BadgeType::Elite => 0.10,
            BadgeType::Popular => 0.05,
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            BadgeType::VerifiedIdentity => "Identity verified via KYC or web-of-trust",
            BadgeType::Benchmarked => "Passed conformance and performance tests",
            BadgeType::Audited => "Third-party security audit completed",
            BadgeType::Official => "Official platform entity",
            BadgeType::Elite => "Top 1% performance tier",
            BadgeType::Popular => "High usage and install count",
        }
    }

    /// Get icon name for display
    pub fn icon(&self) -> &'static str {
        match self {
            BadgeType::VerifiedIdentity => "shield-check",
            BadgeType::Benchmarked => "chart-bar",
            BadgeType::Audited => "document-check",
            BadgeType::Official => "badge-check",
            BadgeType::Elite => "star",
            BadgeType::Popular => "fire",
        }
    }
}

/// A verification badge held by an entity
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Badge {
    /// Unique badge identifier
    pub id: String,
    /// Type of badge
    pub badge_type: BadgeType,
    /// Entity that holds this badge
    pub entity_id: String,
    /// Trust score boost from this badge (0.0-1.0)
    pub trust_boost: f32,
    /// When the badge was issued
    pub issued_at: DateTime<Utc>,
    /// When the badge expires (None = never)
    pub expires_at: Option<DateTime<Utc>>,
    /// Who issued the badge
    pub issuer: String,
    /// Optional verification data (JSON)
    pub verification_data: Option<Value>,
}

impl Badge {
    /// Create a new badge
    pub fn new(
        badge_type: BadgeType,
        entity_id: impl Into<String>,
        issuer: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            badge_type,
            entity_id: entity_id.into(),
            trust_boost: badge_type.default_trust_boost(),
            issued_at: Utc::now(),
            expires_at: None,
            issuer: issuer.into(),
            verification_data: None,
        }
    }

    /// Set custom trust boost
    pub fn with_trust_boost(mut self, boost: f32) -> Self {
        self.trust_boost = boost.clamp(0.0, 1.0);
        self
    }

    /// Set expiration date
    pub fn with_expiration(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Set expiration from now
    pub fn expires_in_days(mut self, days: u32) -> Self {
        self.expires_at = Some(Utc::now() + chrono::Duration::days(days as i64));
        self
    }

    /// Set verification data
    pub fn with_verification_data(mut self, data: Value) -> Self {
        self.verification_data = Some(data);
        self
    }

    /// Check if badge has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Check if badge is active (issued and not expired)
    pub fn is_active(&self) -> bool {
        !self.is_expired()
    }

    /// Get days until expiration (None if no expiration or already expired)
    pub fn days_until_expiration(&self) -> Option<i64> {
        self.expires_at.and_then(|expires| {
            let duration = expires - Utc::now();
            if duration.num_days() >= 0 {
                Some(duration.num_days())
            } else {
                None
            }
        })
    }
}

/// Requirements needed to obtain a badge
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BadgeRequirement {
    /// Minimum number of completed jobs
    MinJobs(u64),
    /// Minimum success rate (0.0-1.0)
    MinSuccessRate(f32),
    /// Minimum benchmark score (0.0-1.0)
    BenchmarkScore(f32),
    /// Must pass security audit
    AuditPassed,
    /// Must complete KYC verification
    KycCompleted,
    /// Minimum number of attestations
    AttestationCount(u32),
    /// Minimum install count
    MinInstalls(u64),
    /// Minimum days active
    MinDaysActive(u32),
    /// Minimum stake amount in sats
    MinStakeSats(u64),
}

impl BadgeRequirement {
    /// Get human-readable description
    pub fn description(&self) -> String {
        match self {
            BadgeRequirement::MinJobs(n) => format!("Complete at least {} jobs", n),
            BadgeRequirement::MinSuccessRate(r) => {
                format!("Maintain {}% success rate", (r * 100.0) as u32)
            }
            BadgeRequirement::BenchmarkScore(s) => {
                format!("Achieve {}% benchmark score", (s * 100.0) as u32)
            }
            BadgeRequirement::AuditPassed => "Pass security audit".to_string(),
            BadgeRequirement::KycCompleted => "Complete KYC verification".to_string(),
            BadgeRequirement::AttestationCount(n) => format!("Receive {} attestations", n),
            BadgeRequirement::MinInstalls(n) => format!("Reach {} installs", n),
            BadgeRequirement::MinDaysActive(n) => format!("Be active for {} days", n),
            BadgeRequirement::MinStakeSats(n) => format!("Stake at least {} sats", n),
        }
    }
}

/// Requirements configuration for a badge type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BadgeRequirements {
    /// Type of badge
    pub badge_type: BadgeType,
    /// All requirements that must be met
    pub requirements: Vec<BadgeRequirement>,
}

impl BadgeRequirements {
    /// Create new badge requirements
    pub fn new(badge_type: BadgeType) -> Self {
        Self {
            badge_type,
            requirements: Vec::new(),
        }
    }

    /// Add a requirement
    pub fn add_requirement(mut self, requirement: BadgeRequirement) -> Self {
        self.requirements.push(requirement);
        self
    }

    /// Get default requirements for each badge type
    pub fn defaults(badge_type: BadgeType) -> Self {
        match badge_type {
            BadgeType::VerifiedIdentity => {
                Self::new(badge_type).add_requirement(BadgeRequirement::KycCompleted)
            }

            BadgeType::Benchmarked => {
                Self::new(badge_type).add_requirement(BadgeRequirement::BenchmarkScore(0.85))
            }

            BadgeType::Audited => {
                Self::new(badge_type).add_requirement(BadgeRequirement::AuditPassed)
            }

            BadgeType::Official => Self::new(badge_type), // Platform-issued only

            BadgeType::Elite => Self::new(badge_type)
                .add_requirement(BadgeRequirement::MinJobs(100))
                .add_requirement(BadgeRequirement::MinSuccessRate(0.99))
                .add_requirement(BadgeRequirement::MinDaysActive(90)),

            BadgeType::Popular => Self::new(badge_type)
                .add_requirement(BadgeRequirement::MinInstalls(1000))
                .add_requirement(BadgeRequirement::MinDaysActive(30)),
        }
    }
}

/// Identity verification methods
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IdentityVerification {
    /// KYC verification through a provider
    Kyc {
        provider: String,
        verified_at: DateTime<Utc>,
    },
    /// Social attestation from other users
    SocialAttestation {
        attestors: Vec<String>,
        min_required: u32,
    },
    /// Stake-based verification
    StakeBased { stake_sats: u64 },
}

impl IdentityVerification {
    /// Create KYC verification
    pub fn kyc(provider: impl Into<String>) -> Self {
        Self::Kyc {
            provider: provider.into(),
            verified_at: Utc::now(),
        }
    }

    /// Create social attestation verification
    pub fn social_attestation(attestors: Vec<String>, min_required: u32) -> Self {
        Self::SocialAttestation {
            attestors,
            min_required,
        }
    }

    /// Create stake-based verification
    pub fn stake_based(stake_sats: u64) -> Self {
        Self::StakeBased { stake_sats }
    }

    /// Check if verification is satisfied
    pub fn is_verified(&self) -> bool {
        match self {
            IdentityVerification::Kyc { .. } => true,
            IdentityVerification::SocialAttestation {
                attestors,
                min_required,
            } => attestors.len() >= *min_required as usize,
            IdentityVerification::StakeBased { stake_sats } => *stake_sats >= 100_000, // 100k sats min
        }
    }
}

/// Result of a benchmark test
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Entity that was tested
    pub entity_id: String,
    /// Test suite used
    pub test_suite: String,
    /// Accuracy percentage (0.0-1.0)
    pub accuracy_pct: f32,
    /// Whether the benchmark passed
    pub passed: bool,
    /// When the test was run
    pub tested_at: DateTime<Utc>,
    /// When the result expires
    pub expires_at: DateTime<Utc>,
}

impl BenchmarkResult {
    /// Create a new benchmark result
    pub fn new(
        entity_id: impl Into<String>,
        test_suite: impl Into<String>,
        accuracy_pct: f32,
        passed: bool,
    ) -> Self {
        let tested_at = Utc::now();
        Self {
            entity_id: entity_id.into(),
            test_suite: test_suite.into(),
            accuracy_pct: accuracy_pct.clamp(0.0, 1.0),
            passed,
            tested_at,
            expires_at: tested_at + chrono::Duration::days(90),
        }
    }

    /// Set custom expiration period
    pub fn expires_in_days(mut self, days: u32) -> Self {
        self.expires_at = self.tested_at + chrono::Duration::days(days as i64);
        self
    }

    /// Check if result has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if result is valid (passed and not expired)
    pub fn is_valid(&self) -> bool {
        self.passed && !self.is_expired()
    }
}

/// Get all active badges for an entity
pub fn get_badges<'a>(badges: &'a [Badge], entity_id: &str) -> Vec<&'a Badge> {
    badges
        .iter()
        .filter(|b| b.entity_id == entity_id && b.is_active())
        .collect()
}

/// Calculate total trust boost from badges
pub fn calculate_badge_trust_boost(badges: &[Badge]) -> f32 {
    badges
        .iter()
        .filter(|b| b.is_active())
        .map(|b| b.trust_boost)
        .sum::<f32>()
        .min(0.5) // Cap total badge boost at 50%
}

/// Check if a badge is expired
pub fn is_badge_expired(badge: &Badge) -> bool {
    badge.is_expired()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_badge_type_trust_boost() {
        assert!(
            BadgeType::Official.default_trust_boost() > BadgeType::Popular.default_trust_boost()
        );
        assert!(
            BadgeType::Audited.default_trust_boost() > BadgeType::Benchmarked.default_trust_boost()
        );
    }

    #[test]
    fn test_badge_type_description() {
        for badge_type in [
            BadgeType::VerifiedIdentity,
            BadgeType::Benchmarked,
            BadgeType::Audited,
            BadgeType::Official,
            BadgeType::Elite,
            BadgeType::Popular,
        ] {
            assert!(!badge_type.description().is_empty());
            assert!(!badge_type.icon().is_empty());
        }
    }

    #[test]
    fn test_badge_creation() {
        let badge = Badge::new(BadgeType::VerifiedIdentity, "entity1", "platform");
        assert_eq!(badge.badge_type, BadgeType::VerifiedIdentity);
        assert_eq!(badge.entity_id, "entity1");
        assert_eq!(badge.issuer, "platform");
        assert!(!badge.is_expired());
        assert!(badge.is_active());
    }

    #[test]
    fn test_badge_expiration() {
        let badge = Badge::new(BadgeType::Benchmarked, "entity1", "tester").expires_in_days(30);

        assert!(!badge.is_expired());
        assert!(badge.is_active());
        assert!(badge.days_until_expiration().unwrap() >= 29);
    }

    #[test]
    fn test_badge_expired() {
        let mut badge = Badge::new(BadgeType::Benchmarked, "entity1", "tester");
        badge.expires_at = Some(Utc::now() - chrono::Duration::days(1));

        assert!(badge.is_expired());
        assert!(!badge.is_active());
        assert!(badge.days_until_expiration().is_none());
    }

    #[test]
    fn test_badge_trust_boost_clamping() {
        let badge = Badge::new(BadgeType::Elite, "entity1", "platform").with_trust_boost(1.5);

        assert_eq!(badge.trust_boost, 1.0);

        let badge = Badge::new(BadgeType::Elite, "entity1", "platform").with_trust_boost(-0.5);

        assert_eq!(badge.trust_boost, 0.0);
    }

    #[test]
    fn test_badge_requirement_descriptions() {
        let requirements = vec![
            BadgeRequirement::MinJobs(100),
            BadgeRequirement::MinSuccessRate(0.95),
            BadgeRequirement::BenchmarkScore(0.85),
            BadgeRequirement::AuditPassed,
            BadgeRequirement::KycCompleted,
            BadgeRequirement::AttestationCount(5),
        ];

        for req in requirements {
            assert!(!req.description().is_empty());
        }
    }

    #[test]
    fn test_badge_requirements_defaults() {
        let elite_reqs = BadgeRequirements::defaults(BadgeType::Elite);
        assert!(!elite_reqs.requirements.is_empty());
        assert!(
            elite_reqs
                .requirements
                .iter()
                .any(|r| matches!(r, BadgeRequirement::MinJobs(_)))
        );

        let identity_reqs = BadgeRequirements::defaults(BadgeType::VerifiedIdentity);
        assert!(
            identity_reqs
                .requirements
                .iter()
                .any(|r| matches!(r, BadgeRequirement::KycCompleted))
        );
    }

    #[test]
    fn test_identity_verification_kyc() {
        let verification = IdentityVerification::kyc("provider1");
        assert!(verification.is_verified());
    }

    #[test]
    fn test_identity_verification_social() {
        let verification = IdentityVerification::social_attestation(
            vec!["user1".to_string(), "user2".to_string()],
            2,
        );
        assert!(verification.is_verified());

        let insufficient = IdentityVerification::social_attestation(vec!["user1".to_string()], 3);
        assert!(!insufficient.is_verified());
    }

    #[test]
    fn test_identity_verification_stake() {
        let sufficient = IdentityVerification::stake_based(200_000);
        assert!(sufficient.is_verified());

        let insufficient = IdentityVerification::stake_based(50_000);
        assert!(!insufficient.is_verified());
    }

    #[test]
    fn test_benchmark_result() {
        let result = BenchmarkResult::new("entity1", "suite1", 0.92, true);
        assert!(result.is_valid());
        assert!(!result.is_expired());

        let failed = BenchmarkResult::new("entity1", "suite1", 0.50, false);
        assert!(!failed.is_valid());
    }

    #[test]
    fn test_benchmark_result_expiration() {
        let mut result = BenchmarkResult::new("entity1", "suite1", 0.92, true);
        result.expires_at = Utc::now() - chrono::Duration::days(1);

        assert!(result.is_expired());
        assert!(!result.is_valid());
    }

    #[test]
    fn test_calculate_badge_trust_boost() {
        let badges = vec![
            Badge::new(BadgeType::VerifiedIdentity, "entity1", "platform"),
            Badge::new(BadgeType::Benchmarked, "entity1", "tester"),
            Badge::new(BadgeType::Audited, "entity1", "auditor"),
        ];

        let total_boost = calculate_badge_trust_boost(&badges);
        assert!(total_boost > 0.0);
        assert!(total_boost <= 0.5); // Capped at 50%
    }

    #[test]
    fn test_calculate_badge_trust_boost_excludes_expired() {
        let mut badge = Badge::new(BadgeType::Elite, "entity1", "platform");
        badge.expires_at = Some(Utc::now() - chrono::Duration::days(1));

        let badges = vec![badge];
        let total_boost = calculate_badge_trust_boost(&badges);
        assert_eq!(total_boost, 0.0);
    }

    #[test]
    fn test_get_badges_filters_by_entity() {
        let badges = vec![
            Badge::new(BadgeType::VerifiedIdentity, "entity1", "platform"),
            Badge::new(BadgeType::Benchmarked, "entity2", "tester"),
            Badge::new(BadgeType::Audited, "entity1", "auditor"),
        ];

        let entity1_badges = get_badges(&badges, "entity1");
        assert_eq!(entity1_badges.len(), 2);

        let entity2_badges = get_badges(&badges, "entity2");
        assert_eq!(entity2_badges.len(), 1);
    }

    #[test]
    fn test_badge_serde() {
        let badge = Badge::new(BadgeType::Official, "entity1", "platform").expires_in_days(365);

        let json = serde_json::to_string(&badge).unwrap();
        let deserialized: Badge = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.badge_type, badge.badge_type);
        assert_eq!(deserialized.entity_id, badge.entity_id);
    }

    #[test]
    fn test_badge_type_serde() {
        let badge_type = BadgeType::VerifiedIdentity;
        let json = serde_json::to_string(&badge_type).unwrap();
        assert_eq!(json, "\"verified_identity\"");

        let deserialized: BadgeType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, badge_type);
    }
}
