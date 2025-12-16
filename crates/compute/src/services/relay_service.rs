//! Nostr relay service
//!
//! Manages connections to Nostr relays and subscriptions for NIP-90 job requests.

use nostr_client::RelayPool;
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
    /// Relay pool for managing connections
    pool: Arc<RwLock<Option<RelayPool>>>,
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
            pool: Arc::new(RwLock::new(None)),
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
        let pool = RelayPool::new();

        for url in &self.relay_urls {
            match pool.add_relay(url).await {
                Ok(_) => {
                    log::info!("Connected to relay: {}", url);
                    self.connected.write().await.push(url.clone());
                }
                Err(e) => {
                    log::warn!("Failed to connect to relay {}: {}", url, e);
                }
            }
        }

        *self.pool.write().await = Some(pool);

        if self.connected.read().await.is_empty() {
            return Err(RelayError::ConnectionFailed(
                "Could not connect to any relays".into(),
            ));
        }

        Ok(())
    }

    /// Disconnect from all relays
    pub async fn disconnect(&self) {
        *self.pool.write().await = None;
        self.connected.write().await.clear();
    }

    /// Subscribe to NIP-90 job requests for a specific pubkey
    pub async fn subscribe_job_requests(&self, pubkey: &str) -> Result<String, RelayError> {
        let pool_guard = self.pool.read().await;
        let pool = pool_guard
            .as_ref()
            .ok_or_else(|| RelayError::SubscriptionFailed("Not connected".into()))?;

        // Create filter for job requests (kinds 5000-5999) tagged with our pubkey
        let filter = nostr_client::Filter::new()
            .kinds(vec![
                5000, 5001, 5002, 5050, 5100, 5250, // Common DVM kinds
            ])
            .tag("p", vec![pubkey.to_string()]);

        let sub_id = pool
            .subscribe_all(vec![filter])
            .await
            .map_err(|e| RelayError::SubscriptionFailed(e.to_string()))?;

        Ok(sub_id)
    }

    /// Publish an event to all connected relays
    pub async fn publish(&self, event: nostr::Event) -> Result<(), RelayError> {
        let pool_guard = self.pool.read().await;
        let pool = pool_guard
            .as_ref()
            .ok_or_else(|| RelayError::PublishFailed("Not connected".into()))?;

        let results = pool.publish(event).await;

        // Check if any relay succeeded
        let any_success = results.iter().any(|(_, r)| r.is_ok());
        if !any_success && !results.is_empty() {
            let errors: Vec<_> = results.iter()
                .filter_map(|(url, r)| r.as_ref().err().map(|e| format!("{}: {}", url, e)))
                .collect();
            return Err(RelayError::PublishFailed(errors.join(", ")));
        }

        Ok(())
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
