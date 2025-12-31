//! Reputation scoring algorithm for marketplace participants
//!
//! This module provides the reputation scoring system that powers trust and discovery
//! in the marketplace. It integrates the provider reputation system with Nostr NIP-32
//! labels and provides CLI access.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::provider_reputation::{ProviderReputation, ReputationTier};

/// Reputation scorer with configurable weights
pub trait ReputationScorer: Send + Sync {
    /// Calculate overall reputation score from components
    fn calculate_score(&self, reputation: &mut ProviderReputation);

    /// Apply time-based decay for inactivity
    fn apply_decay(&self, reputation: &mut ProviderReputation, days_inactive: u32);

    /// Determine if a provider score meets minimum requirements
    fn meets_requirements(&self, reputation: &ProviderReputation, tier: ReputationTier) -> bool;
}

/// Default reputation scorer with standard weights
pub struct DefaultReputationScorer {
    /// Weight for track record component (0.0-1.0)
    pub track_record_weight: f32,

    /// Weight for social component (0.0-1.0)
    pub social_weight: f32,

    /// Weight for economic component (0.0-1.0)
    pub economic_weight: f32,

    /// Weight for verification component (0.0-1.0)
    pub verification_weight: f32,

    /// Decay rate per week (0.0-1.0)
    pub weekly_decay_rate: f32,
}

impl Default for DefaultReputationScorer {
    fn default() -> Self {
        Self {
            track_record_weight: 0.50,
            social_weight: 0.20,
            economic_weight: 0.20,
            verification_weight: 0.10,
            weekly_decay_rate: 0.05, // 5% per week
        }
    }
}

impl ReputationScorer for DefaultReputationScorer {
    fn calculate_score(&self, reputation: &mut ProviderReputation) {
        // Calculate component scores
        let track_record_score = reputation.track_record.calculate();
        let social_score = reputation.social.calculate();
        let economic_score = reputation.economic.calculate();
        let verification_score = reputation.verification.calculate();

        // Weighted average
        reputation.overall_score = (track_record_score * self.track_record_weight
            + social_score * self.social_weight
            + economic_score * self.economic_weight
            + verification_score * self.verification_weight)
            .clamp(0.0, 1.0);

        // Update tier based on metrics
        reputation.tier = ReputationTier::from_metrics(
            reputation.overall_score,
            reputation.track_record.jobs_completed,
            reputation.track_record.success_rate,
        );

        reputation.last_updated = Utc::now();
    }

    fn apply_decay(&self, reputation: &mut ProviderReputation, days_inactive: u32) {
        let weeks = days_inactive as f32 / 7.0;
        let decay_factor = (1.0 - self.weekly_decay_rate).powf(weeks);

        reputation.overall_score = (reputation.overall_score * decay_factor).clamp(0.0, 1.0);
        reputation.track_record.uptime_pct =
            (reputation.track_record.uptime_pct * decay_factor).clamp(0.0, 1.0);

        reputation.last_updated = Utc::now();
    }

    fn meets_requirements(&self, reputation: &ProviderReputation, tier: ReputationTier) -> bool {
        reputation.overall_score >= tier.min_score()
            && reputation.track_record.jobs_completed >= tier.min_jobs()
            && reputation.track_record.success_rate >= tier.min_success_rate()
    }
}

/// Anti-gaming detection for suspicious patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiGamingDetector {
    /// Minimum job value for reputation credit (sats)
    pub min_job_value_sats: u64,

    /// Maximum jobs per hour (rate limiting)
    pub max_jobs_per_hour: u32,

    /// Minimum unique consumers required
    pub min_unique_consumers: u32,

    /// Sybil stake requirement (sats)
    pub sybil_stake_sats: u64,
}

impl Default for AntiGamingDetector {
    fn default() -> Self {
        Self {
            min_job_value_sats: 100, // ~$0.04 at $40k BTC
            max_jobs_per_hour: 100,
            min_unique_consumers: 5,
            sybil_stake_sats: 100_000, // ~$40 at $40k BTC
        }
    }
}

impl AntiGamingDetector {
    /// Detect suspicious job farming patterns
    pub fn detect_job_farming(
        &self,
        jobs: &[(DateTime<Utc>, u64)], // (timestamp, value_sats)
        unique_consumers: u32,
    ) -> bool {
        // Check for too many low-value jobs
        let low_value_count = jobs
            .iter()
            .filter(|(_, value)| *value < self.min_job_value_sats)
            .count();

        if low_value_count > jobs.len() / 2 {
            return true; // More than half are low-value
        }

        // Check for rate limiting violation
        let one_hour_ago = Utc::now() - Duration::hours(1);
        let recent_jobs = jobs.iter().filter(|(ts, _)| *ts > one_hour_ago).count();

        if recent_jobs > self.max_jobs_per_hour as usize {
            return true; // Too many jobs in short time
        }

        // Check for insufficient consumer diversity
        if unique_consumers < self.min_unique_consumers {
            return true; // Not enough unique consumers
        }

        false
    }

    /// Check if provider has met sybil resistance stake requirement
    pub fn meets_stake_requirement(&self, total_earnings_sats: u64) -> bool {
        total_earnings_sats >= self.sybil_stake_sats
    }
}

/// Reputation event for tracking history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationEvent {
    /// Provider pubkey
    pub provider_id: String,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,

    /// Event type
    pub event_type: ReputationEventType,

    /// Score before event
    pub score_before: f32,

    /// Score after event
    pub score_after: f32,

    /// Optional reason/description
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReputationEventType {
    /// Job completed successfully
    JobCompleted,

    /// Job failed
    JobFailed,

    /// Endorsement received
    EndorsementReceived,

    /// Review received
    ReviewReceived,

    /// Dispute lost
    DisputeLost,

    /// Dispute won
    DisputeWon,

    /// Verification completed
    VerificationCompleted,

    /// Decay applied
    DecayApplied,

    /// Tier upgrade
    TierUpgraded,

    /// Tier downgrade
    TierDowngraded,
}

/// Reputation manager for tracking and updating scores
pub struct ReputationManager {
    scorer: Box<dyn ReputationScorer>,
    anti_gaming: AntiGamingDetector,
    reputations: HashMap<String, ProviderReputation>,
    events: Vec<ReputationEvent>,
}

impl ReputationManager {
    /// Create a new reputation manager with default scorer
    pub fn new() -> Self {
        Self {
            scorer: Box::new(DefaultReputationScorer::default()),
            anti_gaming: AntiGamingDetector::default(),
            reputations: HashMap::new(),
            events: Vec::new(),
        }
    }

    /// Create with custom scorer
    pub fn with_scorer(scorer: Box<dyn ReputationScorer>) -> Self {
        Self {
            scorer,
            anti_gaming: AntiGamingDetector::default(),
            reputations: HashMap::new(),
            events: Vec::new(),
        }
    }

    /// Get or create reputation for provider
    pub fn get_or_create(&mut self, provider_id: impl Into<String>) -> &mut ProviderReputation {
        let provider_id = provider_id.into();
        self.reputations
            .entry(provider_id.clone())
            .or_insert_with(|| ProviderReputation::new(provider_id))
    }

    /// Update reputation after job completion
    pub fn record_job_completion(
        &mut self,
        provider_id: &str,
        success: bool,
        value_sats: u64,
        latency_ms: u32,
    ) {
        // Update reputation stats
        let score_before = {
            let reputation = self.get_or_create(provider_id);
            let score = reputation.overall_score;

            // Update track record
            reputation.track_record.jobs_completed += 1;

            let total_jobs = reputation.track_record.jobs_completed;
            let prev_successes =
                (reputation.track_record.success_rate * (total_jobs - 1) as f32) as u64;
            let new_successes = if success {
                prev_successes + 1
            } else {
                prev_successes
            };
            reputation.track_record.success_rate = new_successes as f32 / total_jobs as f32;

            // Update average latency (weighted moving average)
            let alpha = 0.1; // Weight for new observation
            reputation.track_record.avg_latency_ms =
                ((1.0 - alpha) * reputation.track_record.avg_latency_ms as f32
                    + alpha * latency_ms as f32) as u32;

            // Update economic score
            reputation.economic.total_earnings_sats += value_sats;

            score
        };

        // Recalculate score (separate borrow)
        let score_after = {
            let reputation = self.reputations.get_mut(provider_id).unwrap();
            self.scorer.calculate_score(reputation);
            reputation.overall_score
        };

        // Record event
        let event = ReputationEvent {
            provider_id: provider_id.to_string(),
            timestamp: Utc::now(),
            event_type: if success {
                ReputationEventType::JobCompleted
            } else {
                ReputationEventType::JobFailed
            },
            score_before,
            score_after,
            reason: None,
        };
        self.events.push(event);
    }

    /// Record endorsement from another provider
    pub fn record_endorsement(&mut self, provider_id: &str, endorser_tier: ReputationTier) {
        // Update endorsements
        let score_before = {
            let reputation = self.get_or_create(provider_id);
            let score = reputation.overall_score;

            // Endorsements from higher tiers count more
            let endorsement_weight = match endorser_tier {
                ReputationTier::Elite => 3,
                ReputationTier::Trusted => 2,
                ReputationTier::Established => 1,
                ReputationTier::New => 0, // New providers can't endorse
            };

            reputation.social.endorsements += endorsement_weight;
            score
        };

        // Recalculate score
        let score_after = {
            let reputation = self.reputations.get_mut(provider_id).unwrap();
            self.scorer.calculate_score(reputation);
            reputation.overall_score
        };

        let event = ReputationEvent {
            provider_id: provider_id.to_string(),
            timestamp: Utc::now(),
            event_type: ReputationEventType::EndorsementReceived,
            score_before,
            score_after,
            reason: Some(format!("Endorsed by {:?} tier provider", endorser_tier)),
        };
        self.events.push(event);
    }

    /// Record consumer review
    pub fn record_review(&mut self, provider_id: &str, rating: f32) {
        // Update reviews
        let score_before = {
            let reputation = self.get_or_create(provider_id);
            let score = reputation.overall_score;

            let total_reviews = reputation.social.review_count;
            let total_rating = reputation.social.avg_rating * total_reviews as f32;

            reputation.social.review_count += 1;
            reputation.social.avg_rating =
                (total_rating + rating) / reputation.social.review_count as f32;

            score
        };

        // Recalculate score
        let score_after = {
            let reputation = self.reputations.get_mut(provider_id).unwrap();
            self.scorer.calculate_score(reputation);
            reputation.overall_score
        };

        let event = ReputationEvent {
            provider_id: provider_id.to_string(),
            timestamp: Utc::now(),
            event_type: ReputationEventType::ReviewReceived,
            score_before,
            score_after,
            reason: Some(format!("Received {:.1}/5.0 rating", rating)),
        };
        self.events.push(event);
    }

    /// Apply decay for inactive providers
    pub fn apply_decay_to_all(&mut self, days_inactive_threshold: u32) {
        let now = Utc::now();

        for (provider_id, reputation) in self.reputations.iter_mut() {
            let days_inactive = (now - reputation.last_updated).num_days() as u32;

            if days_inactive >= days_inactive_threshold {
                let score_before = reputation.overall_score;
                self.scorer.apply_decay(reputation, days_inactive);

                let event = ReputationEvent {
                    provider_id: provider_id.clone(),
                    timestamp: now,
                    event_type: ReputationEventType::DecayApplied,
                    score_before,
                    score_after: reputation.overall_score,
                    reason: Some(format!("{} days inactive", days_inactive)),
                };
                self.events.push(event);
            }
        }
    }

    /// Get reputation for provider
    pub fn get(&self, provider_id: &str) -> Option<&ProviderReputation> {
        self.reputations.get(provider_id)
    }

    /// Get all reputations sorted by score
    pub fn get_all_sorted(&self) -> Vec<&ProviderReputation> {
        let mut reps: Vec<&ProviderReputation> = self.reputations.values().collect();
        reps.sort_by(|a, b| b.overall_score.partial_cmp(&a.overall_score).unwrap());
        reps
    }

    /// Get reputations by tier
    pub fn get_by_tier(&self, tier: ReputationTier) -> Vec<&ProviderReputation> {
        self.reputations
            .values()
            .filter(|r| r.tier == tier)
            .collect()
    }

    /// Get recent reputation events for provider
    pub fn get_events(&self, provider_id: &str, limit: usize) -> Vec<&ReputationEvent> {
        self.events
            .iter()
            .rev()
            .filter(|e| e.provider_id == provider_id)
            .take(limit)
            .collect()
    }

    /// Check for gaming patterns
    pub fn check_gaming_patterns(
        &self,
        _provider_id: &str,
        jobs: &[(DateTime<Utc>, u64)],
        unique_consumers: u32,
    ) -> bool {
        self.anti_gaming.detect_job_farming(jobs, unique_consumers)
    }
}

impl Default for ReputationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_reputation::{
        EconomicScore, SocialScore, TrackRecordScore, VerificationScore,
    };

    #[test]
    fn test_default_scorer_calculation() {
        let scorer = DefaultReputationScorer::default();
        let mut reputation = ProviderReputation::new("provider1");

        reputation.track_record = TrackRecordScore::new(100, 0.95, 200, 0.98, 60);
        reputation.social = SocialScore::new(10, 4.0, 20);
        reputation.economic = EconomicScore::new(100_000_000, 2, 0, 0.97);
        reputation.verification = VerificationScore::new(true, true, true, false);

        scorer.calculate_score(&mut reputation);

        assert!(reputation.overall_score > 0.8);
        assert!(reputation.overall_score <= 1.0);
    }

    #[test]
    fn test_decay_application() {
        let scorer = DefaultReputationScorer::default();
        let mut reputation = ProviderReputation::new("provider2");
        reputation.overall_score = 0.9;
        reputation.track_record.uptime_pct = 0.99;

        scorer.apply_decay(&mut reputation, 14); // 2 weeks

        assert!(reputation.overall_score < 0.9);
        assert!(reputation.overall_score > 0.8);
    }

    #[test]
    fn test_anti_gaming_job_farming() {
        let detector = AntiGamingDetector::default();

        // Suspicious: many low-value jobs
        let suspicious_jobs: Vec<_> = (0..100)
            .map(|_| (Utc::now(), 10u64)) // All below minimum
            .collect();

        assert!(detector.detect_job_farming(&suspicious_jobs, 10));

        // Legitimate: normal value distribution
        let legit_jobs: Vec<_> = (0..100)
            .map(|i| (Utc::now(), (i * 100 + 200) as u64))
            .collect();

        assert!(!detector.detect_job_farming(&legit_jobs, 10));
    }

    #[test]
    fn test_anti_gaming_rate_limiting() {
        let detector = AntiGamingDetector::default();

        // Too many jobs in short time
        let rapid_jobs: Vec<_> = (0..150).map(|_| (Utc::now(), 1000u64)).collect();

        assert!(detector.detect_job_farming(&rapid_jobs, 10));
    }

    #[test]
    fn test_anti_gaming_consumer_diversity() {
        let detector = AntiGamingDetector::default();

        let jobs: Vec<_> = (0..50).map(|_| (Utc::now(), 1000u64)).collect();

        // Too few unique consumers
        assert!(detector.detect_job_farming(&jobs, 2));

        // Sufficient diversity
        assert!(!detector.detect_job_farming(&jobs, 10));
    }

    #[test]
    fn test_reputation_manager_job_completion() {
        let mut manager = ReputationManager::new();

        manager.record_job_completion("provider1", true, 10_000, 150);
        manager.record_job_completion("provider1", true, 15_000, 120);
        manager.record_job_completion("provider1", false, 0, 500);

        let reputation = manager.get("provider1").unwrap();

        assert_eq!(reputation.track_record.jobs_completed, 3);
        assert!((reputation.track_record.success_rate - 0.6667).abs() < 0.01);
        assert!(reputation.economic.total_earnings_sats == 25_000);
    }

    #[test]
    fn test_reputation_manager_endorsement() {
        let mut manager = ReputationManager::new();

        manager.record_endorsement("provider1", ReputationTier::Elite);
        manager.record_endorsement("provider1", ReputationTier::Trusted);

        let reputation = manager.get("provider1").unwrap();

        // Elite worth 3, Trusted worth 2
        assert_eq!(reputation.social.endorsements, 5);
    }

    #[test]
    fn test_reputation_manager_reviews() {
        let mut manager = ReputationManager::new();

        manager.record_review("provider1", 5.0);
        manager.record_review("provider1", 4.0);
        manager.record_review("provider1", 4.5);

        let reputation = manager.get("provider1").unwrap();

        assert_eq!(reputation.social.review_count, 3);
        assert!((reputation.social.avg_rating - 4.5).abs() < 0.01);
    }

    #[test]
    fn test_reputation_manager_sorting() {
        let mut manager = ReputationManager::new();

        manager.get_or_create("provider1").overall_score = 0.7;
        manager.get_or_create("provider2").overall_score = 0.9;
        manager.get_or_create("provider3").overall_score = 0.5;

        let sorted = manager.get_all_sorted();

        assert_eq!(sorted[0].provider_id, "provider2");
        assert_eq!(sorted[1].provider_id, "provider1");
        assert_eq!(sorted[2].provider_id, "provider3");
    }

    #[test]
    fn test_reputation_events_tracking() {
        let mut manager = ReputationManager::new();

        manager.record_job_completion("provider1", true, 10_000, 150);
        manager.record_endorsement("provider1", ReputationTier::Trusted);
        manager.record_review("provider1", 4.5);

        let events = manager.get_events("provider1", 10);

        assert_eq!(events.len(), 3);
        assert!(matches!(
            events[0].event_type,
            ReputationEventType::ReviewReceived
        ));
        assert!(matches!(
            events[1].event_type,
            ReputationEventType::EndorsementReceived
        ));
        assert!(matches!(
            events[2].event_type,
            ReputationEventType::JobCompleted
        ));
    }
}
