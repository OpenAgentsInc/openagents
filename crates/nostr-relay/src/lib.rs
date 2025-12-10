//! Nostr relay WebSocket client for OpenAgents.
//!
//! This crate provides:
//! - WebSocket connections to Nostr relays
//! - Message parsing (NIP-01 relay protocol)
//! - Subscription management with filters
//! - Connection pooling for multiple relays
//!
//! # Example
//!
//! ```rust,no_run
//! use nostr_relay::{RelayPool, Filter, PoolEvent};
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create a relay pool
//!     let pool = RelayPool::with_relays(vec![
//!         "wss://relay.damus.io".to_string(),
//!         "wss://nos.lol".to_string(),
//!     ]);
//!
//!     // Subscribe to pool events
//!     let mut events = pool.subscribe();
//!
//!     // Connect to relays
//!     pool.connect_default().await;
//!
//!     // Subscribe to kind 1 (text notes)
//!     let filter = Filter::new().kinds(vec![1]).limit(10);
//!     let sub_id = pool.subscribe_all(vec![filter]).await.unwrap();
//!
//!     // Process events
//!     while let Ok(event) = events.recv().await {
//!         match event {
//!             PoolEvent::Event { event, .. } => {
//!                 println!("Received event: {}", event.id);
//!             }
//!             PoolEvent::AllEose { subscription_id } => {
//!                 println!("Got all stored events for {}", subscription_id);
//!             }
//!             _ => {}
//!         }
//!     }
//! }
//! ```

mod connection;
mod message;
mod pool;
mod subscription;

// Re-export main types
pub use connection::{
    ConnectionConfig, ConnectionError, ConnectionState, RelayConnection, Subscription,
};
pub use message::{ClientMessage, Filter, MessageError, RelayMessage};
pub use pool::{PoolEvent, RelayPool};
pub use subscription::{generate_subscription_id, SubscriptionBuilder, SubscriptionTracker};

/// Default relays for OpenAgents.
pub const DEFAULT_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.nostr.band",
    "wss://nostr.wine",
];

/// Create a relay pool with default OpenAgents relays.
pub fn default_pool() -> RelayPool {
    RelayPool::with_relays(DEFAULT_RELAYS.iter().map(|s| s.to_string()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_relays() {
        assert!(!DEFAULT_RELAYS.is_empty());
        for relay in DEFAULT_RELAYS {
            assert!(relay.starts_with("wss://"));
        }
    }

    #[tokio::test]
    async fn test_default_pool() {
        let pool = default_pool();
        // Pool should be created successfully
        // Relays are added via connect_default, not stored directly
        assert_eq!(pool.relay_urls().await.len(), 0);
    }
}
