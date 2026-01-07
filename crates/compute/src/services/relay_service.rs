//! Nostr relay service
//!
//! Manages connections to Nostr relays and subscriptions for NIP-90 job requests.

use nostr_client::{PoolConfig, RelayPool};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info, warn};

/// Default Nostr relays for the compute provider
pub const DEFAULT_RELAYS: &[&str] = &[
    "wss://relay.openagents.com",
    "wss://relay.damus.io",
    "wss://nos.lol",
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

    #[error("not connected")]
    NotConnected,
}

impl From<nostr_client::ClientError> for RelayError {
    fn from(err: nostr_client::ClientError) -> Self {
        match err {
            nostr_client::ClientError::Connection(msg) => RelayError::ConnectionFailed(msg),
            nostr_client::ClientError::Subscription(msg) => RelayError::SubscriptionFailed(msg),
            nostr_client::ClientError::PublishFailed(msg) => RelayError::PublishFailed(msg),
            nostr_client::ClientError::NotConnected => RelayError::NotConnected,
            other => RelayError::ConnectionFailed(other.to_string()),
        }
    }
}

/// Service for managing Nostr relay connections
pub struct RelayService {
    /// URLs of relays to connect to
    relay_urls: Vec<String>,
    /// Relay pool for managing connections
    pool: Arc<RwLock<Option<RelayPool>>>,
    /// Currently connected relays
    connected: Arc<RwLock<Vec<String>>>,
    /// Optional auth key for NIP-42 authentication
    auth_key: Arc<RwLock<Option<[u8; 32]>>>,
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
            pool: Arc::new(RwLock::new(None)),
            connected: Arc::new(RwLock::new(Vec::new())),
            auth_key: Arc::new(RwLock::new(None)),
        }
    }

    /// Set the authentication key for NIP-42 auth
    ///
    /// This should be called before connecting to relays.
    /// The key will be used to automatically respond to AUTH challenges.
    pub async fn set_auth_key(&self, key: [u8; 32]) {
        *self.auth_key.write().await = Some(key);
        // If already connected, also set on the pool
        if let Some(ref pool) = *self.pool.read().await {
            pool.set_auth_key(key).await;
        }
        info!("Auth key set for relay service");
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
        info!("Connecting to {} relays...", self.relay_urls.len());

        // Create relay pool with default config
        let pool = RelayPool::new(PoolConfig::default());

        // Add all configured relays
        for url in &self.relay_urls {
            if let Err(e) = pool.add_relay(url).await {
                warn!("Failed to add relay {}: {}", url, e);
            }
        }

        // Set auth key if configured (must be done before connect)
        if let Some(key) = *self.auth_key.read().await {
            info!("Setting NIP-42 auth key on relay pool");
            pool.set_auth_key(key).await;
        }

        // Connect to all relays
        pool.connect_all().await.map_err(|e| {
            RelayError::ConnectionFailed(format!("Failed to connect to relays: {}", e))
        })?;

        // Get pool stats to determine connected relays
        let stats = pool.pool_stats().await;
        let connected_urls: Vec<String> = if stats.connected_relays > 0 {
            // At least some relays connected
            self.relay_urls.clone()
        } else {
            Vec::new()
        };

        if connected_urls.is_empty() {
            return Err(RelayError::ConnectionFailed(
                "Could not connect to any relays".into(),
            ));
        }

        info!("Connected to {} relays", connected_urls.len());

        // Store connected relays and pool
        *self.connected.write().await = connected_urls;
        *self.pool.write().await = Some(pool);

        Ok(())
    }

    /// Disconnect from all relays
    pub async fn disconnect(&self) {
        if let Some(pool) = self.pool.write().await.take() {
            if let Err(e) = pool.disconnect_all().await {
                warn!("Error during disconnect: {}", e);
            }
        }
        self.connected.write().await.clear();
        info!("Disconnected from all relays");
    }

    /// Subscribe to NIP-90 job requests for a specific pubkey
    ///
    /// Creates a subscription for kind 5000-5999 events (NIP-90 job requests)
    /// addressed to the given pubkey.
    ///
    /// # Arguments
    /// * `pubkey` - The hex-encoded pubkey to receive job requests for
    ///
    /// # Returns
    /// A tuple of (subscription_id, event_receiver) for receiving job request events
    pub async fn subscribe_job_requests(
        &self,
        pubkey: &str,
    ) -> Result<(String, mpsc::Receiver<nostr::Event>), RelayError> {
        let pool_guard = self.pool.read().await;
        let pool = pool_guard.as_ref().ok_or(RelayError::NotConnected)?;

        // Generate subscription ID
        let sub_id = format!("nip90-jobs-{}", &pubkey[..8]);

        // Build NIP-90 job request filter
        // Job requests are kinds 5000-5999 (range 5xxx)
        // They should have a "p" tag with the provider's pubkey
        let filters = vec![serde_json::json!({
            "kinds": [5000, 5001, 5002, 5003, 5004, 5005, 5050, 5100, 5250],
            "#p": [pubkey],
            "limit": 100
        })];

        debug!(
            "Subscribing to NIP-90 job requests for pubkey: {}...",
            &pubkey[..8]
        );

        let rx = pool.subscribe(&sub_id, &filters).await?;

        info!("Subscribed to NIP-90 job requests with ID: {}", sub_id);
        Ok((sub_id, rx))
    }

    /// Publish an event to all connected relays
    ///
    /// # Arguments
    /// * `event` - The Nostr event to publish
    ///
    /// # Returns
    /// Number of relays that accepted the event
    pub async fn publish(&self, event: nostr::Event) -> Result<usize, RelayError> {
        let pool_guard = self.pool.read().await;
        let pool = pool_guard.as_ref().ok_or(RelayError::NotConnected)?;

        debug!("Publishing event {} to relays", event.id);

        let confirmations = pool.publish(&event).await?;
        let success_count = confirmations.iter().filter(|c| c.accepted).count();

        if success_count == 0 {
            return Err(RelayError::PublishFailed(
                "No relays accepted the event".into(),
            ));
        }

        info!(
            "Published event {} to {}/{} relays",
            event.id,
            success_count,
            confirmations.len()
        );

        Ok(success_count)
    }

    /// Unsubscribe from a subscription
    pub async fn unsubscribe(&self, subscription_id: &str) -> Result<(), RelayError> {
        let pool_guard = self.pool.read().await;
        let pool = pool_guard.as_ref().ok_or(RelayError::NotConnected)?;

        pool.unsubscribe(subscription_id).await?;
        debug!("Unsubscribed from {}", subscription_id);

        Ok(())
    }

    /// Check if connected to any relays
    pub async fn is_connected(&self) -> bool {
        !self.connected.read().await.is_empty()
    }

    /// Get pool statistics
    pub async fn get_stats(&self) -> Option<nostr_client::PoolStats> {
        let pool_guard = self.pool.read().await;
        let pool = pool_guard.as_ref()?;
        Some(pool.pool_stats().await)
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
        assert!(
            service
                .relay_urls()
                .contains(&"wss://relay.damus.io".to_string())
        );
    }

    #[test]
    fn test_custom_relays() {
        let urls = vec!["wss://custom.relay".to_string()];
        let service = RelayService::with_relays(urls.clone());
        assert_eq!(service.relay_urls(), &urls);
    }

    #[tokio::test]
    async fn test_not_connected() {
        let service = RelayService::new();

        // Should fail because not connected
        let result = service.subscribe_job_requests("abc123").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RelayError::NotConnected));
    }
}
