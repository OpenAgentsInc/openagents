//! Bifrost protocol for peer coordination via Nostr
//!
//! The Bifrost protocol enables threshold peers to coordinate signing
//! and ECDH operations via encrypted Nostr events.

pub mod messages;
pub mod node;
pub mod peer;
pub mod transport;

pub use messages::{
    BifrostMessage, EcdhRequest, EcdhResponse, SignError, SignRequest, SignResponse, SignResult,
};
pub use node::{BifrostConfig, BifrostNode, RetryConfig as NodeRetryConfig, TimeoutConfig};
pub use peer::{PeerInfo, PeerManager, PeerStatus, RetryConfig as PeerRetryConfig};
pub use transport::{NostrTransport, TransportConfig, BIFROST_EVENT_KIND};
