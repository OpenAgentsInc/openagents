//! NIP-32 label integration for marketplace reputation
//!
//! Implements reputation tracking using NIP-32 labels for marketplace participants.
//! Labels are published to Nostr relays and aggregated for trust scoring.
//!
//! # Label Namespaces
//!
//! - `com.openagents.reputation.job` - Job completion labels (success/failure)
//! - `com.openagents.reputation.trust` - Trust tier labels (new/established/trusted/expert)
//! - `com.openagents.reputation.skill` - Skill quality labels
//! - `com.openagents.reputation.review` - Review labels with ratings
//!
//! # Trust Tiers
//!
//! - **New**: 0-10 jobs, any success rate
//! - **Established**: 10-50 jobs, >80% success rate
//! - **Trusted**: 50-200 jobs, >90% success rate
//! - **Expert**: 200+ jobs, >95% success rate

use nostr::nip32::{Label, LabelEvent, LabelTarget};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Namespace for job completion labels
pub const NS_JOB: &str = "com.openagents.reputation.job";

/// Namespace for trust tier labels
pub const NS_TRUST: &str = "com.openagents.reputation.trust";

/// Namespace for skill quality labels
pub const NS_SKILL: &str = "com.openagents.reputation.skill";

/// Namespace for review labels
pub const NS_REVIEW: &str = "com.openagents.reputation.review";

/// Trust tier for marketplace participants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrustTier {
    /// New provider (0-10 jobs)
    New,
    /// Established provider (10-50 jobs, >80% success)
    Established,
    /// Trusted provider (50-200 jobs, >90% success)
    Trusted,
    /// Expert provider (200+ jobs, >95% success)
    Expert,
}

impl TrustTier {
    /// Get tier as string for labels
    pub fn as_str(&self) -> &'static str {
        match self {
            TrustTier::New => "new",
            TrustTier::Established => "established",
            TrustTier::Trusted => "trusted",
            TrustTier::Expert => "expert",
        }
    }

    /// Parse tier from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "new" => Some(TrustTier::New),
            "established" => Some(TrustTier::Established),
            "trusted" => Some(TrustTier::Trusted),
            "expert" => Some(TrustTier::Expert),
            _ => None,
        }
    }

    /// Calculate tier from metrics
    pub fn from_metrics(jobs_completed: u32, success_rate: f32) -> Self {
        if jobs_completed >= 200 && success_rate >= 0.95 {
            TrustTier::Expert
        } else if jobs_completed >= 50 && success_rate >= 0.90 {
            TrustTier::Trusted
        } else if jobs_completed >= 10 && success_rate >= 0.80 {
            TrustTier::Established
        } else {
            TrustTier::New
        }
    }

    /// Get minimum jobs required for this tier
    pub fn min_jobs(&self) -> u32 {
        match self {
            TrustTier::New => 0,
            TrustTier::Established => 10,
            TrustTier::Trusted => 50,
            TrustTier::Expert => 200,
        }
    }

    /// Get minimum success rate required for this tier
    pub fn min_success_rate(&self) -> f32 {
        match self {
            TrustTier::New => 0.0,
            TrustTier::Established => 0.80,
            TrustTier::Trusted => 0.90,
            TrustTier::Expert => 0.95,
        }
    }

    /// Get reputation weight for discovery ranking
    pub fn discovery_weight(&self) -> f32 {
        match self {
            TrustTier::New => 1.0,
            TrustTier::Established => 1.5,
            TrustTier::Trusted => 2.0,
            TrustTier::Expert => 3.0,
        }
    }
}

/// Job completion status for labels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Success,
    Failure,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Success => "success",
            JobStatus::Failure => "failure",
        }
    }
}

/// Skill quality rating for labels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillQuality {
    Excellent,
    Good,
    Average,
    Poor,
}

impl SkillQuality {
    pub fn as_str(&self) -> &'static str {
        match self {
            SkillQuality::Excellent => "excellent",
            SkillQuality::Good => "good",
            SkillQuality::Average => "average",
            SkillQuality::Poor => "poor",
        }
    }

    pub fn from_rating(rating: f32) -> Self {
        if rating >= 4.5 {
            SkillQuality::Excellent
        } else if rating >= 3.5 {
            SkillQuality::Good
        } else if rating >= 2.5 {
            SkillQuality::Average
        } else {
            SkillQuality::Poor
        }
    }
}

/// Reputation label builder for marketplace events
pub struct ReputationLabel;

impl ReputationLabel {
    /// Create a job completion label
    pub fn job_completion(provider_pubkey: &str, job_id: &str, status: JobStatus) -> LabelEvent {
        let label = Label::new(status.as_str(), NS_JOB);
        let target = LabelTarget::pubkey(provider_pubkey, None::<String>);

        LabelEvent::new(vec![label], vec![target]).with_content(format!(
            "Job {} {}",
            job_id,
            status.as_str()
        ))
    }

    /// Create a trust tier label
    pub fn trust_tier(
        provider_pubkey: &str,
        tier: TrustTier,
        jobs_completed: u32,
        success_rate: f32,
    ) -> LabelEvent {
        let label = Label::new(tier.as_str(), NS_TRUST);
        let target = LabelTarget::pubkey(provider_pubkey, None::<String>);

        LabelEvent::new(vec![label], vec![target]).with_content(format!(
            "{} tier: {} jobs, {:.1}% success rate",
            tier.as_str(),
            jobs_completed,
            success_rate * 100.0
        ))
    }

    /// Create a skill quality label
    pub fn skill_quality(
        skill_id: &str,
        provider_pubkey: &str,
        quality: SkillQuality,
    ) -> LabelEvent {
        let label = Label::new(quality.as_str(), NS_SKILL);
        let targets = vec![
            LabelTarget::event(skill_id, None::<String>),
            LabelTarget::pubkey(provider_pubkey, None::<String>),
        ];

        LabelEvent::new(vec![label], targets)
            .with_content(format!("Skill quality: {}", quality.as_str()))
    }

    /// Create a review label with rating
    pub fn review(
        provider_pubkey: &str,
        job_id: &str,
        rating: f32,
        comment: Option<String>,
    ) -> LabelEvent {
        let rating_str = format!("{:.1}", rating.clamp(1.0, 5.0));
        let label = Label::new(&rating_str, NS_REVIEW);
        let target = LabelTarget::pubkey(provider_pubkey, None::<String>);

        let content = if let Some(c) = comment {
            format!("Job {}: {:.1}/5.0 - {}", job_id, rating, c)
        } else {
            format!("Job {}: {:.1}/5.0", job_id, rating)
        };

        LabelEvent::new(vec![label], vec![target]).with_content(content)
    }
}

/// Aggregated reputation metrics from NIP-32 labels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationMetrics {
    /// Provider public key
    pub provider_pubkey: String,
    /// Total jobs completed
    pub jobs_completed: u32,
    /// Successful jobs
    pub jobs_succeeded: u32,
    /// Failed jobs
    pub jobs_failed: u32,
    /// Success rate (0.0-1.0)
    pub success_rate: f32,
    /// Current trust tier
    pub trust_tier: TrustTier,
    /// Average review rating (1.0-5.0)
    pub avg_rating: f32,
    /// Total reviews received
    pub review_count: u32,
    /// Skill quality ratings
    pub skill_ratings: HashMap<String, SkillQuality>,
}

impl ReputationMetrics {
    /// Create new metrics for provider
    pub fn new(provider_pubkey: impl Into<String>) -> Self {
        Self {
            provider_pubkey: provider_pubkey.into(),
            jobs_completed: 0,
            jobs_succeeded: 0,
            jobs_failed: 0,
            success_rate: 0.0,
            trust_tier: TrustTier::New,
            avg_rating: 0.0,
            review_count: 0,
            skill_ratings: HashMap::new(),
        }
    }

    /// Record a job completion
    pub fn record_job(&mut self, success: bool) {
        self.jobs_completed += 1;
        if success {
            self.jobs_succeeded += 1;
        } else {
            self.jobs_failed += 1;
        }
        self.success_rate = self.jobs_succeeded as f32 / self.jobs_completed as f32;
        self.trust_tier = TrustTier::from_metrics(self.jobs_completed, self.success_rate);
    }

    /// Record a review
    pub fn record_review(&mut self, rating: f32) {
        let total = self.avg_rating * self.review_count as f32;
        self.review_count += 1;
        self.avg_rating = (total + rating) / self.review_count as f32;
    }

    /// Record a skill quality rating
    pub fn record_skill_quality(&mut self, skill_id: impl Into<String>, quality: SkillQuality) {
        self.skill_ratings.insert(skill_id.into(), quality);
    }

    /// Get discovery ranking weight
    pub fn discovery_weight(&self) -> f32 {
        let tier_weight = self.trust_tier.discovery_weight();
        let rating_weight = if self.review_count > 0 {
            self.avg_rating / 5.0
        } else {
            0.8 // Default neutral weight
        };
        tier_weight * rating_weight
    }
}

/// Reputation aggregator for processing NIP-32 labels
pub struct ReputationAggregator {
    metrics: HashMap<String, ReputationMetrics>,
}

impl ReputationAggregator {
    /// Create a new aggregator
    pub fn new() -> Self {
        Self {
            metrics: HashMap::new(),
        }
    }

    /// Process a job completion label
    pub fn process_job_label(&mut self, label: &LabelEvent) {
        for target in &label.targets {
            if let LabelTarget::Pubkey { pubkey, .. } = target {
                let metrics = self
                    .metrics
                    .entry(pubkey.clone())
                    .or_insert_with(|| ReputationMetrics::new(pubkey));

                for l in &label.labels {
                    if l.namespace.as_deref() == Some(NS_JOB) {
                        let success = l.value == "success";
                        metrics.record_job(success);
                    }
                }
            }
        }
    }

    /// Process a review label
    pub fn process_review_label(&mut self, label: &LabelEvent) {
        for target in &label.targets {
            if let LabelTarget::Pubkey { pubkey, .. } = target {
                let metrics = self
                    .metrics
                    .entry(pubkey.clone())
                    .or_insert_with(|| ReputationMetrics::new(pubkey));

                for l in &label.labels {
                    if l.namespace.as_deref() == Some(NS_REVIEW) {
                        if let Ok(rating) = l.value.parse::<f32>() {
                            metrics.record_review(rating);
                        }
                    }
                }
            }
        }
    }

    /// Process a skill quality label
    pub fn process_skill_label(&mut self, label: &LabelEvent) {
        let mut skill_id = None;
        let mut provider_pubkey = None;

        for target in &label.targets {
            match target {
                LabelTarget::Event { id, .. } => skill_id = Some(id.clone()),
                LabelTarget::Pubkey { pubkey, .. } => provider_pubkey = Some(pubkey.clone()),
                _ => {}
            }
        }

        if let (Some(skill), Some(pubkey)) = (skill_id, provider_pubkey) {
            let metrics = self
                .metrics
                .entry(pubkey.clone())
                .or_insert_with(|| ReputationMetrics::new(&pubkey));

            for l in &label.labels {
                if l.namespace.as_deref() == Some(NS_SKILL) {
                    let quality = match l.value.as_str() {
                        "excellent" => SkillQuality::Excellent,
                        "good" => SkillQuality::Good,
                        "average" => SkillQuality::Average,
                        "poor" => SkillQuality::Poor,
                        _ => continue,
                    };
                    metrics.record_skill_quality(skill.clone(), quality);
                }
            }
        }
    }

    /// Get metrics for provider
    pub fn get_metrics(&self, provider_pubkey: &str) -> Option<&ReputationMetrics> {
        self.metrics.get(provider_pubkey)
    }

    /// Get all metrics sorted by discovery weight
    pub fn get_all_ranked(&self) -> Vec<&ReputationMetrics> {
        let mut all: Vec<&ReputationMetrics> = self.metrics.values().collect();
        all.sort_by(|a, b| {
            b.discovery_weight()
                .partial_cmp(&a.discovery_weight())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        all
    }

    /// Get providers by trust tier
    pub fn get_by_tier(&self, tier: TrustTier) -> Vec<&ReputationMetrics> {
        self.metrics
            .values()
            .filter(|m| m.trust_tier == tier)
            .collect()
    }
}

impl Default for ReputationAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trust_tier_from_metrics() {
        assert_eq!(TrustTier::from_metrics(5, 0.90), TrustTier::New);
        assert_eq!(TrustTier::from_metrics(15, 0.85), TrustTier::Established);
        assert_eq!(TrustTier::from_metrics(100, 0.92), TrustTier::Trusted);
        assert_eq!(TrustTier::from_metrics(250, 0.96), TrustTier::Expert);
    }

    #[test]
    fn test_trust_tier_requirements() {
        assert_eq!(TrustTier::New.min_jobs(), 0);
        assert_eq!(TrustTier::Established.min_jobs(), 10);
        assert_eq!(TrustTier::Trusted.min_jobs(), 50);
        assert_eq!(TrustTier::Expert.min_jobs(), 200);

        assert_eq!(TrustTier::New.min_success_rate(), 0.0);
        assert_eq!(TrustTier::Established.min_success_rate(), 0.80);
        assert_eq!(TrustTier::Trusted.min_success_rate(), 0.90);
        assert_eq!(TrustTier::Expert.min_success_rate(), 0.95);
    }

    #[test]
    fn test_trust_tier_discovery_weight() {
        assert_eq!(TrustTier::New.discovery_weight(), 1.0);
        assert_eq!(TrustTier::Established.discovery_weight(), 1.5);
        assert_eq!(TrustTier::Trusted.discovery_weight(), 2.0);
        assert_eq!(TrustTier::Expert.discovery_weight(), 3.0);
    }

    #[test]
    fn test_skill_quality_from_rating() {
        assert_eq!(SkillQuality::from_rating(4.8), SkillQuality::Excellent);
        assert_eq!(SkillQuality::from_rating(4.0), SkillQuality::Good);
        assert_eq!(SkillQuality::from_rating(3.0), SkillQuality::Average);
        assert_eq!(SkillQuality::from_rating(2.0), SkillQuality::Poor);
    }

    #[test]
    fn test_job_completion_label() {
        let label = ReputationLabel::job_completion("pubkey123", "job-1", JobStatus::Success);

        assert_eq!(label.labels.len(), 1);
        assert_eq!(label.labels[0].value, "success");
        assert_eq!(label.labels[0].namespace, Some(NS_JOB.to_string()));
        assert_eq!(label.targets.len(), 1);
    }

    #[test]
    fn test_trust_tier_label() {
        let label = ReputationLabel::trust_tier("pubkey123", TrustTier::Trusted, 100, 0.92);

        assert_eq!(label.labels.len(), 1);
        assert_eq!(label.labels[0].value, "trusted");
        assert_eq!(label.labels[0].namespace, Some(NS_TRUST.to_string()));
        assert!(label.content.contains("100 jobs"));
        assert!(label.content.contains("92.0%"));
    }

    #[test]
    fn test_skill_quality_label() {
        let label = ReputationLabel::skill_quality("skill-1", "pubkey123", SkillQuality::Excellent);

        assert_eq!(label.labels.len(), 1);
        assert_eq!(label.labels[0].value, "excellent");
        assert_eq!(label.labels[0].namespace, Some(NS_SKILL.to_string()));
        assert_eq!(label.targets.len(), 2);
    }

    #[test]
    fn test_review_label() {
        let label =
            ReputationLabel::review("pubkey123", "job-1", 4.5, Some("Great work!".to_string()));

        assert_eq!(label.labels.len(), 1);
        assert_eq!(label.labels[0].value, "4.5");
        assert_eq!(label.labels[0].namespace, Some(NS_REVIEW.to_string()));
        assert!(label.content.contains("4.5/5.0"));
        assert!(label.content.contains("Great work!"));
    }

    #[test]
    fn test_reputation_metrics_job_recording() {
        let mut metrics = ReputationMetrics::new("pubkey123");

        metrics.record_job(true);
        metrics.record_job(true);
        metrics.record_job(false);

        assert_eq!(metrics.jobs_completed, 3);
        assert_eq!(metrics.jobs_succeeded, 2);
        assert_eq!(metrics.jobs_failed, 1);
        assert!((metrics.success_rate - 0.6667).abs() < 0.01);
    }

    #[test]
    fn test_reputation_metrics_review_recording() {
        let mut metrics = ReputationMetrics::new("pubkey123");

        metrics.record_review(5.0);
        metrics.record_review(4.0);
        metrics.record_review(4.5);

        assert_eq!(metrics.review_count, 3);
        assert!((metrics.avg_rating - 4.5).abs() < 0.01);
    }

    #[test]
    fn test_reputation_metrics_discovery_weight() {
        let mut metrics = ReputationMetrics::new("pubkey123");

        // New provider with no reviews
        assert_eq!(metrics.discovery_weight(), 0.8);

        // Established provider with good reviews
        for _ in 0..15 {
            metrics.record_job(true);
        }
        metrics.record_review(4.5);

        assert_eq!(metrics.trust_tier, TrustTier::Established);
        let weight = metrics.discovery_weight();
        assert!(weight > 1.0); // Better than new providers
    }

    #[test]
    fn test_reputation_aggregator_job_processing() {
        let mut aggregator = ReputationAggregator::new();

        let label = ReputationLabel::job_completion("pubkey123", "job-1", JobStatus::Success);
        aggregator.process_job_label(&label);

        let metrics = aggregator.get_metrics("pubkey123").unwrap();
        assert_eq!(metrics.jobs_completed, 1);
        assert_eq!(metrics.jobs_succeeded, 1);
    }

    #[test]
    fn test_reputation_aggregator_review_processing() {
        let mut aggregator = ReputationAggregator::new();

        let label = ReputationLabel::review("pubkey123", "job-1", 4.5, None);
        aggregator.process_review_label(&label);

        let metrics = aggregator.get_metrics("pubkey123").unwrap();
        assert_eq!(metrics.review_count, 1);
        assert_eq!(metrics.avg_rating, 4.5);
    }

    #[test]
    fn test_reputation_aggregator_ranking() {
        let mut aggregator = ReputationAggregator::new();

        // Create providers with different metrics
        for _ in 0..5 {
            let label = ReputationLabel::job_completion("provider1", "job", JobStatus::Success);
            aggregator.process_job_label(&label);
        }

        for _ in 0..20 {
            let label = ReputationLabel::job_completion("provider2", "job", JobStatus::Success);
            aggregator.process_job_label(&label);
        }

        let ranked = aggregator.get_all_ranked();
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].provider_pubkey, "provider2"); // More jobs = higher rank
        assert_eq!(ranked[1].provider_pubkey, "provider1");
    }

    #[test]
    fn test_reputation_aggregator_by_tier() {
        let mut aggregator = ReputationAggregator::new();

        // Create new provider
        let label = ReputationLabel::job_completion("new_provider", "job", JobStatus::Success);
        aggregator.process_job_label(&label);

        // Create established provider
        for _ in 0..15 {
            let label =
                ReputationLabel::job_completion("established_provider", "job", JobStatus::Success);
            aggregator.process_job_label(&label);
        }

        let new_tier = aggregator.get_by_tier(TrustTier::New);
        let established_tier = aggregator.get_by_tier(TrustTier::Established);

        assert_eq!(new_tier.len(), 1);
        assert_eq!(established_tier.len(), 1);
    }
}
