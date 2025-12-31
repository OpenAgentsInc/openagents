//! Provider reputation system for compute marketplace
//!
//! Implements multi-factor reputation tracking with decay, tier system,
//! and tier-based benefits for compute providers.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur in the reputation system
#[derive(Debug, Error)]
pub enum ReputationError {
    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    #[error("Invalid score: {0}")]
    InvalidScore(String),

    #[error("Calculation error: {0}")]
    CalculationError(String),
}

/// Reputation tier classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReputationTier {
    /// New provider: <0.50 overall score, <100 jobs
    New,
    /// Established provider: 0.50-0.79 score, 100+ jobs
    Established,
    /// Trusted provider: 0.80-0.94 score, 500+ jobs, >95% success
    Trusted,
    /// Elite provider: 0.95+ score, 1000+ jobs, >99% success
    Elite,
}

impl ReputationTier {
    /// Get a description of this tier
    pub fn description(&self) -> &'static str {
        match self {
            Self::New => "New provider establishing track record",
            Self::Established => "Established provider with solid history",
            Self::Trusted => "Trusted provider with excellent performance",
            Self::Elite => "Elite provider with outstanding track record",
        }
    }

    /// Get the minimum jobs required for this tier
    pub fn min_jobs(&self) -> u64 {
        match self {
            Self::New => 0,
            Self::Established => 100,
            Self::Trusted => 500,
            Self::Elite => 1000,
        }
    }

    /// Get the minimum score required for this tier
    pub fn min_score(&self) -> f32 {
        match self {
            Self::New => 0.0,
            Self::Established => 0.50,
            Self::Trusted => 0.80,
            Self::Elite => 0.95,
        }
    }

    /// Get the minimum success rate for this tier
    pub fn min_success_rate(&self) -> f32 {
        match self {
            Self::New => 0.0,
            Self::Established => 0.90,
            Self::Trusted => 0.95,
            Self::Elite => 0.99,
        }
    }

    /// Calculate tier from metrics
    pub fn from_metrics(overall_score: f32, jobs_completed: u64, success_rate: f32) -> Self {
        if overall_score >= 0.95 && jobs_completed >= 1000 && success_rate >= 0.99 {
            Self::Elite
        } else if overall_score >= 0.80 && jobs_completed >= 500 && success_rate >= 0.95 {
            Self::Trusted
        } else if overall_score >= 0.50 && jobs_completed >= 100 && success_rate >= 0.90 {
            Self::Established
        } else {
            Self::New
        }
    }
}

/// Track record score component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackRecordScore {
    /// Total jobs completed
    pub jobs_completed: u64,

    /// Success rate (0.0 to 1.0)
    pub success_rate: f32,

    /// Average latency in milliseconds
    pub avg_latency_ms: u32,

    /// Uptime percentage (0.0 to 1.0)
    pub uptime_pct: f32,

    /// Provider age in days
    pub age_days: u32,
}

impl TrackRecordScore {
    /// Create a new track record score
    pub fn new(
        jobs_completed: u64,
        success_rate: f32,
        avg_latency_ms: u32,
        uptime_pct: f32,
        age_days: u32,
    ) -> Self {
        Self {
            jobs_completed,
            success_rate,
            avg_latency_ms,
            uptime_pct,
            age_days,
        }
    }

    /// Calculate the track record score (0.0 to 1.0)
    pub fn calculate(&self) -> f32 {
        // Weight components
        let success_weight = 0.40;
        let uptime_weight = 0.30;
        let latency_weight = 0.20;
        let maturity_weight = 0.10;

        // Success rate contribution
        let success_score = self.success_rate;

        // Uptime contribution
        let uptime_score = self.uptime_pct;

        // Latency contribution (normalized, lower is better)
        let latency_score = if self.avg_latency_ms <= 100 {
            1.0
        } else if self.avg_latency_ms >= 5000 {
            0.0
        } else {
            1.0 - ((self.avg_latency_ms - 100) as f32 / 4900.0)
        };

        // Maturity contribution (capped at 365 days)
        let maturity_score = self.age_days.min(365) as f32 / 365.0;

        // Weighted average
        (success_score * success_weight
            + uptime_score * uptime_weight
            + latency_score * latency_weight
            + maturity_score * maturity_weight)
            .clamp(0.0, 1.0)
    }
}

/// Social score component (endorsements, reviews)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialScore {
    /// Number of endorsements from other providers
    pub endorsements: u32,

    /// Average review rating (0.0 to 5.0)
    pub avg_rating: f32,

    /// Total number of reviews
    pub review_count: u32,
}

impl SocialScore {
    /// Create a new social score
    pub fn new(endorsements: u32, avg_rating: f32, review_count: u32) -> Self {
        Self {
            endorsements,
            avg_rating,
            review_count,
        }
    }

    /// Calculate the social score (0.0 to 1.0)
    pub fn calculate(&self) -> f32 {
        // Normalize rating to 0-1
        let rating_score = (self.avg_rating / 5.0).clamp(0.0, 1.0);

        // Endorsement bonus (diminishing returns)
        let endorsement_score =
            (self.endorsements as f32 / (self.endorsements as f32 + 10.0)).clamp(0.0, 1.0);

        // Review count weight (more reviews = more reliable)
        let review_weight = (self.review_count.min(50) as f32 / 50.0).clamp(0.0, 1.0);

        // Combined score with weights
        (rating_score * 0.6 + endorsement_score * 0.4) * (0.5 + review_weight * 0.5)
    }
}

impl Default for SocialScore {
    fn default() -> Self {
        Self {
            endorsements: 0,
            avg_rating: 0.0,
            review_count: 0,
        }
    }
}

/// Economic score component (payment history, disputes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomicScore {
    /// Total earnings in satoshis
    pub total_earnings_sats: u64,

    /// Number of disputes filed
    pub disputes_filed: u32,

    /// Number of disputes lost
    pub disputes_lost: u32,

    /// Payment reliability (0.0 to 1.0)
    pub payment_reliability: f32,
}

impl EconomicScore {
    /// Create a new economic score
    pub fn new(
        total_earnings_sats: u64,
        disputes_filed: u32,
        disputes_lost: u32,
        payment_reliability: f32,
    ) -> Self {
        Self {
            total_earnings_sats,
            disputes_filed,
            disputes_lost,
            payment_reliability,
        }
    }

    /// Calculate the economic score (0.0 to 1.0)
    pub fn calculate(&self) -> f32 {
        // Payment reliability is the main factor
        let payment_score = self.payment_reliability;

        // Dispute penalty
        let dispute_rate = if self.disputes_filed == 0 {
            0.0
        } else {
            self.disputes_lost as f32 / self.disputes_filed as f32
        };
        let dispute_penalty = dispute_rate * 0.3;

        // Economic activity bonus (diminishing returns)
        let earnings_btc = self.total_earnings_sats as f32 / 100_000_000.0;
        let activity_bonus = (earnings_btc / (earnings_btc + 1.0)).clamp(0.0, 0.2);

        (payment_score - dispute_penalty + activity_bonus).clamp(0.0, 1.0)
    }
}

impl Default for EconomicScore {
    fn default() -> Self {
        Self {
            total_earnings_sats: 0,
            disputes_filed: 0,
            disputes_lost: 0,
            payment_reliability: 1.0,
        }
    }
}

/// Verification score component
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VerificationScore {
    /// Identity verified
    pub identity_verified: bool,

    /// Payment method verified
    pub payment_verified: bool,

    /// Compute capacity verified
    pub capacity_verified: bool,

    /// KYC completed (optional)
    pub kyc_completed: bool,
}

impl VerificationScore {
    /// Create a new verification score
    pub fn new(
        identity_verified: bool,
        payment_verified: bool,
        capacity_verified: bool,
        kyc_completed: bool,
    ) -> Self {
        Self {
            identity_verified,
            payment_verified,
            capacity_verified,
            kyc_completed,
        }
    }

    /// Calculate the verification score (0.0 to 1.0)
    pub fn calculate(&self) -> f32 {
        let mut score = 0.0;

        if self.identity_verified {
            score += 0.30;
        }
        if self.payment_verified {
            score += 0.30;
        }
        if self.capacity_verified {
            score += 0.30;
        }
        if self.kyc_completed {
            score += 0.10;
        }

        score
    }
}

/// Complete provider reputation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderReputation {
    /// Provider ID
    pub provider_id: String,

    /// Overall reputation score (0.0 to 1.0)
    pub overall_score: f32,

    /// Current tier
    pub tier: ReputationTier,

    /// Track record component
    pub track_record: TrackRecordScore,

    /// Social component
    pub social: SocialScore,

    /// Economic component
    pub economic: EconomicScore,

    /// Verification component
    pub verification: VerificationScore,

    /// When last updated
    pub last_updated: DateTime<Utc>,
}

impl ProviderReputation {
    /// Create a new provider reputation
    pub fn new(provider_id: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            overall_score: 0.0,
            tier: ReputationTier::New,
            track_record: TrackRecordScore::new(0, 0.0, 0, 0.0, 0),
            social: SocialScore::default(),
            economic: EconomicScore::default(),
            verification: VerificationScore::default(),
            last_updated: Utc::now(),
        }
    }

    /// Calculate the overall reputation score
    pub fn calculate_overall(&mut self) {
        // Component weights
        let track_record_weight = 0.50;
        let social_weight = 0.20;
        let economic_weight = 0.20;
        let verification_weight = 0.10;

        // Calculate component scores
        let track_record_score = self.track_record.calculate();
        let social_score = self.social.calculate();
        let economic_score = self.economic.calculate();
        let verification_score = self.verification.calculate();

        // Weighted average
        self.overall_score = (track_record_score * track_record_weight
            + social_score * social_weight
            + economic_score * economic_weight
            + verification_score * verification_weight)
            .clamp(0.0, 1.0);

        // Update tier
        self.tier = ReputationTier::from_metrics(
            self.overall_score,
            self.track_record.jobs_completed,
            self.track_record.success_rate,
        );

        self.last_updated = Utc::now();
    }

    /// Apply decay for inactivity (5% per week)
    pub fn apply_decay(&mut self, days_inactive: u32) {
        let weeks = days_inactive as f32 / 7.0;
        let decay_factor = 0.95_f32.powf(weeks);

        self.overall_score = (self.overall_score * decay_factor).clamp(0.0, 1.0);
        self.track_record.uptime_pct =
            (self.track_record.uptime_pct * decay_factor).clamp(0.0, 1.0);

        self.last_updated = Utc::now();
    }

    /// Apply penalty for lost dispute
    pub fn apply_dispute_penalty(&mut self) {
        self.economic.disputes_lost += 1;
        self.economic.payment_reliability = (self.economic.payment_reliability - 0.05).max(0.0);
        self.calculate_overall();
    }
}

/// Benefits for each reputation tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierBenefits {
    /// The tier these benefits apply to
    pub tier: ReputationTier,

    /// Maximum jobs per day (None = unlimited)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_volume_limit: Option<u32>,

    /// Whether escrow is required for jobs
    pub escrow_required: bool,

    /// Priority in job routing
    pub priority_routing: bool,

    /// Can endorse other providers
    pub can_endorse_others: bool,

    /// Featured placement in marketplace
    pub featured_placement: bool,

    /// Commission rate (0.0 to 1.0)
    pub commission_rate: f32,
}

impl TierBenefits {
    /// Get benefits for a tier
    pub fn for_tier(tier: ReputationTier) -> Self {
        match tier {
            ReputationTier::New => Self {
                tier,
                job_volume_limit: Some(10),
                escrow_required: true,
                priority_routing: false,
                can_endorse_others: false,
                featured_placement: false,
                commission_rate: 0.15,
            },
            ReputationTier::Established => Self {
                tier,
                job_volume_limit: Some(100),
                escrow_required: true,
                priority_routing: false,
                can_endorse_others: false,
                featured_placement: false,
                commission_rate: 0.10,
            },
            ReputationTier::Trusted => Self {
                tier,
                job_volume_limit: None,
                escrow_required: false,
                priority_routing: true,
                can_endorse_others: true,
                featured_placement: false,
                commission_rate: 0.05,
            },
            ReputationTier::Elite => Self {
                tier,
                job_volume_limit: None,
                escrow_required: false,
                priority_routing: true,
                can_endorse_others: true,
                featured_placement: true,
                commission_rate: 0.03,
            },
        }
    }

    /// Check if a job count is within the tier limit
    pub fn within_limit(&self, job_count: u32) -> bool {
        self.job_volume_limit.is_none_or(|limit| job_count < limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reputation_tier_properties() {
        assert_eq!(ReputationTier::New.min_jobs(), 0);
        assert_eq!(ReputationTier::Established.min_jobs(), 100);
        assert_eq!(ReputationTier::Trusted.min_score(), 0.80);
        assert_eq!(ReputationTier::Elite.min_success_rate(), 0.99);
    }

    #[test]
    fn test_reputation_tier_from_metrics() {
        let tier = ReputationTier::from_metrics(0.96, 1500, 0.995);
        assert_eq!(tier, ReputationTier::Elite);

        let tier = ReputationTier::from_metrics(0.85, 600, 0.96);
        assert_eq!(tier, ReputationTier::Trusted);

        let tier = ReputationTier::from_metrics(0.60, 150, 0.92);
        assert_eq!(tier, ReputationTier::Established);

        let tier = ReputationTier::from_metrics(0.40, 50, 0.85);
        assert_eq!(tier, ReputationTier::New);
    }

    #[test]
    fn test_track_record_score() {
        let track_record = TrackRecordScore::new(1000, 0.98, 150, 0.99, 180);
        let score = track_record.calculate();

        assert!(score > 0.9);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_social_score() {
        let social = SocialScore::new(25, 4.5, 40);
        let score = social.calculate();

        assert!(score > 0.7);
        assert!(score <= 1.0);

        let no_social = SocialScore::default();
        assert_eq!(no_social.calculate(), 0.0);
    }

    #[test]
    fn test_economic_score() {
        let economic = EconomicScore::new(1_000_000_000, 10, 1, 0.95);
        let score = economic.calculate();

        assert!(score > 0.8);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_verification_score() {
        let full_verification = VerificationScore::new(true, true, true, true);
        assert_eq!(full_verification.calculate(), 1.0);

        let partial = VerificationScore::new(true, true, false, false);
        assert_eq!(partial.calculate(), 0.6);

        let none = VerificationScore::default();
        assert_eq!(none.calculate(), 0.0);
    }

    #[test]
    fn test_provider_reputation_calculation() {
        let mut reputation = ProviderReputation::new("provider1");

        reputation.track_record = TrackRecordScore::new(500, 0.96, 200, 0.98, 120);
        reputation.social = SocialScore::new(15, 4.2, 30);
        reputation.economic = EconomicScore::new(500_000_000, 5, 0, 0.98);
        reputation.verification = VerificationScore::new(true, true, true, false);

        reputation.calculate_overall();

        assert!(reputation.overall_score > 0.8);
        assert!(reputation.overall_score <= 1.0);
        assert!(reputation.tier >= ReputationTier::Trusted);
    }

    #[test]
    fn test_reputation_decay() {
        let mut reputation = ProviderReputation::new("provider2");
        reputation.overall_score = 0.9;
        reputation.track_record.uptime_pct = 0.99;

        // Apply 2 weeks of decay (5% per week = ~9.75% total)
        reputation.apply_decay(14);

        assert!(reputation.overall_score < 0.9);
        assert!(reputation.overall_score > 0.8);
    }

    #[test]
    fn test_dispute_penalty() {
        let mut reputation = ProviderReputation::new("provider3");
        reputation.economic.payment_reliability = 0.95;

        reputation.apply_dispute_penalty();

        assert_eq!(reputation.economic.disputes_lost, 1);
        assert_eq!(reputation.economic.payment_reliability, 0.90);
    }

    #[test]
    fn test_tier_benefits() {
        let new_benefits = TierBenefits::for_tier(ReputationTier::New);
        assert_eq!(new_benefits.job_volume_limit, Some(10));
        assert!(new_benefits.escrow_required);
        assert!(!new_benefits.priority_routing);
        assert_eq!(new_benefits.commission_rate, 0.15);

        let elite_benefits = TierBenefits::for_tier(ReputationTier::Elite);
        assert_eq!(elite_benefits.job_volume_limit, None);
        assert!(!elite_benefits.escrow_required);
        assert!(elite_benefits.priority_routing);
        assert!(elite_benefits.featured_placement);
        assert_eq!(elite_benefits.commission_rate, 0.03);
    }

    #[test]
    fn test_tier_benefits_limits() {
        let new_benefits = TierBenefits::for_tier(ReputationTier::New);
        assert!(new_benefits.within_limit(5));
        assert!(!new_benefits.within_limit(15));

        let elite_benefits = TierBenefits::for_tier(ReputationTier::Elite);
        assert!(elite_benefits.within_limit(1000)); // No limit
    }

    #[test]
    fn test_provider_reputation_serde() {
        let reputation = ProviderReputation::new("provider1");
        let json = serde_json::to_string(&reputation).unwrap();
        let deserialized: ProviderReputation = serde_json::from_str(&json).unwrap();

        assert_eq!(reputation.provider_id, deserialized.provider_id);
        assert_eq!(reputation.tier, deserialized.tier);
    }
}
