//! Nostr client implementation
//!
//! This crate provides a Nostr protocol client with:
//! - WebSocket connections to relays
//! - Event publishing and subscription
//! - Automatic reconnection
//! - Connection health monitoring
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

mod error;
mod relay;

pub use error::{ClientError, Result};
pub use relay::{ConnectionState, RelayConfig, RelayConnection, RelayMessage};
