//! Provider module - Advertise compute capabilities
//!
//! This module provides the provider side of the compute marketplace,
//! allowing nodes to advertise their compute capabilities via NIP-89.

use crate::relay::{MarketplaceRelay, RelayError};
use nostr::{
    Event, EventTemplate, HandlerInfo, HandlerMetadata, HandlerType, KIND_HANDLER_INFO, PricingInfo,
};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Provider name
    pub name: String,
    /// Provider description
    pub description: String,
    /// Optional website
    pub website: Option<String>,
    /// Optional icon URL
    pub icon_url: Option<String>,
    /// Provider region/location
    pub region: Option<String>,
    /// Availability schedule (e.g., "always", "weekdays 9-17")
    pub schedule: String,
    /// Supported models/capabilities
    pub capabilities: Vec<String>,
    /// Pricing per 1k input tokens (in millisats)
    pub price_per_1k_input: u64,
    /// Pricing per 1k output tokens (in millisats)
    pub price_per_1k_output: u64,
    /// Relays to advertise on
    pub relays: Vec<String>,
    /// Re-advertisement interval (in seconds)
    pub readvertise_interval_secs: u64,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            name: "OpenAgents Compute Provider".to_string(),
            description: "Compute marketplace provider powered by OpenAgents".to_string(),
            website: None,
            icon_url: None,
            region: None,
            schedule: "always".to_string(),
            capabilities: vec!["llama3".to_string(), "mistral".to_string()],
            price_per_1k_input: 10,
            price_per_1k_output: 20,
            relays: vec![
                "wss://relay.damus.io".to_string(),
                "wss://nos.lol".to_string(),
            ],
            readvertise_interval_secs: 3600, // 1 hour
        }
    }
}

impl ProviderConfig {
    /// Create a new provider config
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            ..Default::default()
        }
    }

    /// Set website
    pub fn with_website(mut self, url: impl Into<String>) -> Self {
        self.website = Some(url.into());
        self
    }

    /// Set icon URL
    pub fn with_icon(mut self, url: impl Into<String>) -> Self {
        self.icon_url = Some(url.into());
        self
    }

    /// Set region
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Set availability schedule
    pub fn with_schedule(mut self, schedule: impl Into<String>) -> Self {
        self.schedule = schedule.into();
        self
    }

    /// Add a capability
    pub fn add_capability(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.push(capability.into());
        self
    }

    /// Set capabilities
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Set pricing
    pub fn with_pricing(mut self, input_msats: u64, output_msats: u64) -> Self {
        self.price_per_1k_input = input_msats;
        self.price_per_1k_output = output_msats;
        self
    }

    /// Add a relay
    pub fn add_relay(mut self, relay: impl Into<String>) -> Self {
        self.relays.push(relay.into());
        self
    }

    /// Set relays
    pub fn with_relays(mut self, relays: Vec<String>) -> Self {
        self.relays = relays;
        self
    }

    /// Set readvertisement interval
    pub fn with_readvertise_interval(mut self, seconds: u64) -> Self {
        self.readvertise_interval_secs = seconds;
        self
    }

    /// Convert to HandlerInfo event
    pub fn to_handler_info(&self, pubkey: impl Into<String>) -> HandlerInfo {
        let mut metadata = HandlerMetadata::new(&self.name, &self.description);

        if let Some(ref website) = self.website {
            metadata = metadata.with_website(website);
        }

        if let Some(ref icon) = self.icon_url {
            metadata = metadata.with_icon(icon);
        }

        let mut info = HandlerInfo::new(pubkey, HandlerType::ComputeProvider, metadata);

        // Add capabilities
        for capability in &self.capabilities {
            info = info.add_capability(capability);
        }

        // Add pricing (average of input/output for simplicity)
        let avg_price = (self.price_per_1k_input + self.price_per_1k_output) / 2;
        info = info.with_pricing(PricingInfo::new(avg_price).with_model("per-token"));

        // Add region as custom tag if present
        if let Some(ref region) = self.region {
            info = info.add_custom_tag("region", region);
        }

        if !self.schedule.is_empty() {
            info = info.add_custom_tag("schedule", &self.schedule);
        }

        info
    }
}

/// Provider state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderState {
    /// Provider is offline (not accepting jobs)
    Offline,
    /// Provider is online and accepting jobs
    Online,
    /// Provider is paused (temporarily not accepting new jobs)
    Paused,
}

/// Provider advertisement manager
pub struct Provider {
    /// Provider configuration
    config: ProviderConfig,
    /// Current state
    state: ProviderState,
    /// Last advertisement timestamp
    last_advertised: Option<SystemTime>,
}

impl Provider {
    /// Create a new provider
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            state: ProviderState::Offline,
            last_advertised: None,
        }
    }

    /// Get provider configuration
    pub fn config(&self) -> &ProviderConfig {
        &self.config
    }

    /// Get current state
    pub fn state(&self) -> ProviderState {
        self.state
    }

    /// Go online (start accepting jobs)
    pub fn go_online(&mut self) {
        self.state = ProviderState::Online;
    }

    /// Go offline (stop accepting jobs)
    pub fn go_offline(&mut self) {
        self.state = ProviderState::Offline;
    }

    /// Pause (temporarily stop accepting new jobs)
    pub fn pause(&mut self) {
        self.state = ProviderState::Paused;
    }

    /// Check if provider is online
    pub fn is_online(&self) -> bool {
        self.state == ProviderState::Online
    }

    /// Check if provider needs to re-advertise
    pub fn needs_readvertisement(&self) -> bool {
        match self.last_advertised {
            None => true, // Never advertised
            Some(last) => {
                let now = SystemTime::now();
                let elapsed = now.duration_since(last).unwrap_or(Duration::from_secs(0));
                elapsed.as_secs() >= self.config.readvertise_interval_secs
            }
        }
    }

    /// Create advertisement event
    pub fn create_advertisement(&mut self, pubkey: impl Into<String>) -> HandlerInfo {
        // Update last advertised timestamp
        self.last_advertised = Some(SystemTime::now());

        // Create HandlerInfo event
        self.config.to_handler_info(pubkey)
    }

    /// Update provider configuration
    pub fn update_config(&mut self, config: ProviderConfig) {
        self.config = config;
        // Force re-advertisement on config change
        self.last_advertised = None;
    }

    /// Get time until next advertisement
    pub fn time_until_next_advertisement(&self) -> Option<Duration> {
        match self.last_advertised {
            None => Some(Duration::from_secs(0)), // Ready now
            Some(last) => {
                let now = SystemTime::now();
                let elapsed = now.duration_since(last).unwrap_or(Duration::from_secs(0));
                let interval = Duration::from_secs(self.config.readvertise_interval_secs);

                if elapsed >= interval {
                    Some(Duration::from_secs(0)) // Ready now
                } else {
                    Some(interval - elapsed)
                }
            }
        }
    }

    /// Advertise capabilities on Nostr relays
    ///
    /// This publishes a NIP-89 handler info event (kind 31990) advertising
    /// the provider's compute capabilities, pricing, and availability.
    pub async fn advertise(
        &mut self,
        relay: &MarketplaceRelay,
        secret_key: &str,
    ) -> Result<Event, RelayError> {
        // Create handler info from config
        let pubkey = self.get_pubkey_from_secret(secret_key)?;
        let handler_info = self.create_advertisement(&pubkey);

        // Build NIP-89 event (kind 31990)
        let event = self.build_advertisement_event(&handler_info, secret_key)?;

        // Publish to relays
        relay.publish(&event).await?;

        Ok(event)
    }

    /// Parse secret key from hex string
    fn parse_secret_key(&self, secret_key: &str) -> Result<[u8; 32], RelayError> {
        let bytes = hex::decode(secret_key)
            .map_err(|e| RelayError::Client(format!("Invalid secret key hex: {}", e)))?;

        if bytes.len() != 32 {
            return Err(RelayError::Client(format!(
                "Invalid secret key length: expected 32 bytes, got {}",
                bytes.len()
            )));
        }

        let mut sk_bytes = [0u8; 32];
        sk_bytes.copy_from_slice(&bytes);
        Ok(sk_bytes)
    }

    /// Get pubkey from secret key
    fn get_pubkey_from_secret(&self, secret_key: &str) -> Result<String, RelayError> {
        use nostr::get_public_key_hex;

        let sk_bytes = self.parse_secret_key(secret_key)?;
        let pubkey = get_public_key_hex(&sk_bytes)
            .map_err(|e| RelayError::Client(format!("Failed to derive pubkey: {}", e)))?;

        Ok(pubkey)
    }

    /// Build advertisement event (kind 31990)
    fn build_advertisement_event(
        &self,
        handler_info: &HandlerInfo,
        secret_key: &str,
    ) -> Result<Event, RelayError> {
        use nostr::finalize_event;

        let sk_bytes = self.parse_secret_key(secret_key)?;

        // Set content from metadata
        let content = serde_json::json!({
            "name": handler_info.metadata.name,
            "description": handler_info.metadata.description,
            "icon_url": handler_info.metadata.icon_url,
            "website": handler_info.metadata.website,
        })
        .to_string();

        // Create event template for kind 31990 (handler info)
        let template = EventTemplate {
            kind: KIND_HANDLER_INFO,
            tags: handler_info.to_tags(),
            content,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        // Sign and finalize event
        let event = finalize_event(&template, &sk_bytes)
            .map_err(|e| RelayError::Client(format!("Failed to sign event: {}", e)))?;

        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_config_default() {
        let config = ProviderConfig::default();
        assert_eq!(config.name, "OpenAgents Compute Provider");
        assert_eq!(config.schedule, "always");
        assert!(!config.capabilities.is_empty());
        assert!(!config.relays.is_empty());
    }

    #[test]
    fn test_provider_config_builder() {
        let config = ProviderConfig::new("My Provider", "A test provider")
            .with_website("https://example.com")
            .with_icon("https://example.com/icon.png")
            .with_region("us-west")
            .with_schedule("weekdays 9-17")
            .add_capability("gpt-4")
            .with_pricing(15, 30)
            .add_relay("wss://custom.relay");

        assert_eq!(config.name, "My Provider");
        assert_eq!(config.description, "A test provider");
        assert_eq!(config.website, Some("https://example.com".to_string()));
        assert_eq!(config.region, Some("us-west".to_string()));
        assert_eq!(config.schedule, "weekdays 9-17");
        assert!(config.capabilities.contains(&"gpt-4".to_string()));
        assert_eq!(config.price_per_1k_input, 15);
        assert_eq!(config.price_per_1k_output, 30);
    }

    #[test]
    fn test_provider_config_to_handler_info() {
        let config = ProviderConfig::new("Test Provider", "Test description")
            .with_website("https://test.com")
            .add_capability("llama3")
            .with_pricing(10, 20);

        let info = config.to_handler_info("test_pubkey");

        assert_eq!(info.pubkey, "test_pubkey");
        assert_eq!(info.handler_type, HandlerType::ComputeProvider);
        assert_eq!(info.metadata.name, "Test Provider");
        assert!(info.capabilities.contains(&"llama3".to_string()));
        assert!(info.pricing.is_some());
        assert!(
            info.custom_tags
                .iter()
                .any(|(key, value)| key == "schedule" && value == "always")
        );
    }

    #[test]
    fn test_provider_state_transitions() {
        let config = ProviderConfig::default();
        let mut provider = Provider::new(config);

        assert_eq!(provider.state(), ProviderState::Offline);
        assert!(!provider.is_online());

        provider.go_online();
        assert_eq!(provider.state(), ProviderState::Online);
        assert!(provider.is_online());

        provider.pause();
        assert_eq!(provider.state(), ProviderState::Paused);
        assert!(!provider.is_online());

        provider.go_offline();
        assert_eq!(provider.state(), ProviderState::Offline);
        assert!(!provider.is_online());
    }

    #[test]
    fn test_provider_needs_readvertisement() {
        let config = ProviderConfig::default().with_readvertise_interval(60); // 1 minute
        let mut provider = Provider::new(config);

        // Never advertised - needs advertisement
        assert!(provider.needs_readvertisement());

        // Create advertisement
        let _info = provider.create_advertisement("test_pubkey");

        // Just advertised - doesn't need it yet
        assert!(!provider.needs_readvertisement());
    }

    #[test]
    fn test_provider_create_advertisement() {
        let config = ProviderConfig::new("Test", "Test provider")
            .add_capability("llama3")
            .with_pricing(10, 20);

        let mut provider = Provider::new(config);
        assert!(provider.last_advertised.is_none());

        let info = provider.create_advertisement("pubkey123");

        assert_eq!(info.pubkey, "pubkey123");
        assert_eq!(info.metadata.name, "Test");
        assert!(provider.last_advertised.is_some());
    }

    #[test]
    fn test_provider_update_config() {
        let config1 = ProviderConfig::new("Provider 1", "First config");
        let mut provider = Provider::new(config1);

        // Advertise with first config
        let _info = provider.create_advertisement("pubkey");
        assert!(provider.last_advertised.is_some());

        // Update config
        let config2 = ProviderConfig::new("Provider 2", "Second config");
        provider.update_config(config2);

        // Should need re-advertisement after config change
        assert!(provider.needs_readvertisement());
        assert_eq!(provider.config().name, "Provider 2");
    }

    #[test]
    fn test_provider_time_until_next_advertisement() {
        let config = ProviderConfig::default().with_readvertise_interval(3600); // 1 hour
        let mut provider = Provider::new(config);

        // Never advertised - ready now
        assert_eq!(
            provider.time_until_next_advertisement(),
            Some(Duration::from_secs(0))
        );

        // Create advertisement
        let _info = provider.create_advertisement("pubkey");

        // Should have ~1 hour until next
        let time_until = provider.time_until_next_advertisement().unwrap();
        assert!(time_until.as_secs() > 3500); // Should be close to 3600
    }

    #[test]
    fn test_provider_config_with_capabilities() {
        let config = ProviderConfig::new("Test", "Test")
            .with_capabilities(vec!["model1".to_string(), "model2".to_string()]);

        assert_eq!(config.capabilities.len(), 2);
        assert!(config.capabilities.contains(&"model1".to_string()));
        assert!(config.capabilities.contains(&"model2".to_string()));
    }

    #[test]
    fn test_provider_config_with_relays() {
        let relays = vec![
            "wss://relay1.com".to_string(),
            "wss://relay2.com".to_string(),
        ];
        let config = ProviderConfig::new("Test", "Test").with_relays(relays.clone());

        assert_eq!(config.relays, relays);
    }
}
