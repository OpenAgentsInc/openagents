//! Reputation - Trust scoring for agent trading
//!
//! This module provides reputation scoring based on trade attestations,
//! enabling trust-based settlement decisions and counterparty selection.
//!
//! # Example
//!
//! ```ignore
//! use neobank::reputation::{ReputationService, ReputationScore};
//!
//! // Create reputation service
//! let service = ReputationService::new();
//!
//! // Fetch reputation for a counterparty
//! let score = service.fetch_reputation("counterparty_pubkey").await?;
//!
//! // Check if we should pay first based on reputation
//! let my_rep = service.fetch_reputation("my_pubkey").await?;
//! if service.should_pay_first(&my_rep, &score) {
//!     // We have lower reputation, pay first
//! }
//! ```

use crate::error::{Error, Result};
use crate::exchange::TradeAttestation;
use crate::relay::ExchangeRelay;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Reputation score for a trading counterparty
#[derive(Debug, Clone)]
pub struct ReputationScore {
    /// Public key (hex) of the counterparty
    pub pubkey: String,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Total number of completed trades
    pub total_trades: u64,
    /// Total trading volume in satoshis
    pub total_volume_sats: u64,
    /// Average settlement time in milliseconds
    pub avg_settlement_ms: u64,
    /// Dispute rate (0.0 to 1.0)
    pub dispute_rate: f64,
    /// Timestamp of last trade (Unix timestamp)
    pub last_trade: u64,
    /// Web of Trust score (if calculated)
    pub wot_score: Option<f64>,
}

impl Default for ReputationScore {
    fn default() -> Self {
        Self {
            pubkey: String::new(),
            success_rate: 0.0,
            total_trades: 0,
            total_volume_sats: 0,
            avg_settlement_ms: 0,
            dispute_rate: 0.0,
            last_trade: 0,
            wot_score: None,
        }
    }
}

impl ReputationScore {
    /// Create a new empty reputation score for a pubkey
    pub fn new(pubkey: impl Into<String>) -> Self {
        Self {
            pubkey: pubkey.into(),
            ..Default::default()
        }
    }

    /// Calculate a composite trust score (0.0 to 1.0)
    ///
    /// Weights:
    /// - Success rate: 40%
    /// - Trade volume confidence: 20%
    /// - Settlement speed: 20%
    /// - Dispute rate penalty: 20%
    pub fn composite_score(&self) -> f64 {
        // Base score from success rate
        let success_component = self.success_rate * 0.4;

        // Volume confidence (more trades = higher confidence)
        // Caps at 100 trades for max confidence
        let volume_confidence = (self.total_trades as f64 / 100.0).min(1.0) * 0.2;

        // Settlement speed (faster = better)
        // Assume 60s (60000ms) is acceptable, 10s is excellent
        let speed_score = if self.avg_settlement_ms == 0 {
            0.0
        } else {
            let speed_ratio = 10000.0 / (self.avg_settlement_ms as f64).max(10000.0);
            speed_ratio.min(1.0) * 0.2
        };

        // Dispute penalty
        let dispute_penalty = self.dispute_rate * 0.2;

        let base_score = success_component + volume_confidence + speed_score - dispute_penalty;

        // Apply WoT boost if available
        if let Some(wot) = self.wot_score {
            // WoT can boost score by up to 20%
            (base_score + wot * 0.2).min(1.0)
        } else {
            base_score.max(0.0)
        }
    }

    /// Check if this score represents a trusted counterparty
    pub fn is_trusted(&self) -> bool {
        self.composite_score() >= 0.5 && self.total_trades >= 3
    }

    /// Check if this is a new counterparty with no history
    pub fn is_new(&self) -> bool {
        self.total_trades == 0
    }
}

/// Configuration for reputation scoring
#[derive(Debug, Clone)]
pub struct ReputationConfig {
    /// Minimum trades required for trust
    pub min_trades_for_trust: u64,
    /// Minimum success rate for trust (0.0 to 1.0)
    pub min_success_rate: f64,
    /// Maximum acceptable dispute rate
    pub max_dispute_rate: f64,
    /// Cache TTL in seconds
    pub cache_ttl_secs: u64,
    /// Weight for WoT scoring
    pub wot_weight: f64,
}

impl Default for ReputationConfig {
    fn default() -> Self {
        Self {
            min_trades_for_trust: 3,
            min_success_rate: 0.8,
            max_dispute_rate: 0.1,
            cache_ttl_secs: 300, // 5 minutes
            wot_weight: 0.2,
        }
    }
}

/// Cache entry with timestamp
#[derive(Debug, Clone)]
struct CacheEntry {
    score: ReputationScore,
    cached_at: u64,
}

/// Reputation service for fetching and calculating trust scores
pub struct ReputationService {
    /// Relay for fetching attestations (optional)
    relay: Option<Arc<ExchangeRelay>>,
    /// Local attestation cache
    attestations: Arc<RwLock<HashMap<String, Vec<TradeAttestation>>>>,
    /// Cached reputation scores
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// Configuration
    config: ReputationConfig,
}

impl ReputationService {
    /// Create a new reputation service without relay
    pub fn new() -> Self {
        Self {
            relay: None,
            attestations: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(RwLock::new(HashMap::new())),
            config: ReputationConfig::default(),
        }
    }

    /// Create a new reputation service with relay integration
    pub fn new_with_relay(relay: Arc<ExchangeRelay>) -> Self {
        Self {
            relay: Some(relay),
            attestations: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(RwLock::new(HashMap::new())),
            config: ReputationConfig::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(mut self, config: ReputationConfig) -> Self {
        self.config = config;
        self
    }

    /// Get the service configuration
    pub fn config(&self) -> &ReputationConfig {
        &self.config
    }

    // ============================================================
    // Fetching
    // ============================================================

    /// Fetch reputation score for a pubkey
    ///
    /// Checks cache first, then fetches attestations from relay if configured.
    pub async fn fetch_reputation(&self, pubkey: &str) -> Result<ReputationScore> {
        // Check cache first
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(pubkey) {
                if now - entry.cached_at < self.config.cache_ttl_secs {
                    return Ok(entry.score.clone());
                }
            }
        }

        // Fetch attestations
        let attestations = self.fetch_attestations(pubkey, 100).await?;

        // Calculate score
        let score = self.calculate_score(pubkey, &attestations);

        // Cache the result
        {
            let mut cache = self.cache.write().await;
            cache.insert(
                pubkey.to_string(),
                CacheEntry {
                    score: score.clone(),
                    cached_at: now,
                },
            );
        }

        Ok(score)
    }

    /// Fetch attestations for a pubkey
    pub async fn fetch_attestations(
        &self,
        pubkey: &str,
        _limit: usize,
    ) -> Result<Vec<TradeAttestation>> {
        // Check local cache
        let local = self.attestations.read().await;
        if let Some(attestations) = local.get(pubkey) {
            return Ok(attestations.clone());
        }
        drop(local);

        // TODO: If relay is configured, fetch from relay
        // For now, return empty
        Ok(Vec::new())
    }

    /// Add an attestation to the local cache
    pub async fn add_attestation(&self, attestation: TradeAttestation) {
        let mut cache = self.attestations.write().await;
        cache
            .entry(attestation.counterparty.clone())
            .or_default()
            .push(attestation);

        // Invalidate reputation cache for this pubkey
        let mut rep_cache = self.cache.write().await;
        rep_cache.remove(&cache.keys().last().unwrap().clone());
    }

    // ============================================================
    // Calculation
    // ============================================================

    /// Calculate reputation score from attestations
    pub fn calculate_score(&self, pubkey: &str, attestations: &[TradeAttestation]) -> ReputationScore {
        if attestations.is_empty() {
            return ReputationScore::new(pubkey);
        }

        let total_trades = attestations.len() as u64;

        // Count outcomes
        let mut success_count = 0u64;
        let mut dispute_count = 0u64;
        let mut total_volume = 0u64;
        let mut total_settlement_ms = 0u64;
        let mut last_trade = 0u64;

        for att in attestations {
            total_volume += att.amount_sats;
            total_settlement_ms += att.settlement_ms;

            use crate::exchange::TradeOutcome;
            match att.outcome {
                TradeOutcome::Success => success_count += 1,
                TradeOutcome::Slow => success_count += 1, // Still counts as success
                TradeOutcome::Default => {}
                TradeOutcome::Dispute => dispute_count += 1,
            }

            // Track last trade (using event_id hash as proxy for time)
            // In real impl, would use created_at from event
            last_trade = last_trade.max(att.amount_sats); // Placeholder
        }

        let success_rate = success_count as f64 / total_trades as f64;
        let dispute_rate = dispute_count as f64 / total_trades as f64;
        let avg_settlement_ms = total_settlement_ms / total_trades;

        ReputationScore {
            pubkey: pubkey.to_string(),
            success_rate,
            total_trades,
            total_volume_sats: total_volume,
            avg_settlement_ms,
            dispute_rate,
            last_trade,
            wot_score: None,
        }
    }

    /// Calculate Web of Trust score
    ///
    /// Checks how many of our follows have traded successfully with this counterparty.
    pub fn calculate_wot_score(
        &self,
        pubkey: &str,
        attestations: &[TradeAttestation],
        follows: &[String],
    ) -> f64 {
        if follows.is_empty() {
            return 0.0;
        }

        // Count attestations from people we follow
        let mut trusted_attestations = 0;
        let mut trusted_successes = 0;

        for att in attestations {
            if follows.contains(&att.counterparty) {
                trusted_attestations += 1;
                if att.outcome == crate::exchange::TradeOutcome::Success {
                    trusted_successes += 1;
                }
            }
        }

        if trusted_attestations == 0 {
            return 0.0;
        }

        // Return trust score weighted by attestation count
        let base_score = trusted_successes as f64 / trusted_attestations as f64;

        // Apply confidence factor (more attestations = higher confidence)
        let confidence = (trusted_attestations as f64 / 10.0).min(1.0);

        base_score * confidence
    }

    // ============================================================
    // Decision Helpers
    // ============================================================

    /// Determine which party should pay first in settlement
    ///
    /// The party with lower reputation pays first to reduce risk.
    /// Returns true if WE should pay first.
    pub fn should_pay_first(&self, my_rep: &ReputationScore, their_rep: &ReputationScore) -> bool {
        let my_score = my_rep.composite_score();
        let their_score = their_rep.composite_score();

        // If scores are very close (within 5%), use volume as tiebreaker
        if (my_score - their_score).abs() < 0.05 {
            return my_rep.total_volume_sats < their_rep.total_volume_sats;
        }

        // Lower score pays first
        my_score < their_score
    }

    /// Get minimum reputation required for a trade amount
    ///
    /// Higher amounts require higher reputation.
    pub fn min_reputation_for_amount(&self, amount_sats: u64) -> f64 {
        // Tiered requirements:
        // < 10,000 sats: no minimum
        // 10,000 - 100,000: 0.3 minimum
        // 100,000 - 1,000,000: 0.5 minimum
        // > 1,000,000: 0.7 minimum
        match amount_sats {
            0..=9_999 => 0.0,
            10_000..=99_999 => 0.3,
            100_000..=999_999 => 0.5,
            _ => 0.7,
        }
    }

    /// Check if a counterparty meets reputation requirements for an amount
    pub fn meets_requirements(&self, rep: &ReputationScore, amount_sats: u64) -> bool {
        let min_rep = self.min_reputation_for_amount(amount_sats);
        rep.composite_score() >= min_rep
    }

    /// Get a human-readable trust level
    pub fn trust_level(&self, rep: &ReputationScore) -> TrustLevel {
        let score = rep.composite_score();

        if rep.is_new() {
            TrustLevel::Unknown
        } else if score >= 0.8 && rep.total_trades >= 20 {
            TrustLevel::Excellent
        } else if score >= 0.6 && rep.total_trades >= 10 {
            TrustLevel::Good
        } else if score >= 0.4 && rep.total_trades >= 5 {
            TrustLevel::Moderate
        } else if score >= 0.2 {
            TrustLevel::Low
        } else {
            TrustLevel::Poor
        }
    }

    /// Clear the reputation cache
    pub async fn clear_cache(&self) {
        self.cache.write().await.clear();
    }

    /// Clear all attestations
    pub async fn clear_attestations(&self) {
        self.attestations.write().await.clear();
    }
}

impl Default for ReputationService {
    fn default() -> Self {
        Self::new()
    }
}

/// Human-readable trust level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustLevel {
    /// No trading history
    Unknown,
    /// Score >= 0.8, 20+ trades
    Excellent,
    /// Score >= 0.6, 10+ trades
    Good,
    /// Score >= 0.4, 5+ trades
    Moderate,
    /// Score >= 0.2
    Low,
    /// Score < 0.2 or high dispute rate
    Poor,
}

impl TrustLevel {
    pub fn as_str(&self) -> &str {
        match self {
            TrustLevel::Unknown => "unknown",
            TrustLevel::Excellent => "excellent",
            TrustLevel::Good => "good",
            TrustLevel::Moderate => "moderate",
            TrustLevel::Low => "low",
            TrustLevel::Poor => "poor",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exchange::TradeOutcome;

    fn make_attestation(outcome: TradeOutcome, amount: u64, ms: u64) -> TradeAttestation {
        TradeAttestation {
            event_id: format!("event-{}", amount),
            trade_id: format!("trade-{}", amount),
            counterparty: "counterparty".to_string(),
            outcome,
            settlement_ms: ms,
            amount_sats: amount,
        }
    }

    #[test]
    fn test_reputation_score_default() {
        let score = ReputationScore::default();
        assert_eq!(score.success_rate, 0.0);
        assert_eq!(score.total_trades, 0);
        assert!(score.is_new());
    }

    #[test]
    fn test_reputation_score_new() {
        let score = ReputationScore::new("test_pubkey");
        assert_eq!(score.pubkey, "test_pubkey");
        assert!(score.is_new());
    }

    #[test]
    fn test_composite_score_calculation() {
        let mut score = ReputationScore::new("test");
        score.success_rate = 1.0;
        score.total_trades = 100;
        score.avg_settlement_ms = 10000;
        score.dispute_rate = 0.0;

        // Perfect score should be close to 0.8 (40% success + 20% volume + 20% speed)
        let composite = score.composite_score();
        assert!(composite >= 0.75);
        assert!(composite <= 0.85);
    }

    #[test]
    fn test_is_trusted() {
        let mut score = ReputationScore::new("test");

        // New user not trusted
        assert!(!score.is_trusted());

        // Good score but few trades
        score.success_rate = 1.0;
        score.total_trades = 2;
        score.avg_settlement_ms = 10000; // Fast settlement
        assert!(!score.is_trusted()); // Not enough trades

        // Good score with enough trades - composite should exceed 0.5
        score.total_trades = 50; // More trades for higher volume confidence
        // composite = 0.4 (success) + 0.1 (volume 50/100*0.2) + 0.2 (speed) = 0.7
        assert!(score.is_trusted());
    }

    #[test]
    fn test_calculate_score() {
        let service = ReputationService::new();

        let attestations = vec![
            make_attestation(TradeOutcome::Success, 10_000, 5000),
            make_attestation(TradeOutcome::Success, 20_000, 10000),
            make_attestation(TradeOutcome::Slow, 15_000, 30000),
            make_attestation(TradeOutcome::Default, 5_000, 60000),
        ];

        let score = service.calculate_score("test_pubkey", &attestations);

        assert_eq!(score.total_trades, 4);
        assert_eq!(score.total_volume_sats, 50_000);
        assert_eq!(score.success_rate, 0.75); // 3 success (including Slow) out of 4
        assert_eq!(score.dispute_rate, 0.0);
        assert_eq!(score.avg_settlement_ms, 26250); // (5000+10000+30000+60000)/4
    }

    #[test]
    fn test_calculate_score_with_disputes() {
        let service = ReputationService::new();

        let attestations = vec![
            make_attestation(TradeOutcome::Success, 10_000, 5000),
            make_attestation(TradeOutcome::Dispute, 20_000, 10000),
        ];

        let score = service.calculate_score("test_pubkey", &attestations);

        assert_eq!(score.success_rate, 0.5);
        assert_eq!(score.dispute_rate, 0.5);
    }

    #[test]
    fn test_should_pay_first() {
        let service = ReputationService::new();

        let mut low_rep = ReputationScore::new("low");
        low_rep.success_rate = 0.5;
        low_rep.total_trades = 5;

        let mut high_rep = ReputationScore::new("high");
        high_rep.success_rate = 0.95;
        high_rep.total_trades = 50;

        // Low reputation should pay first
        assert!(service.should_pay_first(&low_rep, &high_rep));
        assert!(!service.should_pay_first(&high_rep, &low_rep));
    }

    #[test]
    fn test_min_reputation_for_amount() {
        let service = ReputationService::new();

        assert_eq!(service.min_reputation_for_amount(1_000), 0.0);
        assert_eq!(service.min_reputation_for_amount(50_000), 0.3);
        assert_eq!(service.min_reputation_for_amount(500_000), 0.5);
        assert_eq!(service.min_reputation_for_amount(2_000_000), 0.7);
    }

    #[test]
    fn test_trust_level() {
        let service = ReputationService::new();

        // Unknown
        let new_score = ReputationScore::new("new");
        assert_eq!(service.trust_level(&new_score), TrustLevel::Unknown);

        // Excellent - need composite >= 0.8 and 20+ trades
        // composite = success(0.4) + volume(0.2) + speed(0.2) - disputes(0)
        // Need: 1.0*0.4 + (100/100)*0.2 + 1.0*0.2 = 0.8
        let mut excellent = ReputationScore::new("excellent");
        excellent.success_rate = 1.0; // 100% success
        excellent.total_trades = 100; // Max volume confidence
        excellent.avg_settlement_ms = 10000; // Fast settlement
        excellent.dispute_rate = 0.0;
        // Verify the score is actually >= 0.8
        assert!(excellent.composite_score() >= 0.8, "Expected >= 0.8, got {}", excellent.composite_score());
        assert_eq!(service.trust_level(&excellent), TrustLevel::Excellent);

        // Good - need composite >= 0.6 and 10+ trades
        let mut good = ReputationScore::new("good");
        good.success_rate = 0.90; // Higher success rate
        good.total_trades = 50;
        good.avg_settlement_ms = 10000; // Faster settlement
        good.dispute_rate = 0.0;
        // composite = 0.9*0.4 + 0.5*0.2 + 1.0*0.2 = 0.36 + 0.1 + 0.2 = 0.66
        assert!(good.composite_score() >= 0.6, "Expected >= 0.6, got {}", good.composite_score());
        assert_eq!(service.trust_level(&good), TrustLevel::Good);
    }

    #[test]
    fn test_wot_score() {
        let service = ReputationService::new();

        let follows = vec!["alice".to_string(), "bob".to_string()];

        let attestations = vec![
            TradeAttestation {
                event_id: "1".to_string(),
                trade_id: "t1".to_string(),
                counterparty: "alice".to_string(),
                outcome: TradeOutcome::Success,
                settlement_ms: 5000,
                amount_sats: 10_000,
            },
            TradeAttestation {
                event_id: "2".to_string(),
                trade_id: "t2".to_string(),
                counterparty: "bob".to_string(),
                outcome: TradeOutcome::Success,
                settlement_ms: 5000,
                amount_sats: 20_000,
            },
            TradeAttestation {
                event_id: "3".to_string(),
                trade_id: "t3".to_string(),
                counterparty: "charlie".to_string(), // Not in follows
                outcome: TradeOutcome::Success,
                settlement_ms: 5000,
                amount_sats: 30_000,
            },
        ];

        let wot_score = service.calculate_wot_score("target", &attestations, &follows);

        // 2 attestations from follows, both success -> 1.0 base score
        // Confidence factor: 2/10 = 0.2
        // Final: 1.0 * 0.2 = 0.2
        assert_eq!(wot_score, 0.2);
    }

    #[test]
    fn test_wot_score_no_follows() {
        let service = ReputationService::new();

        let attestations = vec![make_attestation(TradeOutcome::Success, 10_000, 5000)];

        let wot_score = service.calculate_wot_score("target", &attestations, &[]);
        assert_eq!(wot_score, 0.0);
    }

    #[tokio::test]
    async fn test_add_attestation() {
        let service = ReputationService::new();

        let att = TradeAttestation {
            event_id: "1".to_string(),
            trade_id: "t1".to_string(),
            counterparty: "alice".to_string(),
            outcome: TradeOutcome::Success,
            settlement_ms: 5000,
            amount_sats: 10_000,
        };

        service.add_attestation(att.clone()).await;

        let fetched = service.fetch_attestations("alice", 100).await.unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].trade_id, "t1");
    }

    #[tokio::test]
    async fn test_fetch_reputation_caching() {
        let service = ReputationService::new();

        // Add some attestations
        service
            .add_attestation(TradeAttestation {
                event_id: "1".to_string(),
                trade_id: "t1".to_string(),
                counterparty: "alice".to_string(),
                outcome: TradeOutcome::Success,
                settlement_ms: 5000,
                amount_sats: 10_000,
            })
            .await;

        // Fetch reputation (should calculate and cache)
        let rep1 = service.fetch_reputation("alice").await.unwrap();
        assert_eq!(rep1.total_trades, 1);

        // Second fetch should use cache
        let rep2 = service.fetch_reputation("alice").await.unwrap();
        assert_eq!(rep2.total_trades, 1);
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let service = ReputationService::new();

        service
            .add_attestation(TradeAttestation {
                event_id: "1".to_string(),
                trade_id: "t1".to_string(),
                counterparty: "alice".to_string(),
                outcome: TradeOutcome::Success,
                settlement_ms: 5000,
                amount_sats: 10_000,
            })
            .await;

        // Populate cache
        service.fetch_reputation("alice").await.unwrap();

        // Clear
        service.clear_cache().await;
        service.clear_attestations().await;

        // Should return empty score now
        let rep = service.fetch_reputation("alice").await.unwrap();
        assert_eq!(rep.total_trades, 0);
    }
}
