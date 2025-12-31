//! Relay integration for marketplace discovery
//!
//! This module provides relay pool management for discovering and fetching
//! marketplace content (skills, data, compute) from Nostr relays.

use nostr::{Event, KIND_HANDLER_INFO};
use nostr_client::{PoolConfig, RelayPool};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

/// Default relays for marketplace discovery
const DEFAULT_MARKETPLACE_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://relay.primal.net",
    "wss://relay.nostr.band",
    "wss://nos.lol",
];

/// Get marketplace relays from environment or use defaults
///
/// Set MARKETPLACE_RELAYS environment variable as comma-separated URLs:
/// MARKETPLACE_RELAYS="wss://relay1.com,wss://relay2.com"
pub fn get_marketplace_relays() -> Vec<String> {
    std::env::var("MARKETPLACE_RELAYS")
        .ok()
        .map(|relays| {
            relays
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .filter(|v: &Vec<String>| !v.is_empty())
        .unwrap_or_else(|| {
            DEFAULT_MARKETPLACE_RELAYS
                .iter()
                .map(|s| s.to_string())
                .collect()
        })
}

/// Errors that can occur during relay operations
#[derive(Debug, Error)]
pub enum RelayError {
    #[error("client error: {0}")]
    Client(String),

    #[error("connection error: {0}")]
    Connection(String),

    #[error("subscription error: {0}")]
    Subscription(String),

    #[error("no relays connected")]
    NoRelays,

    #[error("timeout waiting for events")]
    Timeout,
}

impl From<nostr_client::ClientError> for RelayError {
    fn from(err: nostr_client::ClientError) -> Self {
        RelayError::Client(err.to_string())
    }
}

/// Marketplace relay manager
pub struct MarketplaceRelay {
    pool: Arc<RelayPool>,
    connected: Arc<RwLock<bool>>,
}

impl MarketplaceRelay {
    /// Create a new marketplace relay manager
    pub fn new() -> Self {
        let config = PoolConfig::default();
        Self {
            pool: Arc::new(RelayPool::new(config)),
            connected: Arc::new(RwLock::new(false)),
        }
    }

    /// Create with custom pool configuration
    pub fn with_config(config: PoolConfig) -> Self {
        Self {
            pool: Arc::new(RelayPool::new(config)),
            connected: Arc::new(RwLock::new(false)),
        }
    }

    /// Connect to default marketplace relays
    pub async fn connect(&self) -> Result<(), RelayError> {
        let relays = get_marketplace_relays();
        let relay_refs: Vec<&str> = relays.iter().map(|s| s.as_str()).collect();
        self.connect_to_relays(&relay_refs).await
    }

    /// Connect to specific relays
    pub async fn connect_to_relays(&self, relays: &[&str]) -> Result<(), RelayError> {
        // Add relays to pool
        for relay in relays {
            self.pool.add_relay(relay).await?;
        }

        // Connect to all relays
        self.pool.connect_all().await?;

        *self.connected.write().await = true;
        Ok(())
    }

    /// Disconnect from all relays
    pub async fn disconnect(&self) -> Result<(), RelayError> {
        self.pool.disconnect_all().await?;
        *self.connected.write().await = false;
        Ok(())
    }

    /// Check if connected to any relays
    pub async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }

    /// Subscribe to handler info events (NIP-89 kind 31990)
    ///
    /// This retrieves skill/agent/compute provider advertisements
    pub async fn subscribe_handlers(
        &self,
        subscription_id: &str,
        handler_type: Option<&str>,
    ) -> Result<tokio::sync::mpsc::Receiver<Event>, RelayError> {
        if !self.is_connected().await {
            return Err(RelayError::NoRelays);
        }

        // Build filter for NIP-89 handler info events
        let mut filter = serde_json::json!({
            "kinds": [KIND_HANDLER_INFO],
            "limit": 100,
        });

        // Add handler type filter if specified
        if let Some(htype) = handler_type {
            filter["#handler"] = serde_json::json!([htype]);
        }

        let filters = vec![filter];
        let rx = self.pool.subscribe(subscription_id, &filters).await?;

        Ok(rx)
    }

    /// Subscribe to skill handler events
    pub async fn subscribe_skills(
        &self,
        subscription_id: &str,
    ) -> Result<tokio::sync::mpsc::Receiver<Event>, RelayError> {
        self.subscribe_handlers(subscription_id, Some("skill"))
            .await
    }

    /// Subscribe to compute provider events
    pub async fn subscribe_compute_providers(
        &self,
        subscription_id: &str,
    ) -> Result<tokio::sync::mpsc::Receiver<Event>, RelayError> {
        self.subscribe_handlers(subscription_id, Some("compute_provider"))
            .await
    }

    /// Subscribe to file metadata events (NIP-94 kind 1063)
    pub async fn subscribe_file_metadata(
        &self,
        subscription_id: &str,
    ) -> Result<tokio::sync::mpsc::Receiver<Event>, RelayError> {
        if !self.is_connected().await {
            return Err(RelayError::NoRelays);
        }

        let filters = vec![serde_json::json!({
            "kinds": [1063], // NIP-94 file metadata
            "limit": 100,
        })];

        let rx = self.pool.subscribe(subscription_id, &filters).await?;
        Ok(rx)
    }

    /// Fetch events by ID
    pub async fn fetch_by_id(
        &self,
        subscription_id: &str,
        event_ids: &[String],
    ) -> Result<tokio::sync::mpsc::Receiver<Event>, RelayError> {
        if !self.is_connected().await {
            return Err(RelayError::NoRelays);
        }

        let filters = vec![serde_json::json!({
            "ids": event_ids,
            "limit": event_ids.len(),
        })];

        let rx = self.pool.subscribe(subscription_id, &filters).await?;
        Ok(rx)
    }

    /// Fetch events by author pubkey
    pub async fn fetch_by_author(
        &self,
        subscription_id: &str,
        pubkey: &str,
        kinds: &[u16],
    ) -> Result<tokio::sync::mpsc::Receiver<Event>, RelayError> {
        if !self.is_connected().await {
            return Err(RelayError::NoRelays);
        }

        let filters = vec![serde_json::json!({
            "authors": [pubkey],
            "kinds": kinds,
            "limit": 50,
        })];

        let rx = self.pool.subscribe(subscription_id, &filters).await?;
        Ok(rx)
    }

    /// Unsubscribe from a subscription
    pub async fn unsubscribe(&self, subscription_id: &str) -> Result<(), RelayError> {
        self.pool.unsubscribe(subscription_id).await?;
        Ok(())
    }

    /// Publish an event to all connected relays
    pub async fn publish(&self, event: &Event) -> Result<(), RelayError> {
        if !self.is_connected().await {
            return Err(RelayError::NoRelays);
        }

        self.pool.publish(event).await?;
        Ok(())
    }

    /// Get relay pool statistics
    pub async fn stats(&self) -> nostr_client::PoolStats {
        self.pool.pool_stats().await
    }
}

impl Default for MarketplaceRelay {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_new_marketplace_relay() {
        let relay = MarketplaceRelay::new();
        // Should create successfully
        assert!(!*relay.connected.try_read().unwrap());
    }

    #[test]
    fn test_default_marketplace_relays() {
        assert!(!DEFAULT_MARKETPLACE_RELAYS.is_empty());
        assert!(DEFAULT_MARKETPLACE_RELAYS.len() >= 3);
    }

    #[test]
    #[serial]
    fn test_get_marketplace_relays_default() {
        unsafe {
            std::env::remove_var("MARKETPLACE_RELAYS");
        }
        let relays = get_marketplace_relays();
        assert_eq!(relays.len(), DEFAULT_MARKETPLACE_RELAYS.len());
        assert_eq!(relays[0], "wss://relay.damus.io");
    }

    #[test]
    #[serial]
    fn test_get_marketplace_relays_from_env() {
        unsafe {
            std::env::set_var("MARKETPLACE_RELAYS", "wss://custom1.com,wss://custom2.com");
        }
        let relays = get_marketplace_relays();
        assert_eq!(relays.len(), 2);
        assert_eq!(relays[0], "wss://custom1.com");
        assert_eq!(relays[1], "wss://custom2.com");
        unsafe {
            std::env::remove_var("MARKETPLACE_RELAYS");
        }
    }

    #[test]
    #[serial]
    fn test_get_marketplace_relays_env_with_spaces() {
        unsafe {
            std::env::set_var(
                "MARKETPLACE_RELAYS",
                " wss://relay1.com , wss://relay2.com ",
            );
        }
        let relays = get_marketplace_relays();
        assert_eq!(relays.len(), 2);
        assert_eq!(relays[0], "wss://relay1.com");
        assert_eq!(relays[1], "wss://relay2.com");
        unsafe {
            std::env::remove_var("MARKETPLACE_RELAYS");
        }
    }

    #[test]
    #[serial]
    fn test_get_marketplace_relays_empty_env_uses_default() {
        unsafe {
            std::env::set_var("MARKETPLACE_RELAYS", "");
        }
        let relays = get_marketplace_relays();
        assert_eq!(relays.len(), DEFAULT_MARKETPLACE_RELAYS.len());
        unsafe {
            std::env::remove_var("MARKETPLACE_RELAYS");
        }
    }

    #[tokio::test]
    async fn test_connect_disconnect() {
        let relay = MarketplaceRelay::new();
        assert!(!relay.is_connected().await);

        // Note: Can't test actual connection without relay access
        // These would work in integration tests with real relays
    }

    #[tokio::test]
    async fn test_subscribe_handlers_when_disconnected() {
        let relay = MarketplaceRelay::new();
        let result = relay.subscribe_handlers("test-sub", None).await;
        assert!(result.is_err());
        match result {
            Err(RelayError::NoRelays) => {}
            _ => panic!("Expected NoRelays error"),
        }
    }
}
