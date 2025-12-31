//! Provider discovery using NIP-89
//!
//! This module implements provider discovery for the marketplace using NIP-89
//! application handler events. Providers advertise their capabilities via
//! kind:31990 events, and users discover them through social recommendations
//! (kind:31989) or direct queries.
//!
//! # Examples
//!
//! ## Discovering compute providers
//!
//! ```
//! use marketplace::core::discovery::{ProviderDiscovery, ProviderQuery, SortBy};
//!
//! // Create discovery manager
//! let discovery = ProviderDiscovery::new();
//!
//! // Query for providers with specific model
//! let query = ProviderQuery::new()
//!     .with_model("llama3")
//!     .with_max_price(50) // max 50 sats per 1k tokens
//!     .with_min_trust_score(0.7)
//!     .sort_by(SortBy::TrustScore);
//!
//! let providers = discovery.query(&query);
//! for provider in providers {
//!     println!("{}: {} (trust: {})",
//!         provider.metadata.name,
//!         provider.pubkey,
//!         provider.trust_score
//!     );
//! }
//! ```
//!
//! ## Building provider query with filters
//!
//! ```
//! use marketplace::core::discovery::{ProviderQuery, SortBy};
//!
//! let query = ProviderQuery::new()
//!     .with_model("mistral")
//!     .with_region("us-west")
//!     .with_max_price(100)
//!     .sort_by(SortBy::Price);
//!
//! assert_eq!(query.model, Some("mistral".to_string()));
//! assert_eq!(query.region, Some("us-west".to_string()));
//! ```

use nostr::{HandlerInfo, HandlerMetadata, PricingInfo, SocialTrustScore};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Re-export NIP-89 types
pub use nostr::{
    HandlerType, KIND_HANDLER_INFO, KIND_HANDLER_RECOMMENDATION, Nip89Error, is_handler_info_kind,
    is_handler_recommendation_kind, is_nip89_kind,
};

/// Compute provider information discovered via NIP-89
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeProvider {
    /// Provider's pubkey
    pub pubkey: String,
    /// Provider metadata (name, description, etc.)
    pub metadata: ProviderMetadata,
    /// Supported capabilities (model names, job types, etc.)
    pub capabilities: Vec<String>,
    /// Pricing information
    pub pricing: Option<ProviderPricing>,
    /// Social trust score (based on recommendations)
    pub trust_score: f32,
    /// Number of recommendations
    pub recommendation_count: u32,
    /// Relay hints where this provider was found
    pub relays: Vec<String>,
}

/// Provider metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMetadata {
    /// Human-readable name
    pub name: String,
    /// Description of services
    pub description: String,
    /// Optional icon URL
    pub icon_url: Option<String>,
    /// Optional website
    pub website: Option<String>,
    /// Optional region/location
    pub region: Option<String>,
}

impl ProviderMetadata {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            icon_url: None,
            website: None,
            region: None,
        }
    }

    pub fn with_icon(mut self, url: impl Into<String>) -> Self {
        self.icon_url = Some(url.into());
        self
    }

    pub fn with_website(mut self, url: impl Into<String>) -> Self {
        self.website = Some(url.into());
        self
    }

    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }
}

impl From<HandlerMetadata> for ProviderMetadata {
    fn from(metadata: HandlerMetadata) -> Self {
        Self {
            name: metadata.name,
            description: metadata.description,
            icon_url: metadata.icon_url,
            website: metadata.website,
            region: None,
        }
    }
}

/// Provider pricing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPricing {
    /// Price in millisats
    pub amount_msats: u64,
    /// Pricing model (e.g., "per-token", "per-request", "per-minute")
    pub model: String,
    /// Optional currency (defaults to sats)
    pub currency: Option<String>,
}

impl From<PricingInfo> for ProviderPricing {
    fn from(pricing: PricingInfo) -> Self {
        Self {
            amount_msats: pricing.amount,
            model: pricing.model.unwrap_or_else(|| "per-request".to_string()),
            currency: pricing.currency,
        }
    }
}

impl ComputeProvider {
    /// Create a new compute provider
    pub fn new(pubkey: impl Into<String>, metadata: ProviderMetadata) -> Self {
        Self {
            pubkey: pubkey.into(),
            metadata,
            capabilities: Vec::new(),
            pricing: None,
            trust_score: 0.0,
            recommendation_count: 0,
            relays: Vec::new(),
        }
    }

    /// Add a capability
    pub fn add_capability(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.push(capability.into());
        self
    }

    /// Set pricing
    pub fn with_pricing(mut self, pricing: ProviderPricing) -> Self {
        self.pricing = Some(pricing);
        self
    }

    /// Add a relay hint
    pub fn add_relay(mut self, relay: impl Into<String>) -> Self {
        self.relays.push(relay.into());
        self
    }

    /// Check if provider supports a specific model
    pub fn supports_model(&self, model: &str) -> bool {
        self.capabilities
            .iter()
            .any(|c| c.eq_ignore_ascii_case(model))
    }

    /// Check if provider is in a specific region
    pub fn in_region(&self, region: &str) -> bool {
        if let Some(provider_region) = &self.metadata.region {
            provider_region.eq_ignore_ascii_case(region)
        } else {
            false
        }
    }
}

/// Provider discovery query
#[derive(Debug, Clone)]
pub struct ProviderQuery {
    /// Filter by model
    pub model: Option<String>,
    /// Filter by region
    pub region: Option<String>,
    /// Filter by minimum trust score
    pub min_trust_score: Option<f32>,
    /// Filter by maximum price
    pub max_price_msats: Option<u64>,
    /// Sort order
    pub sort_by: SortBy,
}

/// Sort order for provider results
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    /// Sort by trust score (descending)
    TrustScore,
    /// Sort by price (ascending)
    Price,
    /// Sort by number of recommendations (descending)
    Recommendations,
    /// Sort by reputation-weighted discovery ranking (descending)
    ReputationWeighted,
}

impl Default for ProviderQuery {
    fn default() -> Self {
        Self {
            model: None,
            region: None,
            min_trust_score: None,
            max_price_msats: None,
            sort_by: SortBy::TrustScore,
        }
    }
}

impl ProviderQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    pub fn with_min_trust_score(mut self, score: f32) -> Self {
        self.min_trust_score = Some(score);
        self
    }

    pub fn with_max_price(mut self, price_msats: u64) -> Self {
        self.max_price_msats = Some(price_msats);
        self
    }

    pub fn sort_by(mut self, sort_by: SortBy) -> Self {
        self.sort_by = sort_by;
        self
    }

    /// Check if a provider matches this query
    pub fn matches(&self, provider: &ComputeProvider) -> bool {
        // Check model filter
        if let Some(ref model) = self.model {
            if !provider.supports_model(model) {
                return false;
            }
        }

        // Check region filter
        if let Some(ref region) = self.region {
            if !provider.in_region(region) {
                return false;
            }
        }

        // Check trust score filter
        if let Some(min_score) = self.min_trust_score {
            if provider.trust_score < min_score {
                return false;
            }
        }

        // Check price filter
        if let Some(max_price) = self.max_price_msats {
            if let Some(ref pricing) = provider.pricing {
                if pricing.amount_msats > max_price {
                    return false;
                }
            }
        }

        true
    }

    /// Sort providers according to the sort order
    ///
    /// For ReputationWeighted sorting, you must call sort_with_reputation instead.
    pub fn sort(&self, providers: &mut [ComputeProvider]) {
        match self.sort_by {
            SortBy::TrustScore => {
                providers.sort_by(|a, b| {
                    b.trust_score
                        .partial_cmp(&a.trust_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            SortBy::Price => {
                providers.sort_by(|a, b| {
                    let a_price = a
                        .pricing
                        .as_ref()
                        .map(|p| p.amount_msats)
                        .unwrap_or(u64::MAX);
                    let b_price = b
                        .pricing
                        .as_ref()
                        .map(|p| p.amount_msats)
                        .unwrap_or(u64::MAX);
                    a_price.cmp(&b_price)
                });
            }
            SortBy::Recommendations => {
                providers.sort_by(|a, b| b.recommendation_count.cmp(&a.recommendation_count));
            }
            SortBy::ReputationWeighted => {
                // Fallback to trust score if reputation data not provided
                providers.sort_by(|a, b| {
                    b.trust_score
                        .partial_cmp(&a.trust_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }
    }

    /// Sort providers with reputation data
    ///
    /// Uses ReputationMetrics to calculate discovery weights that combine
    /// trust tier, success rate, and review ratings for optimal ranking.
    pub fn sort_with_reputation(
        &self,
        providers: &mut [ComputeProvider],
        reputation_lookup: &dyn Fn(&str) -> Option<f32>,
    ) {
        match self.sort_by {
            SortBy::ReputationWeighted => {
                providers.sort_by(|a, b| {
                    let a_weight = reputation_lookup(&a.pubkey).unwrap_or(a.trust_score);
                    let b_weight = reputation_lookup(&b.pubkey).unwrap_or(b.trust_score);
                    b_weight
                        .partial_cmp(&a_weight)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            _ => self.sort(providers),
        }
    }
}

/// Provider discovery manager
#[derive(Debug, Default)]
pub struct ProviderDiscovery {
    /// Discovered providers by pubkey
    providers: HashMap<String, ComputeProvider>,
    /// Social trust scores by handler ID
    trust_scores: HashMap<String, SocialTrustScore>,
}

impl ProviderDiscovery {
    /// Create a new provider discovery manager
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            trust_scores: HashMap::new(),
        }
    }

    /// Add a provider from a HandlerInfo event
    pub fn add_provider(&mut self, info: HandlerInfo, relay: Option<String>) {
        let mut metadata: ProviderMetadata = info.metadata.into();

        // Extract region from custom tags if present
        if let Some((_, region)) = info.custom_tags.iter().find(|(k, _)| k == "region") {
            metadata = metadata.with_region(region);
        }

        let mut provider = ComputeProvider::new(info.pubkey.clone(), metadata);

        // Add capabilities
        for capability in info.capabilities {
            provider = provider.add_capability(capability);
        }

        // Add pricing
        if let Some(pricing) = info.pricing {
            provider = provider.with_pricing(pricing.into());
        }

        // Add relay hint
        if let Some(relay) = relay {
            provider = provider.add_relay(relay);
        }

        // Update trust score if we have one
        if let Some(score) = self.trust_scores.get(&info.pubkey) {
            provider.trust_score = score.trust_score;
            provider.recommendation_count =
                score.direct_follows + score.follow_of_follows + score.two_degrees + score.unknown;
        }

        self.providers.insert(info.pubkey, provider);
    }

    /// Add a recommendation to update trust scores
    pub fn add_recommendation(
        &mut self,
        handler_id: impl Into<String>,
        social_distance: SocialDistance,
    ) {
        let handler_id = handler_id.into();
        let score = self
            .trust_scores
            .entry(handler_id.clone())
            .or_insert_with(|| SocialTrustScore::new(&handler_id));

        match social_distance {
            SocialDistance::DirectFollow => score.add_direct_follow(),
            SocialDistance::FollowOfFollow => score.add_follow_of_follow(),
            SocialDistance::TwoDegrees => score.add_two_degrees(),
            SocialDistance::Unknown => score.add_unknown(),
        }

        // Update provider trust score if it exists
        if let Some(provider) = self.providers.get_mut(&handler_id) {
            provider.trust_score = score.trust_score;
            provider.recommendation_count =
                score.direct_follows + score.follow_of_follows + score.two_degrees + score.unknown;
        }
    }

    /// Query providers with filters
    pub fn query(&self, query: &ProviderQuery) -> Vec<ComputeProvider> {
        let mut results: Vec<ComputeProvider> = self
            .providers
            .values()
            .filter(|p| query.matches(p))
            .cloned()
            .collect();

        query.sort(&mut results);
        results
    }

    /// Get a specific provider by pubkey
    pub fn get_provider(&self, pubkey: &str) -> Option<&ComputeProvider> {
        self.providers.get(pubkey)
    }

    /// Get all providers
    pub fn get_all_providers(&self) -> Vec<&ComputeProvider> {
        self.providers.values().collect()
    }

    /// Get number of providers
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }
}

/// Social distance for trust scoring
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocialDistance {
    /// Direct follow
    DirectFollow,
    /// Follow of a follow
    FollowOfFollow,
    /// Two degrees separation
    TwoDegrees,
    /// Unknown user
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_metadata() {
        let metadata = ProviderMetadata::new("Test Provider", "A test compute provider")
            .with_icon("https://example.com/icon.png")
            .with_website("https://example.com")
            .with_region("us-west");

        assert_eq!(metadata.name, "Test Provider");
        assert_eq!(metadata.region, Some("us-west".to_string()));
    }

    #[test]
    fn test_compute_provider() {
        let metadata = ProviderMetadata::new("LLM Provider", "Provides LLM inference");
        let provider = ComputeProvider::new("pubkey123", metadata)
            .add_capability("llama3")
            .add_capability("mistral")
            .with_pricing(ProviderPricing {
                amount_msats: 1000,
                model: "per-token".to_string(),
                currency: None,
            })
            .add_relay("wss://relay.example.com");

        assert_eq!(provider.capabilities.len(), 2);
        assert!(provider.supports_model("llama3"));
        assert!(provider.supports_model("MISTRAL")); // Case insensitive
        assert!(!provider.supports_model("gpt4"));
        assert_eq!(provider.relays.len(), 1);
    }

    #[test]
    fn test_provider_query_matching() {
        let metadata = ProviderMetadata::new("Test", "Test").with_region("us-west");
        let provider = ComputeProvider::new("pubkey123", metadata)
            .add_capability("llama3")
            .with_pricing(ProviderPricing {
                amount_msats: 1000,
                model: "per-token".to_string(),
                currency: None,
            });

        // Match by model
        let query = ProviderQuery::new().with_model("llama3");
        assert!(query.matches(&provider));

        let query = ProviderQuery::new().with_model("gpt4");
        assert!(!query.matches(&provider));

        // Match by region
        let query = ProviderQuery::new().with_region("us-west");
        assert!(query.matches(&provider));

        let query = ProviderQuery::new().with_region("eu-central");
        assert!(!query.matches(&provider));

        // Match by price
        let query = ProviderQuery::new().with_max_price(2000);
        assert!(query.matches(&provider));

        let query = ProviderQuery::new().with_max_price(500);
        assert!(!query.matches(&provider));
    }

    #[test]
    fn test_provider_discovery() {
        let mut discovery = ProviderDiscovery::new();

        // Add a provider
        let metadata = HandlerMetadata::new("Provider 1", "First provider");
        let info = HandlerInfo::new("pubkey1", HandlerType::ComputeProvider, metadata)
            .add_capability("llama3")
            .with_pricing(PricingInfo::new(1000));

        discovery.add_provider(info, Some("wss://relay1.com".to_string()));

        assert_eq!(discovery.provider_count(), 1);

        // Add recommendations
        discovery.add_recommendation("pubkey1", SocialDistance::DirectFollow);
        discovery.add_recommendation("pubkey1", SocialDistance::DirectFollow);

        let provider = discovery.get_provider("pubkey1").unwrap();
        assert_eq!(provider.recommendation_count, 2);
        assert!(provider.trust_score > 0.0);
    }

    #[test]
    fn test_provider_query_sorting() {
        let mut discovery = ProviderDiscovery::new();

        // Add multiple providers with different trust scores
        for i in 1..=3 {
            let metadata =
                HandlerMetadata::new(format!("Provider {}", i), format!("Provider {}", i));
            let info = HandlerInfo::new(
                format!("pubkey{}", i),
                HandlerType::ComputeProvider,
                metadata,
            );
            discovery.add_provider(info, None);
        }

        // Add recommendations (different amounts for each)
        discovery.add_recommendation("pubkey1", SocialDistance::DirectFollow);
        discovery.add_recommendation("pubkey2", SocialDistance::DirectFollow);
        discovery.add_recommendation("pubkey2", SocialDistance::DirectFollow);
        discovery.add_recommendation("pubkey3", SocialDistance::FollowOfFollow);

        // Query and sort by trust score
        let query = ProviderQuery::new().sort_by(SortBy::TrustScore);
        let results = discovery.query(&query);

        assert_eq!(results.len(), 3);
        // pubkey2 should be first (2 direct follows)
        assert_eq!(results[0].pubkey, "pubkey2");
    }

    #[test]
    fn test_social_distance() {
        let mut discovery = ProviderDiscovery::new();

        let metadata = HandlerMetadata::new("Test", "Test");
        let info = HandlerInfo::new("pubkey1", HandlerType::ComputeProvider, metadata);
        discovery.add_provider(info, None);

        // Test different social distances
        discovery.add_recommendation("pubkey1", SocialDistance::DirectFollow);
        let provider = discovery.get_provider("pubkey1").unwrap();
        let score1 = provider.trust_score;

        discovery.add_recommendation("pubkey1", SocialDistance::FollowOfFollow);
        let provider = discovery.get_provider("pubkey1").unwrap();
        let score2 = provider.trust_score;

        // Adding recommendations should increase score
        assert!(score2 > score1);
    }
}
