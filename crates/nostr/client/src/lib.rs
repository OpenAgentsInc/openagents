//! Nostr client implementation
//!
//! This crate provides a Nostr protocol client with:
//! - WebSocket connections to relays
//! - Event publishing and subscription
//! - Automatic reconnection
//! - Connection health monitoring
//! - Outbox model for intelligent relay selection (NIP-65)
//! - Local event caching
//!
//! # Example
//!
//! ```no_run
//! use nostr_client::{RelayConnection, RelayMessage};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let relay = RelayConnection::new("wss://relay.example.com")?;
//!     relay.connect().await?;
//!
//!     // Subscribe to events
//!     let filters = vec![serde_json::json!({"kinds": [1]})];
//!     relay.subscribe("my-sub", &filters).await?;
//!
//!     // Receive messages
//!     while let Ok(Some(msg)) = relay.recv().await {
//!         match msg {
//!             RelayMessage::Event(sub_id, event) => {
//!                 println!("Received event: {:?}", event);
//!             }
//!             RelayMessage::Eose(sub_id) => {
//!                 println!("End of stored events for: {}", sub_id);
//!                 break;
//!             }
//!             _ => {}
//!         }
//!     }
//!
//!     relay.disconnect().await?;
//!     Ok(())
//! }
//! ```

mod cache;
mod connection_pool;
mod contacts;
pub mod dvm;
mod error;
mod negentropy;
mod outbox;
mod pool;
mod queue;
mod recovery;
mod relay;
mod subscription;

pub use cache::{CacheConfig, EventCache};
pub use connection_pool::{
    ConnectionPoolConfig, ConnectionPoolManager, PoolStats as ConnectionPoolStats,
};
pub use contacts::{ContactManager, MergeStrategy};
pub use error::{ClientError, Result};
pub use negentropy::{SyncSession, SyncSessionId, SyncSessionManager, build_initial_message};
pub use outbox::{OutboxConfig, OutboxModel};
pub use pool::{PoolConfig, PoolStats, RelayPool, RelayStats};
pub use queue::{MessageQueue, MessageStatus, QueueConfig, QueuedMessage};
pub use recovery::{CircuitBreaker, CircuitState, ExponentialBackoff, HealthMetrics};
pub use relay::{ConnectionState, PublishConfirmation, RelayConfig, RelayConnection, RelayMessage};
pub use subscription::{EventCallback, Subscription};
