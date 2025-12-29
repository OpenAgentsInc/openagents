//! Relay pool for managing multiple relay connections
//!
//! Provides intelligent routing, automatic failover, and subscription management
//! across multiple Nostr relays.

use crate::error::{ClientError, Result};
use crate::outbox::OutboxModel;
use crate::relay::{PublishConfirmation, RelayConfig, RelayConnection};
use nostr::Event;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// Configuration for relay pool
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of concurrent relay connections
    pub max_relays: usize,
    /// Connection timeout for new relays
    pub connection_timeout: Duration,
    /// Minimum number of write confirmations to wait for
    pub min_write_confirmations: usize,
    /// Enable automatic reconnection
    pub auto_reconnect: bool,
    /// Relay configuration template
    pub relay_config: RelayConfig,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_relays: 10,
            connection_timeout: Duration::from_secs(10),
            min_write_confirmations: 2,
            auto_reconnect: true,
            relay_config: RelayConfig::default(),
        }
    }
}

/// Statistics for a single relay
#[derive(Debug, Clone)]
pub struct RelayStats {
    /// Relay URL
    pub url: String,
    /// Number of events received
    pub events_received: u64,
    /// Number of events published
    pub events_published: u64,
    /// Number of errors
    pub errors: u64,
    /// Average latency in milliseconds
    pub avg_latency_ms: u64,
    /// Last successful connection time
    pub last_connected: Option<Instant>,
    /// Is currently connected
    pub connected: bool,
}

/// Overall pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Number of connected relays
    pub connected_relays: usize,
    /// Total relays in pool
    pub total_relays: usize,
    /// Total events received across all relays
    pub total_events_received: u64,
    /// Total events published across all relays
    pub total_events_published: u64,
    /// Total errors across all relays
    pub total_errors: u64,
}

/// Relay pool for managing multiple connections
pub struct RelayPool {
    /// Connected relays (URL -> connection)
    relays: Arc<RwLock<HashMap<String, Arc<RelayConnection>>>>,
    /// Relay statistics
    stats: Arc<RwLock<HashMap<String, RelayStats>>>,
    /// Outbox model for intelligent routing
    outbox: Arc<RwLock<OutboxModel>>,
    /// Configuration
    config: PoolConfig,
    /// Active subscriptions (subscription_id -> relay URLs)
    subscriptions: Arc<RwLock<HashMap<String, HashSet<String>>>>,
}

impl RelayPool {
    /// Create a new relay pool
    pub fn new(config: PoolConfig) -> Self {
        Self {
            relays: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(HashMap::new())),
            outbox: Arc::new(RwLock::new(OutboxModel::new())),
            config,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a relay to the pool (doesn't connect yet)
    pub async fn add_relay(&self, url: &str) -> Result<()> {
        let relays = self.relays.read().await;

        // Check max relays limit
        if relays.len() >= self.config.max_relays {
            return Err(ClientError::Internal(format!(
                "Maximum relay limit ({}) reached",
                self.config.max_relays
            )));
        }

        // Check if already exists
        if relays.contains_key(url) {
            return Ok(());
        }

        drop(relays);

        // Create relay connection
        let relay = RelayConnection::with_config(url, self.config.relay_config.clone())?;

        // Add to pool
        let mut relays = self.relays.write().await;
        relays.insert(url.to_string(), Arc::new(relay));

        // Initialize stats
        let mut stats = self.stats.write().await;
        stats.insert(
            url.to_string(),
            RelayStats {
                url: url.to_string(),
                events_received: 0,
                events_published: 0,
                errors: 0,
                avg_latency_ms: 0,
                last_connected: None,
                connected: false,
            },
        );

        info!("Added relay to pool: {}", url);
        Ok(())
    }

    /// Remove a relay from the pool (disconnects if connected)
    pub async fn remove_relay(&self, url: &str) -> Result<()> {
        let mut relays = self.relays.write().await;

        if let Some(relay) = relays.remove(url) {
            // Disconnect
            if let Err(e) = relay.disconnect().await {
                warn!("Error disconnecting relay during removal: {}", e);
            }

            // Remove stats
            let mut stats = self.stats.write().await;
            stats.remove(url);

            info!("Removed relay from pool: {}", url);
        }

        Ok(())
    }

    /// Connect to all relays in the pool
    pub async fn connect_all(&self) -> Result<()> {
        let relays = self.relays.read().await;
        let mut handles = Vec::new();

        for (url, relay) in relays.iter() {
            let url = url.clone();
            let relay = Arc::clone(relay);
            let stats = Arc::clone(&self.stats);
            let timeout = self.config.connection_timeout;

            let handle = tokio::spawn(async move {
                match tokio::time::timeout(timeout, relay.connect()).await {
                    Ok(Ok(_)) => {
                        info!("Connected to relay: {}", url);

                        // Update stats
                        let mut stats = stats.write().await;
                        if let Some(stat) = stats.get_mut(&url) {
                            stat.connected = true;
                            stat.last_connected = Some(Instant::now());
                        }

                        Ok(url)
                    }
                    Ok(Err(e)) => {
                        warn!("Failed to connect to relay {}: {}", url, e);

                        // Update error count
                        let mut stats = stats.write().await;
                        if let Some(stat) = stats.get_mut(&url) {
                            stat.errors += 1;
                        }

                        Err(e)
                    }
                    Err(_) => {
                        warn!("Connection timeout for relay: {}", url);

                        // Update error count
                        let mut stats = stats.write().await;
                        if let Some(stat) = stats.get_mut(&url) {
                            stat.errors += 1;
                        }

                        Err(ClientError::Timeout(format!(
                            "Connection timeout for {}",
                            url
                        )))
                    }
                }
            });

            handles.push(handle);
        }

        // Wait for all connections
        let mut successful = 0;
        for handle in handles {
            if let Ok(Ok(_)) = handle.await {
                successful += 1;
            }
        }

        info!("Connected to {}/{} relays", successful, relays.len());

        if successful == 0 {
            return Err(ClientError::Connection(
                "Failed to connect to any relay".to_string(),
            ));
        }

        Ok(())
    }

    /// Disconnect from all relays
    pub async fn disconnect_all(&self) -> Result<()> {
        let relays = self.relays.read().await;
        let mut handles = Vec::new();

        for (url, relay) in relays.iter() {
            let url = url.clone();
            let relay = Arc::clone(relay);
            let stats = Arc::clone(&self.stats);

            let handle = tokio::spawn(async move {
                if let Err(e) = relay.disconnect().await {
                    warn!("Error disconnecting from {}: {}", url, e);
                }

                // Update stats
                let mut stats = stats.write().await;
                if let Some(stat) = stats.get_mut(&url) {
                    stat.connected = false;
                }
            });

            handles.push(handle);
        }

        // Wait for all disconnections
        for handle in handles {
            let _ = handle.await;
        }

        info!("Disconnected from all relays");
        Ok(())
    }

    /// Publish an event to appropriate relays using outbox model
    pub async fn publish(&self, event: &Event) -> Result<Vec<PublishConfirmation>> {
        let relays = self.relays.read().await;

        // Get publish relays for this event using outbox model
        let outbox = self.outbox.read().await;
        let target_relay_urls = outbox.get_publish_relays(event);

        // If no specific relays, use all connected relays
        let target_relays: Vec<_> = if target_relay_urls.is_empty() {
            relays.values().cloned().collect()
        } else {
            target_relay_urls
                .iter()
                .filter_map(|url| relays.get(url).cloned())
                .collect()
        };

        self.publish_to_relay_connections(event, target_relays).await
    }

    /// Publish an event to a specific relay URL list
    pub async fn publish_to_relays(
        &self,
        event: &Event,
        relay_urls: &[String],
    ) -> Result<Vec<PublishConfirmation>> {
        let relays = self.relays.read().await;
        let target_relays: Vec<_> = if relay_urls.is_empty() {
            relays.values().cloned().collect()
        } else {
            let mut selected: Vec<_> = relay_urls
                .iter()
                .filter_map(|url| relays.get(url).cloned())
                .collect();
            if selected.is_empty() {
                selected = relays.values().cloned().collect();
            }
            selected
        };

        self.publish_to_relay_connections(event, target_relays).await
    }

    async fn publish_to_relay_connections(
        &self,
        event: &Event,
        target_relays: Vec<Arc<RelayConnection>>,
    ) -> Result<Vec<PublishConfirmation>> {
        if target_relays.is_empty() {
            return Err(ClientError::NotConnected);
        }

        debug!(
            "Publishing event {} to {} relays",
            event.id,
            target_relays.len()
        );

        // Publish to all target relays concurrently
        let mut handles = Vec::new();
        let stats = Arc::clone(&self.stats);

        for relay in target_relays {
            let event = event.clone();
            let relay_url = relay.url().to_string();
            let stats = Arc::clone(&stats);

            let handle = tokio::spawn(async move {
                let start = Instant::now();
                let result = relay
                    .publish_event(&event, Duration::from_secs(5))
                    .await;

                // Update stats
                let latency = start.elapsed().as_millis() as u64;
                let mut stats = stats.write().await;
                if let Some(stat) = stats.get_mut(&relay_url) {
                    match &result {
                        Ok(_) => {
                            stat.events_published += 1;
                            // Update running average
                            if stat.avg_latency_ms == 0 {
                                stat.avg_latency_ms = latency;
                            } else {
                                stat.avg_latency_ms = (stat.avg_latency_ms + latency) / 2;
                            }
                        }
                        Err(_) => {
                            stat.errors += 1;
                        }
                    }
                }

                (relay_url, result)
            });

            handles.push(handle);
        }

        // Collect results
        let mut confirmations = Vec::new();
        let mut errors = Vec::new();

        for handle in handles {
            match handle.await {
                Ok((relay_url, Ok(confirmation))) => {
                    debug!("Event published to {}: {:?}", relay_url, confirmation);
                    confirmations.push(confirmation);
                }
                Ok((relay_url, Err(e))) => {
                    warn!("Failed to publish to {}: {}", relay_url, e);
                    errors.push(e);
                }
                Err(e) => {
                    warn!("Task error while publishing: {}", e);
                }
            }
        }

        // Check minimum confirmations
        if confirmations.len() < self.config.min_write_confirmations {
            warn!(
                "Only {} confirmations (minimum: {})",
                confirmations.len(),
                self.config.min_write_confirmations
            );

            // Return error if we didn't get enough confirmations
            if !errors.is_empty() {
                return Err(errors.into_iter().next().unwrap());
            } else {
                return Err(ClientError::PublishFailed(format!(
                    "Insufficient confirmations: got {}, required {}",
                    confirmations.len(),
                    self.config.min_write_confirmations
                )));
            }
        }

        Ok(confirmations)
    }

    /// Subscribe to events on all connected relays
    ///
    /// Returns a bounded receiver with a buffer of 1000 events per relay.
    /// This provides backpressure to prevent unbounded memory growth.
    pub async fn subscribe(
        &self,
        subscription_id: &str,
        filters: &[Value],
    ) -> Result<mpsc::Receiver<Event>> {
        let relays = self.relays.read().await;
        // Use bounded channel to prevent unbounded memory growth
        let (tx, rx) = mpsc::channel(1000);
        let mut subscription_relays = HashSet::new();

        for (url, relay) in relays.iter() {
            let relay = Arc::clone(relay);
            let filters = filters.to_vec();
            let sub_id = subscription_id.to_string();
            let sub_id_clone = sub_id.clone();
            let url = url.clone();
            let url_clone = url.clone();
            let tx = tx.clone();
            let stats = Arc::clone(&self.stats);

            // Subscribe to this relay
            match relay
                .subscribe_with_channel(&sub_id, &filters)
                .await
            {
                Ok(mut relay_rx) => {
                    subscription_relays.insert(url.clone());

                    // Spawn task to forward events
                    tokio::spawn(async move {
                        while let Some(event) = relay_rx.recv().await {
                            // Update stats
                            let mut stats = stats.write().await;
                            if let Some(stat) = stats.get_mut(&url_clone) {
                                stat.events_received += 1;
                            }
                            drop(stats);

                            // Forward event with backpressure
                            // This will wait if the channel is full, providing natural backpressure
                            if tx.send(event).await.is_err() {
                                debug!("Event receiver dropped for subscription {}", sub_id_clone);
                                break;
                            }
                        }
                    });

                    debug!("Subscribed to {} with ID {}", url, sub_id);
                }
                Err(e) => {
                    warn!("Failed to subscribe to {}: {}", url, e);
                }
            }
        }

        if subscription_relays.is_empty() {
            return Err(ClientError::Internal(
                "Failed to subscribe to any relay".to_string(),
            ));
        }

        // Track active subscription
        let sub_count = subscription_relays.len();
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.insert(subscription_id.to_string(), subscription_relays);

        info!(
            "Subscribed to {} relays with ID {}",
            sub_count,
            subscription_id
        );

        Ok(rx)
    }

    /// Unsubscribe from all relays
    pub async fn unsubscribe(&self, subscription_id: &str) -> Result<()> {
        let subscriptions = self.subscriptions.read().await;
        let relay_urls = match subscriptions.get(subscription_id) {
            Some(urls) => urls.clone(),
            None => return Ok(()), // Not subscribed
        };
        drop(subscriptions);

        let relays = self.relays.read().await;
        let mut handles = Vec::new();

        for url in relay_urls {
            if let Some(relay) = relays.get(&url) {
                let relay = Arc::clone(relay);
                let sub_id = subscription_id.to_string();

                let handle = tokio::spawn(async move {
                    if let Err(e) = relay.unsubscribe(&sub_id).await {
                        warn!("Error unsubscribing from {}: {}", url, e);
                    }
                });

                handles.push(handle);
            }
        }

        // Wait for all unsubscribes
        for handle in handles {
            let _ = handle.await;
        }

        // Remove from tracking
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.remove(subscription_id);

        info!("Unsubscribed from all relays: {}", subscription_id);
        Ok(())
    }

    /// Get statistics for all relays
    pub async fn relay_stats(&self) -> Vec<RelayStats> {
        let stats = self.stats.read().await;
        stats.values().cloned().collect()
    }

    /// Get overall pool statistics
    pub async fn pool_stats(&self) -> PoolStats {
        let relays = self.relays.read().await;
        let stats = self.stats.read().await;

        let connected_relays = stats.values().filter(|s| s.connected).count();
        let total_events_received = stats.values().map(|s| s.events_received).sum();
        let total_events_published = stats.values().map(|s| s.events_published).sum();
        let total_errors = stats.values().map(|s| s.errors).sum();

        PoolStats {
            connected_relays,
            total_relays: relays.len(),
            total_events_received,
            total_events_published,
            total_errors,
        }
    }

    /// Get list of connected relay URLs
    pub async fn connected_relays(&self) -> Vec<String> {
        let stats = self.stats.read().await;
        stats
            .values()
            .filter(|s| s.connected)
            .map(|s| s.url.clone())
            .collect()
    }

    /// Check if connected to any relay
    pub async fn is_connected(&self) -> bool {
        let stats = self.stats.read().await;
        stats.values().any(|s| s.connected)
    }

    /// Update outbox model with a relay list event (NIP-65)
    pub async fn update_relay_list(&self, event: &Event) -> Result<()> {
        let outbox = self.outbox.read().await;
        outbox.update_relay_list(&event.pubkey, event)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pool_creation() {
        let pool = RelayPool::new(PoolConfig::default());
        assert!(!pool.is_connected().await);

        let stats = pool.pool_stats().await;
        assert_eq!(stats.total_relays, 0);
        assert_eq!(stats.connected_relays, 0);
    }

    #[tokio::test]
    async fn test_add_relay() {
        let pool = RelayPool::new(PoolConfig::default());

        pool.add_relay("wss://relay1.example.com").await.unwrap();
        pool.add_relay("wss://relay2.example.com").await.unwrap();

        let stats = pool.pool_stats().await;
        assert_eq!(stats.total_relays, 2);
    }

    #[tokio::test]
    async fn test_add_relay_max_limit() {
        let config = PoolConfig {
            max_relays: 2,
            ..Default::default()
        };
        let pool = RelayPool::new(config);

        pool.add_relay("wss://relay1.example.com").await.unwrap();
        pool.add_relay("wss://relay2.example.com").await.unwrap();

        let result = pool.add_relay("wss://relay3.example.com").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_add_duplicate_relay() {
        let pool = RelayPool::new(PoolConfig::default());

        pool.add_relay("wss://relay1.example.com").await.unwrap();
        pool.add_relay("wss://relay1.example.com").await.unwrap();

        let stats = pool.pool_stats().await;
        assert_eq!(stats.total_relays, 1);
    }

    #[tokio::test]
    async fn test_remove_relay() {
        let pool = RelayPool::new(PoolConfig::default());

        pool.add_relay("wss://relay1.example.com").await.unwrap();
        assert_eq!(pool.pool_stats().await.total_relays, 1);

        pool.remove_relay("wss://relay1.example.com").await.unwrap();
        assert_eq!(pool.pool_stats().await.total_relays, 0);
    }

    #[tokio::test]
    async fn test_relay_stats() {
        let pool = RelayPool::new(PoolConfig::default());

        pool.add_relay("wss://relay1.example.com").await.unwrap();
        pool.add_relay("wss://relay2.example.com").await.unwrap();

        let stats = pool.relay_stats().await;
        assert_eq!(stats.len(), 2);

        for stat in stats {
            assert_eq!(stat.events_received, 0);
            assert_eq!(stat.events_published, 0);
            assert_eq!(stat.errors, 0);
            assert!(!stat.connected);
        }
    }
}
