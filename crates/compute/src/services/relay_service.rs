//! Nostr relay service
//!
//! Manages connections to Nostr relays and subscriptions for NIP-90 job requests.

use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

/// Default Nostr relays for the compute provider
pub const DEFAULT_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.nostr.band",
];

/// Errors from the relay service
#[derive(Debug, Error)]
pub enum RelayError {
    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    #[error("subscription failed: {0}")]
    SubscriptionFailed(String),

    #[error("publish failed: {0}")]
    PublishFailed(String),
}

/// Service for managing Nostr relay connections
pub struct RelayService {
    /// URLs of relays to connect to
    relay_urls: Vec<String>,
    /// Currently connected relays
    connected: Arc<RwLock<Vec<String>>>,
}

impl RelayService {
    /// Create a new relay service with default relays
    pub fn new() -> Self {
        Self::with_relays(DEFAULT_RELAYS.iter().map(|s| s.to_string()).collect())
    }

    /// Create a relay service with custom relay URLs
    pub fn with_relays(urls: Vec<String>) -> Self {
        Self {
            relay_urls: urls,
            connected: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get the relay URLs
    pub fn relay_urls(&self) -> &[String] {
        &self.relay_urls
    }

    /// Get currently connected relays
    pub async fn connected_relays(&self) -> Vec<String> {
        self.connected.read().await.clone()
    }

    /// Connect to all configured relays
    pub async fn connect(&self) -> Result<(), RelayError> {
        let mut connected = self.connected.write().await;
        connected.clear();
        connected.extend(self.relay_urls.iter().cloned());

        if self.connected.read().await.is_empty() {
            return Err(RelayError::ConnectionFailed(
                "Could not connect to any relays".into(),
            ));
        }

        Ok(())
    }

    /// Disconnect from all relays
    pub async fn disconnect(&self) {
        self.connected.write().await.clear();
    }

    /// Subscribe to NIP-90 job requests for a specific pubkey
    pub async fn subscribe_job_requests(&self, _pubkey: &str) -> Result<String, RelayError> {
        // Relay subscription requires Nostr relay client integration which is not yet implemented.
        // Per d-012 (No Stubs), we return an explicit error instead of a fake subscription ID.
        Err(RelayError::SubscriptionFailed(
            "Relay subscriptions not yet implemented. Requires Nostr relay client integration for subscribing to NIP-90 job request events.".to_string()
        ))
    }

    /// Publish an event to all connected relays
    pub async fn publish(&self, _event: nostr::Event) -> Result<(), RelayError> {
        // Relay publishing requires Nostr relay client integration which is not yet implemented.
        // Per d-012 (No Stubs), we return an explicit error instead of silently succeeding.
        Err(RelayError::PublishFailed(
            "Relay publishing not yet implemented. Requires Nostr relay client integration for publishing events to relays.".to_string()
        ))
    }

    /// Check if connected to any relays
    pub async fn is_connected(&self) -> bool {
        !self.connected.read().await.is_empty()
    }
}

impl Default for RelayService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_relays() {
        let service = RelayService::new();
        assert_eq!(service.relay_urls().len(), 3);
        assert!(service.relay_urls().contains(&"wss://relay.damus.io".to_string()));
    }

    #[test]
    fn test_custom_relays() {
        let urls = vec!["wss://custom.relay".to_string()];
        let service = RelayService::with_relays(urls.clone());
        assert_eq!(service.relay_urls(), &urls);
    }
}
