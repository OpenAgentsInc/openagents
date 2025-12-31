//! Mint Trust - Cashu mint discovery and trust scoring (NIP-87)
//!
//! This module provides mint discovery, health monitoring, and trust scoring
//! based on Nostr recommendations and operational metrics.
//!
//! # Example
//!
//! ```ignore
//! use neobank::mint_trust::{MintTrustService, MintNetwork};
//! use neobank::types::Currency;
//!
//! // Create mint trust service
//! let service = MintTrustService::new();
//!
//! // Add known mints
//! service.add_mint("https://mint.example.com", Currency::Btc, MintNetwork::Mainnet).await?;
//!
//! // Select best mint for a currency
//! let mint_url = service.select_mint(Currency::Btc, 0.5).await?;
//!
//! // Check mint health
//! let health = service.check_mint_health(&mint_url).await?;
//! if health.is_healthy() {
//!     println!("Mint is operational");
//! }
//! ```

use crate::error::{Error, Result};
use crate::relay::ExchangeRelay;
use crate::types::Currency;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use url::Url;

/// Event kind for mint recommendations (NIP-87)
pub const MINT_RECOMMENDATION_KIND: u16 = 38172;

/// Bitcoin network a mint operates on
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MintNetwork {
    /// Bitcoin mainnet
    Mainnet,
    /// Bitcoin testnet
    Testnet,
    /// Bitcoin signet
    Signet,
    /// Mutinynet
    Mutinynet,
}

impl MintNetwork {
    pub fn as_str(&self) -> &str {
        match self {
            MintNetwork::Mainnet => "mainnet",
            MintNetwork::Testnet => "testnet",
            MintNetwork::Signet => "signet",
            MintNetwork::Mutinynet => "mutinynet",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "mainnet" | "main" => Some(MintNetwork::Mainnet),
            "testnet" | "test" => Some(MintNetwork::Testnet),
            "signet" => Some(MintNetwork::Signet),
            "mutinynet" => Some(MintNetwork::Mutinynet),
            _ => None,
        }
    }
}

/// Information about a Cashu mint
#[derive(Debug, Clone)]
pub struct MintInfo {
    /// Mint URL
    pub url: Url,
    /// Mint public key (if known)
    pub pubkey: Option<String>,
    /// Supported currency
    pub currency: Currency,
    /// Network the mint operates on
    pub network: MintNetwork,
    /// Supported NUTs (Cashu protocol extensions)
    pub supported_nuts: Vec<u8>,
    /// Trust score (0.0 to 1.0)
    pub trust_score: f64,
    /// Number of recommendations
    pub recommendations: u64,
    /// Last seen timestamp
    pub last_seen: u64,
    /// Average response time in ms
    pub avg_response_ms: u64,
    /// Uptime percentage (0.0 to 1.0)
    pub uptime: f64,
}

impl MintInfo {
    /// Create a new MintInfo
    pub fn new(url: Url, currency: Currency, network: MintNetwork) -> Self {
        Self {
            url,
            pubkey: None,
            currency,
            network,
            supported_nuts: Vec::new(),
            trust_score: 0.0,
            recommendations: 0,
            last_seen: 0,
            avg_response_ms: 0,
            uptime: 1.0,
        }
    }

    /// Update last seen timestamp
    pub fn mark_seen(&mut self) {
        self.last_seen = now_secs();
    }

    /// Check if mint was seen recently
    pub fn is_active(&self, max_age_secs: u64) -> bool {
        let now = now_secs();
        now - self.last_seen < max_age_secs
    }
}

/// Mint recommendation from a Nostr user
#[derive(Debug, Clone)]
pub struct MintRecommendation {
    /// Recommender's public key
    pub pubkey: String,
    /// Mint URL being recommended
    pub mint_url: Url,
    /// Recommendation score (1-5 stars or similar)
    pub rating: u8,
    /// Optional comment
    pub comment: Option<String>,
    /// Timestamp of recommendation
    pub created_at: u64,
}

/// Mint health status
#[derive(Debug, Clone)]
pub struct MintHealth {
    /// Mint URL
    pub url: Url,
    /// Whether mint is reachable
    pub reachable: bool,
    /// Response time in ms
    pub response_ms: u64,
    /// Last check timestamp
    pub checked_at: u64,
    /// Error message if unhealthy
    pub error: Option<String>,
}

impl MintHealth {
    /// Check if mint is healthy
    pub fn is_healthy(&self) -> bool {
        self.reachable && self.response_ms < 5000 // 5s timeout
    }

    /// Create a healthy status
    pub fn healthy(url: Url, response_ms: u64) -> Self {
        Self {
            url,
            reachable: true,
            response_ms,
            checked_at: now_secs(),
            error: None,
        }
    }

    /// Create an unhealthy status
    pub fn unhealthy(url: Url, error: impl Into<String>) -> Self {
        Self {
            url,
            reachable: false,
            response_ms: 0,
            checked_at: now_secs(),
            error: Some(error.into()),
        }
    }
}

/// Mint trust service for discovery and scoring
pub struct MintTrustService {
    /// Relay for fetching recommendations (optional)
    relay: Option<Arc<ExchangeRelay>>,
    /// Known mints
    mints: Arc<RwLock<HashMap<String, MintInfo>>>,
    /// Recommendations per mint
    recommendations: Arc<RwLock<HashMap<String, Vec<MintRecommendation>>>>,
    /// Allowlist (mints we explicitly trust)
    allowlist: Arc<RwLock<HashSet<String>>>,
    /// Blocklist (mints we explicitly distrust)
    blocklist: Arc<RwLock<HashSet<String>>>,
    /// Health cache
    health_cache: Arc<RwLock<HashMap<String, MintHealth>>>,
    /// Configuration
    config: MintTrustConfig,
}

/// Configuration for mint trust service
#[derive(Debug, Clone)]
pub struct MintTrustConfig {
    /// Maximum age for health cache (seconds)
    pub health_cache_ttl_secs: u64,
    /// Minimum trust score to use a mint
    pub min_trust_score: f64,
    /// Weight for recommendation count in scoring
    pub recommendation_weight: f64,
    /// Weight for uptime in scoring
    pub uptime_weight: f64,
    /// Weight for response time in scoring
    pub response_weight: f64,
}

impl Default for MintTrustConfig {
    fn default() -> Self {
        Self {
            health_cache_ttl_secs: 300, // 5 minutes
            min_trust_score: 0.3,
            recommendation_weight: 0.4,
            uptime_weight: 0.3,
            response_weight: 0.3,
        }
    }
}

impl MintTrustService {
    /// Create a new mint trust service
    pub fn new() -> Self {
        Self {
            relay: None,
            mints: Arc::new(RwLock::new(HashMap::new())),
            recommendations: Arc::new(RwLock::new(HashMap::new())),
            allowlist: Arc::new(RwLock::new(HashSet::new())),
            blocklist: Arc::new(RwLock::new(HashSet::new())),
            health_cache: Arc::new(RwLock::new(HashMap::new())),
            config: MintTrustConfig::default(),
        }
    }

    /// Create with relay integration
    pub fn with_relay(mut self, relay: Arc<ExchangeRelay>) -> Self {
        self.relay = Some(relay);
        self
    }

    /// Create with custom config
    pub fn with_config(mut self, config: MintTrustConfig) -> Self {
        self.config = config;
        self
    }

    /// Get the configuration
    pub fn config(&self) -> &MintTrustConfig {
        &self.config
    }

    // ============================================================
    // Mint Management
    // ============================================================

    /// Add a mint to the known list
    pub async fn add_mint(
        &self,
        url: &str,
        currency: Currency,
        network: MintNetwork,
    ) -> Result<()> {
        let url = Url::parse(url).map_err(|e| Error::Database(e.to_string()))?;
        let key = url.to_string();

        let mut mints = self.mints.write().await;
        mints.insert(key, MintInfo::new(url, currency, network));

        Ok(())
    }

    /// Get mint info
    pub async fn get_mint(&self, url: &str) -> Option<MintInfo> {
        self.mints.read().await.get(url).cloned()
    }

    /// Get all mints for a currency
    pub async fn get_mints_for_currency(&self, currency: Currency) -> Vec<MintInfo> {
        self.mints
            .read()
            .await
            .values()
            .filter(|m| m.currency == currency)
            .cloned()
            .collect()
    }

    /// Get all mints
    pub async fn list_mints(&self) -> Vec<MintInfo> {
        self.mints.read().await.values().cloned().collect()
    }

    // ============================================================
    // Recommendations
    // ============================================================

    /// Add a recommendation
    pub async fn add_recommendation(&self, rec: MintRecommendation) {
        let key = rec.mint_url.to_string();

        // Update recommendation list
        {
            let mut recs = self.recommendations.write().await;
            recs.entry(key.clone()).or_default().push(rec);
        }

        // Update mint info
        {
            let mut mints = self.mints.write().await;
            if let Some(mint) = mints.get_mut(&key) {
                mint.recommendations += 1;
                mint.trust_score = self.calculate_trust_score(mint, &[]).await;
            }
        }
    }

    /// Get recommendations for a mint
    pub async fn get_recommendations(&self, mint_url: &str) -> Vec<MintRecommendation> {
        self.recommendations
            .read()
            .await
            .get(mint_url)
            .cloned()
            .unwrap_or_default()
    }

    /// Fetch recommendations from people we follow (Web of Trust)
    pub async fn fetch_wot_recommendations(
        &self,
        follows: &[String],
    ) -> Result<Vec<MintRecommendation>> {
        // Filter recommendations to only those from follows
        let all_recs = self.recommendations.read().await;

        let wot_recs: Vec<MintRecommendation> = all_recs
            .values()
            .flatten()
            .filter(|r| follows.contains(&r.pubkey))
            .cloned()
            .collect();

        Ok(wot_recs)
    }

    // ============================================================
    // Health Monitoring
    // ============================================================

    /// Check mint health (with caching)
    pub async fn check_mint_health(&self, url: &Url) -> Result<MintHealth> {
        let key = url.to_string();

        // Check cache first
        {
            let cache = self.health_cache.read().await;
            if let Some(health) = cache.get(&key) {
                let age = now_secs() - health.checked_at;
                if age < self.config.health_cache_ttl_secs {
                    return Ok(health.clone());
                }
            }
        }

        // Perform health check
        let health = self.probe_mint(url).await;

        // Update cache
        {
            let mut cache = self.health_cache.write().await;
            cache.insert(key.clone(), health.clone());
        }

        // Update mint info
        {
            let mut mints = self.mints.write().await;
            if let Some(mint) = mints.get_mut(&key) {
                mint.mark_seen();
                if health.is_healthy() {
                    mint.avg_response_ms = (mint.avg_response_ms + health.response_ms) / 2;
                }
            }
        }

        Ok(health)
    }

    /// Probe a mint (actual health check)
    async fn probe_mint(&self, url: &Url) -> MintHealth {
        // In a real implementation, this would make HTTP requests to the mint
        // For now, return a mock healthy response
        MintHealth::healthy(url.clone(), 100)
    }

    /// Check which NUTs a mint supports
    pub async fn probe_nut_support(&self, url: &Url) -> Result<Vec<u8>> {
        let key = url.to_string();

        // In a real implementation, would query mint's /v1/info endpoint
        // For now, return basic NUTs
        let nuts = vec![1, 2, 3, 4, 5, 6, 7, 8]; // Basic Cashu NUTs

        // Update mint info
        {
            let mut mints = self.mints.write().await;
            if let Some(mint) = mints.get_mut(&key) {
                mint.supported_nuts = nuts.clone();
            }
        }

        Ok(nuts)
    }

    // ============================================================
    // Trust Scoring
    // ============================================================

    /// Calculate trust score for a mint
    async fn calculate_trust_score(&self, mint: &MintInfo, _wot_follows: &[String]) -> f64 {
        let mut score = 0.0;

        // Recommendation component
        let rec_score = (mint.recommendations as f64 / 10.0).min(1.0);
        score += rec_score * self.config.recommendation_weight;

        // Uptime component
        score += mint.uptime * self.config.uptime_weight;

        // Response time component (faster = better)
        let response_score = if mint.avg_response_ms == 0 {
            0.5 // Unknown
        } else {
            (1000.0 / mint.avg_response_ms as f64).min(1.0)
        };
        score += response_score * self.config.response_weight;

        // Allowlist boost
        if self.allowlist.read().await.contains(&mint.url.to_string()) {
            score = (score + 0.2).min(1.0);
        }

        score.min(1.0)
    }

    /// Recalculate trust score for a mint
    pub async fn update_trust_score(&self, mint_url: &str) -> Result<f64> {
        let mut mints = self.mints.write().await;
        let mint = mints
            .get_mut(mint_url)
            .ok_or_else(|| Error::Database("Mint not found".to_string()))?;

        let score = self.calculate_trust_score(mint, &[]).await;
        mint.trust_score = score;

        Ok(score)
    }

    // ============================================================
    // Mint Selection
    // ============================================================

    /// Select best mint for a currency
    pub async fn select_mint(&self, currency: Currency, min_trust: f64) -> Result<Url> {
        let mints = self.mints.read().await;
        let blocklist = self.blocklist.read().await;

        let best = mints
            .values()
            .filter(|m| m.currency == currency)
            .filter(|m| m.trust_score >= min_trust)
            .filter(|m| !blocklist.contains(&m.url.to_string()))
            .max_by(|a, b| a.trust_score.partial_cmp(&b.trust_score).unwrap());

        best.map(|m| m.url.clone())
            .ok_or_else(|| Error::Database("No suitable mint found".to_string()))
    }

    /// Get mints ranked by trust score
    pub async fn ranked_mints(&self, currency: Currency) -> Vec<MintInfo> {
        let blocklist = self.blocklist.read().await;
        let mut mints: Vec<MintInfo> = self
            .mints
            .read()
            .await
            .values()
            .filter(|m| m.currency == currency)
            .filter(|m| !blocklist.contains(&m.url.to_string()))
            .cloned()
            .collect();

        mints.sort_by(|a, b| b.trust_score.partial_cmp(&a.trust_score).unwrap());
        mints
    }

    // ============================================================
    // Allowlist / Blocklist
    // ============================================================

    /// Check if a mint is allowed
    pub async fn is_allowed(&self, url: &Url) -> bool {
        let key = url.to_string();
        let blocklist = self.blocklist.read().await;

        if blocklist.contains(&key) {
            return false;
        }

        // If we have an allowlist and it's not empty, mint must be in it
        let allowlist = self.allowlist.read().await;
        if !allowlist.is_empty() {
            return allowlist.contains(&key);
        }

        true // Default allow if no allowlist
    }

    /// Add to allowlist
    pub async fn add_to_allowlist(&self, url: &Url) {
        self.allowlist.write().await.insert(url.to_string());
        self.blocklist.write().await.remove(&url.to_string());
    }

    /// Add to blocklist
    pub async fn add_to_blocklist(&self, url: &Url) {
        self.blocklist.write().await.insert(url.to_string());
        self.allowlist.write().await.remove(&url.to_string());
    }

    /// Remove from allowlist
    pub async fn remove_from_allowlist(&self, url: &Url) {
        self.allowlist.write().await.remove(&url.to_string());
    }

    /// Remove from blocklist
    pub async fn remove_from_blocklist(&self, url: &Url) {
        self.blocklist.write().await.remove(&url.to_string());
    }

    /// Clear caches
    pub async fn clear_caches(&self) {
        self.health_cache.write().await.clear();
    }

    // ============================================================
    // Tag Builders
    // ============================================================

    /// Build NIP-87 mint recommendation tags
    pub fn build_recommendation_tags(
        &self,
        mint_url: &Url,
        rating: u8,
        comment: Option<&str>,
    ) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["u".to_string(), mint_url.to_string(), "cashu".to_string()],
            vec!["rating".to_string(), rating.to_string()],
        ];

        if let Some(c) = comment {
            tags.push(vec!["comment".to_string(), c.to_string()]);
        }

        tags
    }
}

impl Default for MintTrustService {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current Unix timestamp
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mint_network() {
        assert_eq!(MintNetwork::Mainnet.as_str(), "mainnet");
        assert_eq!(MintNetwork::from_str("mainnet"), Some(MintNetwork::Mainnet));
        assert_eq!(MintNetwork::from_str("TESTNET"), Some(MintNetwork::Testnet));
        assert_eq!(MintNetwork::from_str("unknown"), None);
    }

    #[test]
    fn test_mint_info_creation() {
        let url = Url::parse("https://mint.example.com").unwrap();
        let info = MintInfo::new(url.clone(), Currency::Btc, MintNetwork::Mainnet);

        assert_eq!(info.url, url);
        assert_eq!(info.currency, Currency::Btc);
        assert_eq!(info.network, MintNetwork::Mainnet);
        assert_eq!(info.trust_score, 0.0);
    }

    #[test]
    fn test_mint_info_mark_seen() {
        let url = Url::parse("https://mint.example.com").unwrap();
        let mut info = MintInfo::new(url, Currency::Btc, MintNetwork::Mainnet);

        assert_eq!(info.last_seen, 0);
        info.mark_seen();
        assert!(info.last_seen > 0);
        assert!(info.is_active(60)); // Active in last minute
    }

    #[test]
    fn test_mint_health() {
        let url = Url::parse("https://mint.example.com").unwrap();

        let healthy = MintHealth::healthy(url.clone(), 100);
        assert!(healthy.is_healthy());
        assert!(healthy.reachable);
        assert_eq!(healthy.response_ms, 100);

        let unhealthy = MintHealth::unhealthy(url, "Connection timeout");
        assert!(!unhealthy.is_healthy());
        assert!(!unhealthy.reachable);
        assert_eq!(unhealthy.error, Some("Connection timeout".to_string()));
    }

    #[tokio::test]
    async fn test_add_and_get_mint() {
        let service = MintTrustService::new();

        service
            .add_mint(
                "https://mint.example.com",
                Currency::Btc,
                MintNetwork::Mainnet,
            )
            .await
            .unwrap();

        let mint = service.get_mint("https://mint.example.com/").await.unwrap();

        assert_eq!(mint.currency, Currency::Btc);
        assert_eq!(mint.network, MintNetwork::Mainnet);
    }

    #[tokio::test]
    async fn test_get_mints_for_currency() {
        let service = MintTrustService::new();

        service
            .add_mint(
                "https://btc1.example.com",
                Currency::Btc,
                MintNetwork::Mainnet,
            )
            .await
            .unwrap();
        service
            .add_mint(
                "https://btc2.example.com",
                Currency::Btc,
                MintNetwork::Testnet,
            )
            .await
            .unwrap();
        service
            .add_mint(
                "https://usd.example.com",
                Currency::Usd,
                MintNetwork::Mainnet,
            )
            .await
            .unwrap();

        let btc_mints = service.get_mints_for_currency(Currency::Btc).await;
        assert_eq!(btc_mints.len(), 2);

        let usd_mints = service.get_mints_for_currency(Currency::Usd).await;
        assert_eq!(usd_mints.len(), 1);
    }

    #[tokio::test]
    async fn test_add_recommendation() {
        let service = MintTrustService::new();
        let url = Url::parse("https://mint.example.com").unwrap();

        service
            .add_mint(
                "https://mint.example.com",
                Currency::Btc,
                MintNetwork::Mainnet,
            )
            .await
            .unwrap();

        let rec = MintRecommendation {
            pubkey: "user1".to_string(),
            mint_url: url.clone(),
            rating: 5,
            comment: Some("Great mint!".to_string()),
            created_at: now_secs(),
        };

        service.add_recommendation(rec).await;

        let recs = service
            .get_recommendations("https://mint.example.com/")
            .await;
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].rating, 5);

        // Check mint recommendation count updated
        let mint = service.get_mint("https://mint.example.com/").await.unwrap();
        assert_eq!(mint.recommendations, 1);
    }

    #[tokio::test]
    async fn test_allowlist_blocklist() {
        let service = MintTrustService::new();
        let url = Url::parse("https://mint.example.com").unwrap();

        // Default: allowed
        assert!(service.is_allowed(&url).await);

        // Add to blocklist
        service.add_to_blocklist(&url).await;
        assert!(!service.is_allowed(&url).await);

        // Move to allowlist
        service.add_to_allowlist(&url).await;
        assert!(service.is_allowed(&url).await);

        // Remove from allowlist (back to default)
        service.remove_from_allowlist(&url).await;
        assert!(service.is_allowed(&url).await);
    }

    #[tokio::test]
    async fn test_select_mint() {
        let service = MintTrustService::new();

        service
            .add_mint(
                "https://low.example.com",
                Currency::Btc,
                MintNetwork::Mainnet,
            )
            .await
            .unwrap();
        service
            .add_mint(
                "https://high.example.com",
                Currency::Btc,
                MintNetwork::Mainnet,
            )
            .await
            .unwrap();

        // Set different trust scores
        {
            let mut mints = service.mints.write().await;
            mints
                .get_mut("https://low.example.com/")
                .unwrap()
                .trust_score = 0.3;
            mints
                .get_mut("https://high.example.com/")
                .unwrap()
                .trust_score = 0.8;
        }

        // Select with low min trust
        let best = service.select_mint(Currency::Btc, 0.1).await.unwrap();
        assert_eq!(best.as_str(), "https://high.example.com/");

        // Select with high min trust - only high mint qualifies
        let best = service.select_mint(Currency::Btc, 0.5).await.unwrap();
        assert_eq!(best.as_str(), "https://high.example.com/");

        // Select with very high min trust - should fail
        let result = service.select_mint(Currency::Btc, 0.9).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ranked_mints() {
        let service = MintTrustService::new();

        service
            .add_mint("https://a.example.com", Currency::Btc, MintNetwork::Mainnet)
            .await
            .unwrap();
        service
            .add_mint("https://b.example.com", Currency::Btc, MintNetwork::Mainnet)
            .await
            .unwrap();
        service
            .add_mint("https://c.example.com", Currency::Btc, MintNetwork::Mainnet)
            .await
            .unwrap();

        // Set trust scores
        {
            let mut mints = service.mints.write().await;
            mints.get_mut("https://a.example.com/").unwrap().trust_score = 0.5;
            mints.get_mut("https://b.example.com/").unwrap().trust_score = 0.9;
            mints.get_mut("https://c.example.com/").unwrap().trust_score = 0.7;
        }

        let ranked = service.ranked_mints(Currency::Btc).await;
        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].url.as_str(), "https://b.example.com/");
        assert_eq!(ranked[1].url.as_str(), "https://c.example.com/");
        assert_eq!(ranked[2].url.as_str(), "https://a.example.com/");
    }

    #[test]
    fn test_build_recommendation_tags() {
        let service = MintTrustService::new();
        let url = Url::parse("https://mint.example.com").unwrap();

        let tags = service.build_recommendation_tags(&url, 5, Some("Great!"));

        assert!(
            tags.iter()
                .any(|t| t[0] == "u" && t[1] == "https://mint.example.com/")
        );
        assert!(tags.iter().any(|t| t[0] == "rating" && t[1] == "5"));
        assert!(tags.iter().any(|t| t[0] == "comment" && t[1] == "Great!"));
    }

    #[tokio::test]
    async fn test_probe_nut_support() {
        let service = MintTrustService::new();
        let url = Url::parse("https://mint.example.com").unwrap();

        service
            .add_mint(
                "https://mint.example.com",
                Currency::Btc,
                MintNetwork::Mainnet,
            )
            .await
            .unwrap();

        let nuts = service.probe_nut_support(&url).await.unwrap();
        assert!(!nuts.is_empty());

        // Check mint info updated
        let mint = service.get_mint("https://mint.example.com/").await.unwrap();
        assert!(!mint.supported_nuts.is_empty());
    }
}
