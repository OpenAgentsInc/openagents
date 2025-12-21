//! Bifrost protocol for peer coordination via Nostr
//!
//! The Bifrost protocol enables threshold peers to coordinate signing
//! and ECDH operations via encrypted Nostr events.

pub mod messages;
pub mod node;
pub mod transport;

pub use messages::{BifrostMessage, SignRequest, SignResponse};
pub use node::BifrostNode;
