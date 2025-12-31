//! Trust score calculation types
//!
//! This module provides comprehensive trust scoring for marketplace entities
//! including providers, creators, and agents with weighted component scoring.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during trust score operations
#[derive(Debug, Clone, Error, PartialEq, Serialize, Deserialize)]
pub enum TrustError {
    #[error("Invalid score: {0}")]
    InvalidScore(String),

    #[error("Invalid component weight: {0}")]
    InvalidWeight(String),

    #[error("Insufficient data for calculation")]
    InsufficientData,
}

/// Type of entity being scored
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityType {
    /// Compute provider
    Provider,

    /// Skill creator
    Creator,

    /// Autonomous agent
    Agent,
}

impl EntityType {
    /// Get human-readable description
    pub fn description(&self) -> &str {
        match self {
            EntityType::Provider => "Provider",
            EntityType::Creator => "Creator",
            EntityType::Agent => "Agent",
        }
    }
}

/// Trust tier based on overall score
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TrustTier {
    /// New entity, score < 0.50
    New,

    /// Established entity, score 0.50-0.79
    Established,

    /// Trusted entity, score 0.80-0.94
    Trusted,

    /// Elite entity, score 0.95+
    Elite,
}

impl TrustTier {
    /// Get tier from score
    pub fn from_score(score: f32) -> Self {
        if score >= 0.95 {
            TrustTier::Elite
        } else if score >= 0.80 {
            TrustTier::Trusted
        } else if score >= 0.50 {
            TrustTier::Established
        } else {
            TrustTier::New
        }
    }

    /// Get minimum score for this tier
    pub fn min_score(&self) -> f32 {
        match self {
            TrustTier::New => 0.0,
            TrustTier::Established => 0.50,
            TrustTier::Trusted => 0.80,
            TrustTier::Elite => 0.95,
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> &str {
        match self {
            TrustTier::New => "New",
            TrustTier::Established => "Established",
            TrustTier::Trusted => "Trusted",
            TrustTier::Elite => "Elite",
        }
    }

    /// Check if tier qualifies for feature access
    pub fn has_feature_access(&self) -> bool {
        matches!(self, TrustTier::Trusted | TrustTier::Elite)
    }
}

/// Track record component of trust score
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackRecordComponent {
    /// Number of jobs completed
    pub jobs_completed: u64,

    /// Success rate (0.0-1.0)
    pub success_rate: f32,

    /// Average rating (0.0-5.0)
    pub avg_rating: f32,

    /// Account age in days
    pub age_days: u32,

    /// Dispute rate (0.0-1.0)
    pub dispute_rate: f32,
}

impl TrackRecordComponent {
    /// Calculate track record score (0.0-1.0)
    pub fn calculate_score(&self) -> f32 {
        // Success rate: 40%
        let success_score = self.success_rate * 0.4;

        // Rating: 30% (normalized from 0-5 to 0-1)
        let rating_score = (self.avg_rating / 5.0) * 0.3;

        // Experience (jobs + age): 20%
        let jobs_score = (self.jobs_completed.min(100) as f32 / 100.0) * 0.1;
        let age_score = (self.age_days.min(365) as f32 / 365.0) * 0.1;

        // Low dispute rate: 10%
        let dispute_score = (1.0 - self.dispute_rate) * 0.1;

        (success_score + rating_score + jobs_score + age_score + dispute_score).min(1.0)
    }
}

/// Social component of trust score
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocialComponent {
    /// Total endorsement count
    pub endorsement_count: u32,

    /// Endorsements from trusted entities
    pub endorsements_from_trusted: u32,

    /// Follower count
    pub follower_count: u32,

    /// NIP-89 recommendations
    pub nip89_recommendations: u32,
}

impl SocialComponent {
    /// Calculate social score (0.0-1.0)
    pub fn calculate_score(&self) -> f32 {
        // Trusted endorsements: 50% (more valuable)
        let trusted_score = (self.endorsements_from_trusted.min(20) as f32 / 20.0) * 0.5;

        // Total endorsements: 20%
        let endorsement_score = (self.endorsement_count.min(50) as f32 / 50.0) * 0.2;

        // NIP-89 recommendations: 20%
        let nip89_score = (self.nip89_recommendations.min(10) as f32 / 10.0) * 0.2;

        // Followers: 10%
        let follower_score = (self.follower_count.min(1000) as f32 / 1000.0) * 0.1;

        (trusted_score + endorsement_score + nip89_score + follower_score).min(1.0)
    }
}

/// Economic component of trust score
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicComponent {
    /// Stake amount in satoshis
    pub stake_sats: u64,

    /// Total earnings history
    pub earnings_history_sats: u64,

    /// Payment reliability (0.0-1.0)
    pub payment_reliability: u8,

    /// Days active in marketplace
    pub active_days: u32,
}

impl EconomicComponent {
    /// Calculate economic score (0.0-1.0)
    pub fn calculate_score(&self) -> f32 {
        // Stake: 40% (1M sats = max)
        let stake_score = (self.stake_sats.min(1_000_000) as f32 / 1_000_000.0) * 0.4;

        // Earnings history: 30% (10M sats = max)
        let earnings_score =
            (self.earnings_history_sats.min(10_000_000) as f32 / 10_000_000.0) * 0.3;

        // Payment reliability: 20%
        let reliability_score = (self.payment_reliability as f32 / 100.0) * 0.2;

        // Activity: 10%
        let activity_score = (self.active_days.min(180) as f32 / 180.0) * 0.1;

        (stake_score + earnings_score + reliability_score + activity_score).min(1.0)
    }
}

/// Verification component of trust score
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationComponent {
    /// Identity verified
    pub identity_verified: bool,

    /// Benchmarked for performance
    pub benchmarked: bool,

    /// Code audited
    pub audited: bool,

    /// Official/certified
    pub official: bool,
}

impl VerificationComponent {
    /// Calculate verification score (0.0-1.0)
    pub fn calculate_score(&self) -> f32 {
        let mut score = 0.0;

        // Identity: 40%
        if self.identity_verified {
            score += 0.4;
        }

        // Benchmarked: 25%
        if self.benchmarked {
            score += 0.25;
        }

        // Audited: 25%
        if self.audited {
            score += 0.25;
        }

        // Official: 10%
        if self.official {
            score += 0.1;
        }

        score
    }
}

/// Weighted trust score components
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrustComponents {
    /// Track record score (0.0-1.0)
    pub track_record: f32,

    /// Social score (0.0-1.0)
    pub social: f32,

    /// Economic score (0.0-1.0)
    pub economic: f32,

    /// Verification score (0.0-1.0)
    pub verification: f32,
}

impl TrustComponents {
    /// Create new components
    pub fn new(track_record: f32, social: f32, economic: f32, verification: f32) -> Self {
        Self {
            track_record: track_record.clamp(0.0, 1.0),
            social: social.clamp(0.0, 1.0),
            economic: economic.clamp(0.0, 1.0),
            verification: verification.clamp(0.0, 1.0),
        }
    }

    /// Calculate weighted overall score
    /// Weights: track_record (35%), social (25%), economic (25%), verification (15%)
    pub fn calculate_overall(&self) -> f32 {
        let score = (self.track_record * 0.35)
            + (self.social * 0.25)
            + (self.economic * 0.25)
            + (self.verification * 0.15);
        score.clamp(0.0, 1.0)
    }
}

/// Benefits available for each trust tier
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TierBenefits {
    /// Can participate in marketplace
    pub can_participate: bool,

    /// Can receive priority jobs
    pub priority_jobs: bool,

    /// Can earn higher rates
    pub premium_rates: bool,

    /// Reduced marketplace fees
    pub fee_discount_pct: u8,

    /// Access to beta features
    pub beta_access: bool,

    /// Featured in marketplace
    pub featured: bool,
}

impl TierBenefits {
    /// Get benefits for a trust tier
    pub fn for_tier(tier: TrustTier) -> Self {
        match tier {
            TrustTier::New => Self {
                can_participate: true,
                priority_jobs: false,
                premium_rates: false,
                fee_discount_pct: 0,
                beta_access: false,
                featured: false,
            },
            TrustTier::Established => Self {
                can_participate: true,
                priority_jobs: false,
                premium_rates: false,
                fee_discount_pct: 5,
                beta_access: false,
                featured: false,
            },
            TrustTier::Trusted => Self {
                can_participate: true,
                priority_jobs: true,
                premium_rates: true,
                fee_discount_pct: 10,
                beta_access: true,
                featured: false,
            },
            TrustTier::Elite => Self {
                can_participate: true,
                priority_jobs: true,
                premium_rates: true,
                fee_discount_pct: 15,
                beta_access: true,
                featured: true,
            },
        }
    }
}

/// Complete trust score for an entity
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrustScore {
    /// Entity identifier
    pub entity_id: String,

    /// Type of entity
    pub entity_type: EntityType,

    /// Overall trust score (0.0-1.0)
    pub overall_score: f32,

    /// Trust tier
    pub tier: TrustTier,

    /// Component scores
    pub components: TrustComponents,

    /// When score was last calculated
    pub last_calculated: DateTime<Utc>,
}

impl TrustScore {
    /// Create a new trust score
    pub fn new(
        entity_id: impl Into<String>,
        entity_type: EntityType,
        components: TrustComponents,
    ) -> Self {
        let overall_score = components.calculate_overall();
        let tier = TrustTier::from_score(overall_score);

        Self {
            entity_id: entity_id.into(),
            entity_type,
            overall_score,
            tier,
            components,
            last_calculated: Utc::now(),
        }
    }

    /// Get tier benefits
    pub fn benefits(&self) -> TierBenefits {
        TierBenefits::for_tier(self.tier)
    }

    /// Check if score qualifies for a feature
    pub fn qualifies_for(&self, required_tier: TrustTier) -> bool {
        self.tier >= required_tier
    }

    /// Recalculate overall score from components
    pub fn recalculate(&mut self) {
        self.overall_score = self.components.calculate_overall();
        self.tier = TrustTier::from_score(self.overall_score);
        self.last_calculated = Utc::now();
    }
}

/// Calculate trust score from component data
pub fn calculate_trust_score(
    entity_id: impl Into<String>,
    entity_type: EntityType,
    track_record: TrackRecordComponent,
    social: SocialComponent,
    economic: EconomicComponent,
    verification: VerificationComponent,
) -> TrustScore {
    let components = TrustComponents::new(
        track_record.calculate_score(),
        social.calculate_score(),
        economic.calculate_score(),
        verification.calculate_score(),
    );

    TrustScore::new(entity_id, entity_type, components)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_type_description() {
        assert_eq!(EntityType::Provider.description(), "Provider");
        assert_eq!(EntityType::Creator.description(), "Creator");
    }

    #[test]
    fn test_trust_tier_from_score() {
        assert_eq!(TrustTier::from_score(0.3), TrustTier::New);
        assert_eq!(TrustTier::from_score(0.6), TrustTier::Established);
        assert_eq!(TrustTier::from_score(0.85), TrustTier::Trusted);
        assert_eq!(TrustTier::from_score(0.97), TrustTier::Elite);
    }

    #[test]
    fn test_trust_tier_min_score() {
        assert_eq!(TrustTier::New.min_score(), 0.0);
        assert_eq!(TrustTier::Established.min_score(), 0.50);
        assert_eq!(TrustTier::Trusted.min_score(), 0.80);
        assert_eq!(TrustTier::Elite.min_score(), 0.95);
    }

    #[test]
    fn test_trust_tier_feature_access() {
        assert!(!TrustTier::New.has_feature_access());
        assert!(!TrustTier::Established.has_feature_access());
        assert!(TrustTier::Trusted.has_feature_access());
        assert!(TrustTier::Elite.has_feature_access());
    }

    #[test]
    fn test_track_record_score() {
        let track_record = TrackRecordComponent {
            jobs_completed: 100,
            success_rate: 0.95,
            avg_rating: 4.5,
            age_days: 180,
            dispute_rate: 0.02,
        };

        let score = track_record.calculate_score();
        assert!(score > 0.8);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_track_record_poor_score() {
        let track_record = TrackRecordComponent {
            jobs_completed: 5,
            success_rate: 0.6,
            avg_rating: 2.0,
            age_days: 10,
            dispute_rate: 0.3,
        };

        let score = track_record.calculate_score();
        assert!(score < 0.5);
    }

    #[test]
    fn test_social_score() {
        let social = SocialComponent {
            endorsement_count: 30,
            endorsements_from_trusted: 10,
            follower_count: 500,
            nip89_recommendations: 5,
        };

        let score = social.calculate_score();
        // 10/20*0.5 + 30/50*0.2 + 5/10*0.2 + 500/1000*0.1 = 0.25 + 0.12 + 0.1 + 0.05 = 0.52
        assert!(score > 0.5);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_social_minimal_score() {
        let social = SocialComponent {
            endorsement_count: 0,
            endorsements_from_trusted: 0,
            follower_count: 0,
            nip89_recommendations: 0,
        };

        let score = social.calculate_score();
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_economic_score() {
        let economic = EconomicComponent {
            stake_sats: 500_000,
            earnings_history_sats: 5_000_000,
            payment_reliability: 95,
            active_days: 120,
        };

        let score = economic.calculate_score();
        assert!(score > 0.6);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_economic_minimal_score() {
        let economic = EconomicComponent {
            stake_sats: 0,
            earnings_history_sats: 0,
            payment_reliability: 0,
            active_days: 0,
        };

        let score = economic.calculate_score();
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_verification_full_score() {
        let verification = VerificationComponent {
            identity_verified: true,
            benchmarked: true,
            audited: true,
            official: true,
        };

        let score = verification.calculate_score();
        assert_eq!(score, 1.0);
    }

    #[test]
    fn test_verification_partial_score() {
        let verification = VerificationComponent {
            identity_verified: true,
            benchmarked: false,
            audited: true,
            official: false,
        };

        let score = verification.calculate_score();
        assert_eq!(score, 0.65);
    }

    #[test]
    fn test_trust_components_overall() {
        let components = TrustComponents::new(0.8, 0.7, 0.75, 0.9);
        let overall = components.calculate_overall();

        // 0.8*0.35 + 0.7*0.25 + 0.75*0.25 + 0.9*0.15 = 0.7775
        assert!((overall - 0.7775).abs() < 0.01);
    }

    #[test]
    fn test_trust_components_clamping() {
        let components = TrustComponents::new(1.5, -0.2, 0.5, 0.8);
        assert_eq!(components.track_record, 1.0);
        assert_eq!(components.social, 0.0);
    }

    #[test]
    fn test_tier_benefits_new() {
        let benefits = TierBenefits::for_tier(TrustTier::New);
        assert!(benefits.can_participate);
        assert!(!benefits.priority_jobs);
        assert!(!benefits.featured);
        assert_eq!(benefits.fee_discount_pct, 0);
    }

    #[test]
    fn test_tier_benefits_elite() {
        let benefits = TierBenefits::for_tier(TrustTier::Elite);
        assert!(benefits.can_participate);
        assert!(benefits.priority_jobs);
        assert!(benefits.premium_rates);
        assert!(benefits.featured);
        assert_eq!(benefits.fee_discount_pct, 15);
    }

    #[test]
    fn test_trust_score_creation() {
        let components = TrustComponents::new(0.85, 0.75, 0.8, 0.9);
        let score = TrustScore::new("entity1", EntityType::Provider, components);

        assert_eq!(score.entity_id, "entity1");
        assert_eq!(score.tier, TrustTier::Trusted);
        assert!(score.overall_score > 0.8);
    }

    #[test]
    fn test_trust_score_qualifies() {
        let components = TrustComponents::new(0.9, 0.85, 0.88, 1.0);
        let score = TrustScore::new("entity1", EntityType::Provider, components);

        assert!(score.qualifies_for(TrustTier::New));
        assert!(score.qualifies_for(TrustTier::Established));
        assert!(score.qualifies_for(TrustTier::Trusted));
        assert!(!score.qualifies_for(TrustTier::Elite)); // Score is ~0.89, needs 0.95+
    }

    #[test]
    fn test_trust_score_recalculate() {
        let components = TrustComponents::new(0.4, 0.3, 0.35, 0.5);
        let mut score = TrustScore::new("entity1", EntityType::Creator, components);
        let old_tier = score.tier;
        assert_eq!(old_tier, TrustTier::New); // 0.4*0.35 + 0.3*0.25 + 0.35*0.25 + 0.5*0.15 = 0.3775

        // Update components to push into Trusted tier
        score.components.track_record = 0.9;
        score.components.social = 0.85;
        score.components.economic = 0.8;
        score.components.verification = 0.9;
        score.recalculate();

        assert!(score.overall_score > 0.8);
        assert_eq!(score.tier, TrustTier::Trusted);
        assert!(score.tier > old_tier);
    }

    #[test]
    fn test_calculate_trust_score_function() {
        let track_record = TrackRecordComponent {
            jobs_completed: 80,
            success_rate: 0.92,
            avg_rating: 4.3,
            age_days: 150,
            dispute_rate: 0.03,
        };

        let social = SocialComponent {
            endorsement_count: 25,
            endorsements_from_trusted: 8,
            follower_count: 300,
            nip89_recommendations: 4,
        };

        let economic = EconomicComponent {
            stake_sats: 400_000,
            earnings_history_sats: 3_000_000,
            payment_reliability: 88,
            active_days: 90,
        };

        let verification = VerificationComponent {
            identity_verified: true,
            benchmarked: true,
            audited: false,
            official: false,
        };

        let score = calculate_trust_score(
            "provider1",
            EntityType::Provider,
            track_record,
            social,
            economic,
            verification,
        );

        assert_eq!(score.entity_id, "provider1");
        assert!(score.overall_score > 0.6);
        assert!(score.tier >= TrustTier::Established);
    }

    #[test]
    fn test_trust_score_serde() {
        let components = TrustComponents::new(0.8, 0.7, 0.75, 0.9);
        let score = TrustScore::new("entity1", EntityType::Agent, components);

        let json = serde_json::to_string(&score).unwrap();
        let deserialized: TrustScore = serde_json::from_str(&json).unwrap();
        assert_eq!(score.entity_id, deserialized.entity_id);
        assert_eq!(score.tier, deserialized.tier);
    }
}
