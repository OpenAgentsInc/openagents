//! Minimal Nostr relay/DVM client transport.
//!
//! This crate intentionally exposes a small surface:
//! - relay publish/subscribe over WebSocket
//! - multi-relay pool fanout
//! - lightweight DVM helpers on top of NIP-90 kinds

pub mod dvm;
pub mod error;
pub mod pool;
pub mod relay;
pub mod subscription;

pub use dvm::DvmClient;
pub use error::{ClientError, Result};
pub use pool::{PoolConfig, RelayPool};
pub use relay::{ConnectionState, PublishConfirmation, RelayConfig, RelayConnection, RelayMessage};
pub use subscription::{EventCallback, Subscription};
