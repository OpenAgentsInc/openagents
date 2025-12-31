//! Decentralized compute provider types for the Nostr marketplace.
//!
//! This module provides types for compute providers offering inference,
//! compute, and other services in the marketplace.

use crate::identity::NostrIdentity;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur when working with providers
#[derive(Error, Debug)]
pub enum ProviderError {
    /// Invalid pricing configuration
    #[error("invalid pricing: {0}")]
    InvalidPricing(String),

    /// Invalid capabilities configuration
    #[error("invalid capabilities: {0}")]
    InvalidCapabilities(String),

    /// Invalid Lightning address
    #[error("invalid lightning address: {0}")]
    InvalidLightningAddress(String),
}

/// Geographic region for compute provider
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Region {
    /// US West Coast
    UsWest,
    /// US East Coast
    UsEast,
    /// US Central
    UsCentral,
    /// Western Europe
    EuWest,
    /// Central Europe
    EuCentral,
    /// Eastern Europe
    EuEast,
    /// Asia Pacific
    AsiaPacific,
    /// South Asia
    AsiaSouth,
    /// East Asia
    AsiaEast,
    /// Latin America
    Latam,
    /// Africa
    Africa,
    /// Oceania
    Oceania,
}

impl Region {
    /// Get a human-readable name for this region
    pub fn display_name(&self) -> &'static str {
        match self {
            Region::UsWest => "US West",
            Region::UsEast => "US East",
            Region::UsCentral => "US Central",
            Region::EuWest => "Western Europe",
            Region::EuCentral => "Central Europe",
            Region::EuEast => "Eastern Europe",
            Region::AsiaPacific => "Asia Pacific",
            Region::AsiaSouth => "South Asia",
            Region::AsiaEast => "East Asia",
            Region::Latam => "Latin America",
            Region::Africa => "Africa",
            Region::Oceania => "Oceania",
        }
    }
}

/// Pricing structure for compute services
///
/// # Examples
///
/// ```ignore
/// use nostr::provider::ComputePricing;
///
/// // Create pricing: 10 sats per 1k input, 20 sats per 1k output, 100 sats minimum
/// let pricing = ComputePricing::new(10, 20, 100).expect("valid pricing");
///
/// // Calculate cost for 5000 input + 2000 output tokens
/// let cost = pricing.calculate_cost(5000, 2000);
/// assert_eq!(cost, 90); // (5 * 10) + (2 * 20) = 90 sats
///
/// // Minimum is applied for small requests
/// let small_cost = pricing.calculate_cost(100, 50);
/// assert_eq!(small_cost, 100); // minimum_sats applied
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputePricing {
    /// Cost per 1,000 input tokens in satoshis
    pub per_1k_input_sats: u64,

    /// Cost per 1,000 output tokens in satoshis
    pub per_1k_output_sats: u64,

    /// Minimum payment in satoshis
    pub minimum_sats: u64,
}

impl ComputePricing {
    /// Create a new pricing structure
    pub fn new(
        per_1k_input_sats: u64,
        per_1k_output_sats: u64,
        minimum_sats: u64,
    ) -> Result<Self, ProviderError> {
        if per_1k_input_sats == 0 && per_1k_output_sats == 0 {
            return Err(ProviderError::InvalidPricing(
                "at least one rate must be non-zero".to_string(),
            ));
        }

        Ok(Self {
            per_1k_input_sats,
            per_1k_output_sats,
            minimum_sats,
        })
    }

    /// Calculate cost for a given number of tokens
    pub fn calculate_cost(&self, input_tokens: u64, output_tokens: u64) -> u64 {
        let input_cost = (input_tokens * self.per_1k_input_sats) / 1000;
        let output_cost = (output_tokens * self.per_1k_output_sats) / 1000;
        let total = input_cost + output_cost;

        // Apply minimum
        total.max(self.minimum_sats)
    }
}

/// Capabilities of a compute provider
///
/// # Examples
///
/// ```ignore
/// use nostr::provider::ComputeCapabilities;
///
/// let mut caps = ComputeCapabilities::new(
///     vec!["llama-70b".to_string(), "mistral-7b".to_string()],
///     8192,  // 8k context
///     2048,  // 2k max output
/// ).expect("valid capabilities");
///
/// assert!(caps.supports_model("llama-70b"));
/// assert!(!caps.supports_model("gpt-4"));
///
/// // Add new model
/// caps.add_model("mixtral-8x7b");
/// assert!(caps.supports_model("mixtral-8x7b"));
/// assert_eq!(caps.models.len(), 3);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeCapabilities {
    /// Supported models (e.g., ["llama-70b", "mistral-7b"])
    pub models: Vec<String>,

    /// Maximum context window size in tokens
    pub max_context: u32,

    /// Maximum output length in tokens
    pub max_output: u32,
}

impl ComputeCapabilities {
    /// Create new capabilities
    pub fn new(
        models: Vec<String>,
        max_context: u32,
        max_output: u32,
    ) -> Result<Self, ProviderError> {
        if models.is_empty() {
            return Err(ProviderError::InvalidCapabilities(
                "must support at least one model".to_string(),
            ));
        }

        if max_context == 0 {
            return Err(ProviderError::InvalidCapabilities(
                "max_context must be greater than 0".to_string(),
            ));
        }

        if max_output == 0 {
            return Err(ProviderError::InvalidCapabilities(
                "max_output must be greater than 0".to_string(),
            ));
        }

        Ok(Self {
            models,
            max_context,
            max_output,
        })
    }

    /// Check if a specific model is supported
    pub fn supports_model(&self, model: &str) -> bool {
        self.models.iter().any(|m| m == model)
    }

    /// Add a new model to capabilities
    pub fn add_model(&mut self, model: impl Into<String>) {
        let model = model.into();
        if !self.supports_model(&model) {
            self.models.push(model);
        }
    }
}

/// Provider reputation metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderReputation {
    /// Total number of jobs completed
    pub jobs_completed: u64,

    /// Success rate (0.0 to 1.0)
    pub success_rate: f32,

    /// Average latency in milliseconds
    pub avg_latency_ms: u32,

    /// Uptime percentage (0.0 to 1.0)
    pub uptime_pct: f32,
}

impl Default for ProviderReputation {
    fn default() -> Self {
        Self {
            jobs_completed: 0,
            success_rate: 0.0,
            avg_latency_ms: 0,
            uptime_pct: 1.0,
        }
    }
}

impl ProviderReputation {
    /// Get the reputation tier based on metrics
    pub fn tier(&self) -> ReputationTier {
        if self.jobs_completed < 100 {
            ReputationTier::New
        } else if self.jobs_completed < 1000 {
            if self.success_rate >= 0.95 {
                ReputationTier::Established
            } else {
                ReputationTier::New
            }
        } else {
            if self.success_rate >= 0.99 && self.uptime_pct >= 0.99 {
                // Top 10% criteria - simplified to high success + uptime
                if self.avg_latency_ms < 500 {
                    ReputationTier::Premium
                } else {
                    ReputationTier::Trusted
                }
            } else if self.success_rate >= 0.99 {
                ReputationTier::Trusted
            } else {
                ReputationTier::Established
            }
        }
    }

    /// Check if this provider is reliable (>95% success rate)
    pub fn is_reliable(&self) -> bool {
        self.success_rate >= 0.95
    }

    /// Check if this provider is fast (<1000ms avg latency)
    pub fn is_fast(&self) -> bool {
        self.avg_latency_ms < 1000
    }
}

/// Reputation tier classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReputationTier {
    /// New provider (<100 jobs)
    New,
    /// Established provider (100-1000 jobs, >95% success)
    Established,
    /// Trusted provider (1000+ jobs, >99% success)
    Trusted,
    /// Premium provider (top tier performance)
    Premium,
}

impl ReputationTier {
    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            ReputationTier::New => "New Provider",
            ReputationTier::Established => "Established Provider",
            ReputationTier::Trusted => "Trusted Provider",
            ReputationTier::Premium => "Premium Provider",
        }
    }

    /// Get the minimum jobs required for this tier
    pub fn min_jobs(&self) -> u64 {
        match self {
            ReputationTier::New => 0,
            ReputationTier::Established => 100,
            ReputationTier::Trusted => 1000,
            ReputationTier::Premium => 1000,
        }
    }
}

/// A compute provider in the marketplace
///
/// # Examples
///
/// ```ignore
/// use nostr::provider::{ComputeProvider, ComputePricing, ComputeCapabilities, Region};
/// use nostr::identity::NostrIdentity;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let identity = NostrIdentity::new("npub1...").expect("valid pubkey");
///
/// let pricing = ComputePricing::new(10, 20, 100)?;
/// let capabilities = ComputeCapabilities::new(
///     vec!["llama-70b".to_string()],
///     8192,
///     2048,
/// )?;
///
/// let provider = ComputeProvider::new(
///     identity,
///     "provider@getalby.com",
///     Region::UsWest,
///     pricing,
///     capabilities,
/// )?;
///
/// assert_eq!(provider.region, Region::UsWest);
/// assert!(provider.capabilities.supports_model("llama-70b"));
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeProvider {
    /// Nostr identity for this provider
    pub id: NostrIdentity,

    /// Lightning address for payments
    pub lightning_address: String,

    /// Geographic region
    pub region: Region,

    /// Whether the provider is currently online
    pub online: bool,

    /// Pricing structure
    pub pricing: ComputePricing,

    /// Compute capabilities
    pub capabilities: ComputeCapabilities,

    /// Reputation metrics
    pub reputation: ProviderReputation,

    /// Optional display name
    pub name: Option<String>,

    /// Optional description
    pub description: Option<String>,
}

impl ComputeProvider {
    /// Create a new compute provider
    pub fn new(
        id: NostrIdentity,
        lightning_address: impl Into<String>,
        region: Region,
        pricing: ComputePricing,
        capabilities: ComputeCapabilities,
    ) -> Result<Self, ProviderError> {
        let lightning_address = lightning_address.into();

        // Validate Lightning address format
        if !lightning_address.contains('@') || lightning_address.split('@').count() != 2 {
            return Err(ProviderError::InvalidLightningAddress(lightning_address));
        }

        Ok(Self {
            id,
            lightning_address,
            region,
            online: false,
            pricing,
            capabilities,
            reputation: ProviderReputation::default(),
            name: None,
            description: None,
        })
    }

    /// Set the provider as online
    pub fn set_online(&mut self, online: bool) {
        self.online = online;
    }

    /// Set the display name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Get the reputation tier
    pub fn tier(&self) -> ReputationTier {
        self.reputation.tier()
    }

    /// Check if this provider supports a specific model
    pub fn supports_model(&self, model: &str) -> bool {
        self.capabilities.supports_model(model)
    }

    /// Calculate cost for a job
    pub fn calculate_job_cost(&self, input_tokens: u64, output_tokens: u64) -> u64 {
        self.pricing.calculate_cost(input_tokens, output_tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_display_name() {
        assert_eq!(Region::UsWest.display_name(), "US West");
        assert_eq!(Region::EuCentral.display_name(), "Central Europe");
        assert_eq!(Region::AsiaPacific.display_name(), "Asia Pacific");
    }

    #[test]
    fn test_compute_pricing() {
        let pricing = ComputePricing::new(10, 20, 100).unwrap();

        // 1000 input, 500 output = 10 + 10 = 20 sats, but minimum is 100
        assert_eq!(pricing.calculate_cost(1000, 500), 100);

        // 10000 input, 5000 output = 100 + 100 = 200 sats
        assert_eq!(pricing.calculate_cost(10000, 5000), 200);
    }

    #[test]
    fn test_pricing_validation() {
        assert!(ComputePricing::new(0, 0, 100).is_err());
        assert!(ComputePricing::new(10, 0, 100).is_ok());
        assert!(ComputePricing::new(0, 20, 100).is_ok());
    }

    #[test]
    fn test_compute_capabilities() {
        let mut caps = ComputeCapabilities::new(
            vec!["llama-70b".to_string(), "mistral-7b".to_string()],
            8192,
            2048,
        )
        .unwrap();

        assert!(caps.supports_model("llama-70b"));
        assert!(caps.supports_model("mistral-7b"));
        assert!(!caps.supports_model("gpt-4"));

        caps.add_model("gpt-4");
        assert!(caps.supports_model("gpt-4"));

        // Adding again should not duplicate
        caps.add_model("gpt-4");
        assert_eq!(caps.models.len(), 3);
    }

    #[test]
    fn test_capabilities_validation() {
        assert!(ComputeCapabilities::new(vec![], 8192, 2048).is_err());
        assert!(ComputeCapabilities::new(vec!["model".to_string()], 0, 2048).is_err());
        assert!(ComputeCapabilities::new(vec!["model".to_string()], 8192, 0).is_err());
    }

    #[test]
    fn test_provider_reputation_tier() {
        let mut rep = ProviderReputation::default();

        // New provider
        assert_eq!(rep.tier(), ReputationTier::New);

        // Established provider
        rep.jobs_completed = 500;
        rep.success_rate = 0.96;
        assert_eq!(rep.tier(), ReputationTier::Established);

        // Trusted provider
        rep.jobs_completed = 1500;
        rep.success_rate = 0.99;
        rep.uptime_pct = 0.99;
        rep.avg_latency_ms = 1000;
        assert_eq!(rep.tier(), ReputationTier::Trusted);

        // Premium provider
        rep.avg_latency_ms = 400;
        assert_eq!(rep.tier(), ReputationTier::Premium);
    }

    #[test]
    fn test_reputation_checks() {
        let mut rep = ProviderReputation::default();
        rep.success_rate = 0.97;
        rep.avg_latency_ms = 800;

        assert!(rep.is_reliable());
        assert!(rep.is_fast());

        rep.success_rate = 0.90;
        assert!(!rep.is_reliable());
    }

    #[test]
    fn test_reputation_tier_info() {
        assert_eq!(ReputationTier::New.min_jobs(), 0);
        assert_eq!(ReputationTier::Established.min_jobs(), 100);
        assert_eq!(ReputationTier::Trusted.min_jobs(), 1000);
        assert_eq!(ReputationTier::Premium.description(), "Premium Provider");
    }

    #[test]
    fn test_compute_provider() {
        let pubkey = "a".repeat(64);
        let identity = NostrIdentity::new(&pubkey).unwrap();

        let pricing = ComputePricing::new(10, 20, 100).unwrap();
        let capabilities =
            ComputeCapabilities::new(vec!["llama-70b".to_string()], 8192, 2048).unwrap();

        let provider = ComputeProvider::new(
            identity,
            "provider@domain.com",
            Region::UsWest,
            pricing,
            capabilities,
        )
        .unwrap()
        .with_name("Test Provider")
        .with_description("A test provider");

        assert_eq!(provider.name.as_ref().unwrap(), "Test Provider");
        assert_eq!(provider.region, Region::UsWest);
        assert!(!provider.online);
        assert!(provider.supports_model("llama-70b"));
        assert_eq!(provider.tier(), ReputationTier::New);

        let cost = provider.calculate_job_cost(1000, 500);
        assert_eq!(cost, 100); // Minimum
    }

    #[test]
    fn test_invalid_lightning_address() {
        let pubkey = "a".repeat(64);
        let identity = NostrIdentity::new(&pubkey).unwrap();
        let pricing = ComputePricing::new(10, 20, 100).unwrap();
        let capabilities = ComputeCapabilities::new(vec!["model".to_string()], 8192, 2048).unwrap();

        assert!(
            ComputeProvider::new(identity, "invalid", Region::UsWest, pricing, capabilities,)
                .is_err()
        );
    }
}
