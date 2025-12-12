//! Relay pool for managing connections to multiple Nostr relays.
//!
//! This module provides a pool that can connect to multiple relays simultaneously,
//! aggregate events, and broadcast to all connected relays.

use crate::connection::{ConnectionConfig, ConnectionError, ConnectionState, RelayConnection};
use crate::message::{Filter, RelayMessage};
use crate::subscription::{generate_subscription_id, SubscriptionTracker};
use nostr::Event;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn};

/// Events emitted by the relay pool.
#[derive(Debug, Clone)]
pub enum PoolEvent {
    /// A relay connected
    Connected { relay_url: String },
    /// A relay disconnected
    Disconnected { relay_url: String },
    /// An event was received from a relay
    Event {
        relay_url: String,
        subscription_id: String,
        event: Event,
    },
    /// EOSE received for a subscription on a relay
    Eose {
        relay_url: String,
        subscription_id: String,
    },
    /// All relays have sent EOSE for a subscription
    AllEose { subscription_id: String },
    /// OK response for a published event
    Ok {
        relay_url: String,
        event_id: String,
        success: bool,
        message: String,
    },
    /// Notice from a relay
    Notice { relay_url: String, message: String },
    /// Connection error
    Error { relay_url: String, error: String },
}

/// A pool of Nostr relay connections.
pub struct RelayPool {
    /// Connections indexed by URL
    connections: Arc<RwLock<HashMap<String, RelayConnection>>>,
    /// Subscription trackers indexed by subscription ID
    subscriptions: Arc<RwLock<HashMap<String, SubscriptionTracker>>>,
    /// Broadcast channel for pool events
    events_tx: broadcast::Sender<PoolEvent>,
    /// Default relays to connect to
    default_relays: Vec<String>,
}

impl RelayPool {
    /// Create a new relay pool.
    pub fn new() -> Self {
        let (events_tx, _) = broadcast::channel(1000);
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            events_tx,
            default_relays: Vec::new(),
        }
    }

    /// Create a new relay pool with default relays.
    pub fn with_relays(relays: Vec<String>) -> Self {
        let mut pool = Self::new();
        pool.default_relays = relays;
        pool
    }

    /// Subscribe to pool events.
    pub fn subscribe(&self) -> broadcast::Receiver<PoolEvent> {
        self.events_tx.subscribe()
    }

    /// Get all relay URLs in the pool.
    pub async fn relay_urls(&self) -> Vec<String> {
        self.connections.read().await.keys().cloned().collect()
    }

    /// Get connection states for all relays.
    pub async fn states(&self) -> HashMap<String, ConnectionState> {
        let conns = self.connections.read().await;
        let mut states = HashMap::new();
        for (url, conn) in conns.iter() {
            states.insert(url.clone(), conn.state().await);
        }
        states
    }

    /// Check if a relay is connected.
    pub async fn is_connected(&self, url: &str) -> bool {
        if let Some(conn) = self.connections.read().await.get(url) {
            conn.is_connected().await
        } else {
            false
        }
    }

    /// Add a relay to the pool.
    pub async fn add_relay(&self, url: impl Into<String>) -> Result<(), ConnectionError> {
        let url = url.into();
        info!("Adding relay to pool: {}", url);

        let config = ConnectionConfig::new(&url);
        let conn = RelayConnection::new(config);

        {
            let mut conns = self.connections.write().await;
            conns.insert(url.clone(), conn);
        }

        Ok(())
    }

    /// Remove a relay from the pool.
    pub async fn remove_relay(&self, url: &str) {
        info!("Removing relay from pool: {}", url);

        let mut conn = {
            let mut conns = self.connections.write().await;
            conns.remove(url)
        };

        if let Some(ref mut c) = conn {
            c.disconnect().await;
        }

        // Update subscription trackers
        let mut subs = self.subscriptions.write().await;
        for tracker in subs.values_mut() {
            tracker.remove_relay(url);
        }
    }

    /// Connect to a specific relay.
    pub async fn connect_relay(&self, url: &str) -> Result<(), ConnectionError> {
        debug!("Connecting to relay: {}", url);

        let mut conn = {
            let mut conns = self.connections.write().await;
            if let Some(c) = conns.remove(url) {
                c
            } else {
                // Create new connection if not exists
                let config = ConnectionConfig::new(url);
                RelayConnection::new(config)
            }
        };

        conn.connect().await?;

        // Spawn task to forward relay messages to pool events
        self.spawn_message_forwarder(&conn, url.to_string());

        // Resubscribe to active subscriptions
        {
            let subs = self.subscriptions.read().await;
            for tracker in subs.values() {
                if let Err(e) = conn.subscribe(&tracker.id, tracker.filters.clone()).await {
                    warn!(
                        "Failed to resubscribe {} on {}: {}",
                        tracker.id, url, e
                    );
                }
            }
        }

        // Store connection
        {
            let mut conns = self.connections.write().await;
            conns.insert(url.to_string(), conn);
        }

        // Emit connected event
        let _ = self.events_tx.send(PoolEvent::Connected {
            relay_url: url.to_string(),
        });

        Ok(())
    }

    /// Spawn a task to forward relay messages to pool events.
    fn spawn_message_forwarder(&self, conn: &RelayConnection, relay_url: String) {
        let mut rx = conn.subscribe_messages();
        let events_tx = self.events_tx.clone();
        let subscriptions = self.subscriptions.clone();

        tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                let pool_event = match msg {
                    RelayMessage::Event {
                        subscription_id,
                        event,
                    } => PoolEvent::Event {
                        relay_url: relay_url.clone(),
                        subscription_id,
                        event,
                    },
                    RelayMessage::Eose { subscription_id } => {
                        // Update subscription tracker
                        let all_eose = {
                            let mut subs = subscriptions.write().await;
                            if let Some(tracker) = subs.get_mut(&subscription_id) {
                                tracker.mark_eose(&relay_url);
                                tracker.all_eose
                            } else {
                                false
                            }
                        };

                        // Emit EOSE event
                        let _ = events_tx.send(PoolEvent::Eose {
                            relay_url: relay_url.clone(),
                            subscription_id: subscription_id.clone(),
                        });

                        // Emit AllEose if all relays have sent EOSE
                        if all_eose {
                            PoolEvent::AllEose {
                                subscription_id,
                            }
                        } else {
                            continue;
                        }
                    }
                    RelayMessage::Ok {
                        event_id,
                        success,
                        message,
                    } => PoolEvent::Ok {
                        relay_url: relay_url.clone(),
                        event_id,
                        success,
                        message,
                    },
                    RelayMessage::Notice { message } => PoolEvent::Notice {
                        relay_url: relay_url.clone(),
                        message,
                    },
                    RelayMessage::Closed {
                        subscription_id,
                        message,
                    } => {
                        warn!(
                            "Subscription {} closed by {}: {}",
                            subscription_id, relay_url, message
                        );
                        continue;
                    }
                    _ => continue,
                };

                let _ = events_tx.send(pool_event);
            }

            // Connection closed
            let _ = events_tx.send(PoolEvent::Disconnected {
                relay_url: relay_url.clone(),
            });
        });
    }

    /// Connect to all relays in the pool.
    pub async fn connect_all(&self) -> Vec<(String, Result<(), ConnectionError>)> {
        let urls: Vec<String> = self.connections.read().await.keys().cloned().collect();

        let mut results = Vec::new();
        for url in urls {
            let result = self.connect_relay(&url).await;
            results.push((url, result));
        }
        results
    }

    /// Connect to default relays.
    pub async fn connect_default(&self) -> Vec<(String, Result<(), ConnectionError>)> {
        let relays = self.default_relays.clone();

        // Add relays to pool
        for url in &relays {
            let _ = self.add_relay(url).await;
        }

        // Connect to all
        self.connect_all().await
    }

    /// Disconnect from all relays.
    pub async fn disconnect_all(&self) {
        let mut conns = self.connections.write().await;
        for (_, conn) in conns.iter_mut() {
            conn.disconnect().await;
        }
    }

    /// Publish an event to all connected relays.
    pub async fn publish(&self, event: Event) -> Vec<(String, Result<(), ConnectionError>)> {
        let conns = self.connections.read().await;
        let mut results = Vec::new();

        for (url, conn) in conns.iter() {
            if conn.is_connected().await {
                let result = conn.publish(event.clone()).await;
                results.push((url.clone(), result));
            }
        }

        results
    }

    /// Publish an event to specific relays.
    pub async fn publish_to(
        &self,
        event: Event,
        relay_urls: &[String],
    ) -> Vec<(String, Result<(), ConnectionError>)> {
        let conns = self.connections.read().await;
        let mut results = Vec::new();

        for url in relay_urls {
            if let Some(conn) = conns.get(url) {
                if conn.is_connected().await {
                    let result = conn.publish(event.clone()).await;
                    results.push((url.clone(), result));
                }
            }
        }

        results
    }

    /// Subscribe to events on all connected relays.
    pub async fn subscribe_all(
        &self,
        filters: Vec<Filter>,
    ) -> Result<String, ConnectionError> {
        let subscription_id = generate_subscription_id();
        self.subscribe_with_id(&subscription_id, filters).await?;
        Ok(subscription_id)
    }

    /// Subscribe with a specific subscription ID.
    pub async fn subscribe_with_id(
        &self,
        subscription_id: &str,
        filters: Vec<Filter>,
    ) -> Result<(), ConnectionError> {
        info!(
            "Creating subscription {} with {} filters",
            subscription_id,
            filters.len()
        );

        // Create tracker
        let mut tracker = SubscriptionTracker::new(subscription_id, filters.clone());

        // Subscribe on all connected relays
        let conns = self.connections.read().await;
        for (url, conn) in conns.iter() {
            if conn.is_connected().await {
                match conn.subscribe(subscription_id, filters.clone()).await {
                    Ok(()) => {
                        tracker.add_relay(url);
                    }
                    Err(e) => {
                        warn!("Failed to subscribe on {}: {}", url, e);
                    }
                }
            }
        }

        // Store tracker
        {
            let mut subs = self.subscriptions.write().await;
            subs.insert(subscription_id.to_string(), tracker);
        }

        Ok(())
    }

    /// Unsubscribe from events on all relays.
    pub async fn unsubscribe(&self, subscription_id: &str) -> Result<(), ConnectionError> {
        info!("Closing subscription {}", subscription_id);

        // Remove tracker
        {
            let mut subs = self.subscriptions.write().await;
            subs.remove(subscription_id);
        }

        // Unsubscribe on all relays
        let conns = self.connections.read().await;
        for (url, conn) in conns.iter() {
            if conn.is_connected().await {
                if let Err(e) = conn.unsubscribe(subscription_id).await {
                    warn!("Failed to unsubscribe {} on {}: {}", subscription_id, url, e);
                }
            }
        }

        Ok(())
    }

    /// Get active subscription IDs.
    pub async fn subscription_ids(&self) -> Vec<String> {
        self.subscriptions.read().await.keys().cloned().collect()
    }

    /// Get the number of connected relays.
    pub async fn connected_count(&self) -> usize {
        let conns = self.connections.read().await;
        let mut count = 0;
        for conn in conns.values() {
            if conn.is_connected().await {
                count += 1;
            }
        }
        count
    }
}

impl Default for RelayPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_new() {
        let pool = RelayPool::new();
        assert!(pool.default_relays.is_empty());
    }

    #[test]
    fn test_pool_with_relays() {
        let pool = RelayPool::with_relays(vec![
            "wss://relay1.com".to_string(),
            "wss://relay2.com".to_string(),
        ]);
        assert_eq!(pool.default_relays.len(), 2);
    }

    #[tokio::test]
    async fn test_pool_add_relay() {
        let pool = RelayPool::new();
        pool.add_relay("wss://relay.example.com").await.unwrap();

        let urls = pool.relay_urls().await;
        assert_eq!(urls.len(), 1);
        assert!(urls.contains(&"wss://relay.example.com".to_string()));
    }

    #[tokio::test]
    async fn test_pool_remove_relay() {
        let pool = RelayPool::new();
        pool.add_relay("wss://relay1.com").await.unwrap();
        pool.add_relay("wss://relay2.com").await.unwrap();

        pool.remove_relay("wss://relay1.com").await;

        let urls = pool.relay_urls().await;
        assert_eq!(urls.len(), 1);
        assert!(!urls.contains(&"wss://relay1.com".to_string()));
        assert!(urls.contains(&"wss://relay2.com".to_string()));
    }

    #[tokio::test]
    async fn test_pool_subscription_ids_empty() {
        let pool = RelayPool::new();
        let ids = pool.subscription_ids().await;
        assert!(ids.is_empty());
    }

    #[tokio::test]
    async fn test_pool_connected_count_none() {
        let pool = RelayPool::new();
        pool.add_relay("wss://relay.example.com").await.unwrap();

        // Not connected yet
        assert_eq!(pool.connected_count().await, 0);
    }

    // Note: Integration tests requiring actual relay connections would be in separate test file
}
