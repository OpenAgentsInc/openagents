//! Bifrost Protocol - Threshold Peer Coordination via Nostr
//!
//! The Bifrost protocol enables k-of-n threshold signature peers to coordinate
//! signing and ECDH operations using encrypted Nostr events as the transport layer.
//!
//! # Overview
//!
//! Bifrost provides a decentralized coordination layer for FROST threshold signatures,
//! allowing peers to:
//!
//! - Coordinate multi-round signing protocols without a central coordinator
//! - Perform threshold ECDH for encrypted communication
//! - Discover and verify threshold peers via Nostr
//! - Handle failures gracefully with automatic retry and timeout mechanisms
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      Application                             │
//! │              (Wallet, Agent, Service)                        │
//! └────────────────────┬────────────────────────────────────────┘
//!                      │
//!           ┌──────────▼─────────────┐
//!           │    BifrostNode        │  ← High-level API
//!           │  (sign, ecdh, ping)   │
//!           └──────────┬─────────────┘
//!                      │
//!      ┌───────────────┼───────────────┐
//!      │               │               │
//!      ▼               ▼               ▼
//! ┌─────────┐   ┌─────────────┐   ┌──────────┐
//! │  Peer   │   │ Aggregator  │   │Transport │
//! │ Manager │   │ (Sign/ECDH) │   │ (Nostr)  │
//! └─────────┘   └─────────────┘   └──────────┘
//! ```
//!
//! # Protocol Flow
//!
//! ## Signing Round (2-of-3 example)
//!
//! ```text
//! Peer 1 (Initiator)          Peer 2               Peer 3
//!     │                          │                    │
//!     ├─SignRequest──────────────┼───────────────────>│
//!     │  (event hash + nonce)    │                    │
//!     │                          │                    │
//!     │<──────────SignResponse───┼────────────────────┤
//!     │   (partial sig + nonce)  │                    │
//!     │                          │                    │
//!     ├─SignReturn───────────────┼───────────────────>│
//!     │  (final signature)       │                    │
//! ```
//!
//! ## ECDH Round
//!
//! ```text
//! Peer 1 (Initiator)          Peer 2               Peer 3
//!     │                          │                    │
//!     ├─EcdhRequest──────────────┼───────────────────>│
//!     │  (target pubkey)         │                    │
//!     │                          │                    │
//!     │<──────────EcdhResponse───┼────────────────────┤
//!     │   (partial ECDH)         │                    │
//!     │                          │                    │
//!     └─ Aggregate → Shared Secret
//! ```
//!
//! # Usage
//!
//! ```rust,no_run
//! use frostr::bifrost::BifrostNode;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Create and start node
//! let mut node = BifrostNode::new()?;
//! node.start().await?;
//!
//! // Sign an event hash
//! let event_hash = [0u8; 32];
//! let signature = node.sign(&event_hash).await?;
//! println!("Signature: {}", hex::encode(signature));
//!
//! // Perform ECDH
//! let peer_pubkey = [0u8; 32];
//! let shared_secret = node.ecdh(&peer_pubkey).await?;
//! println!("Shared secret: {}", hex::encode(shared_secret));
//!
//! // Shutdown
//! node.stop().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Message Types
//!
//! | Message | Kind | Direction | Purpose |
//! |---------|------|-----------|---------|
//! | [`SignRequest`] | `/sign/req` | Initiator → Peers | Start signing round |
//! | [`SignResponse`] | `/sign/res` | Peers → Initiator | Provide partial signature |
//! | [`SignResult`] | `/sign/ret` | Initiator → Peers | Broadcast final signature |
//! | [`SignError`] | `/sign/err` | Any → Any | Report signing failure |
//! | [`EcdhRequest`] | `/ecdh/req` | Initiator → Peers | Start ECDH round |
//! | [`EcdhResponse`] | `/ecdh/res` | Peers → Initiator | Provide partial ECDH |
//!
//! # Security Considerations
//!
//! - **Nonce Commitments**: Prevents signature malleability attacks
//! - **Peer Verification**: All messages are signed and verified
//! - **Timeout Enforcement**: Prevents indefinite blocking (default 30s)
//! - **Rate Limiting**: Protects against DoS attacks
//! - **Audit Logging**: All operations logged for forensics
//!
//! # Error Handling
//!
//! Operations can fail due to:
//!
//! - **Network failures**: Relay disconnections, timeouts
//! - **Cryptographic errors**: Invalid signatures, malformed data
//! - **Threshold failures**: Insufficient peers responding
//! - **Protocol violations**: Invalid message sequences
//!
//! All errors are wrapped in [`anyhow::Error`] with context.
//!
//! # Performance
//!
//! Typical operation latencies (on public relays):
//!
//! - Sign operation: 2-5 seconds (2-of-3)
//! - ECDH operation: 1-3 seconds (2-of-3)
//! - Peer discovery: 1-2 seconds
//!
//! # Testing
//!
//! ```bash
//! # Run all bifrost tests
//! cargo test -p frostr --features bifrost bifrost
//!
//! # Run integration tests
//! cargo test -p frostr --features bifrost --test bifrost_integration
//! ```

pub mod aggregator;
pub mod messages;
pub mod node;
pub mod peer;
pub mod serialization;
pub mod transport;

pub use aggregator::EcdhAggregator;
pub use messages::{
    BifrostMessage,
    // FROST two-phase signing protocol (RFC 9591)
    CommitmentRequest,
    CommitmentResponse,
    EcdhRequest,
    EcdhResponse,
    PartialSignature,
    ParticipantCommitment,
    Ping,
    Pong,
    SignError,
    SignResult,
    SigningPackageMessage,
};
pub use node::{BifrostConfig, BifrostNode, RetryConfig as NodeRetryConfig, TimeoutConfig};
pub use peer::{PeerInfo, PeerManager, PeerStatus, RetryConfig as PeerRetryConfig};
pub use serialization::{
    COMMITMENT_SIZE, CommitmentBundle, IDENTIFIER_SIZE, SIG_SHARE_SIZE, SignatureBundle,
    deserialize_commitments, deserialize_identifier, deserialize_sig_share, serialize_commitments,
    serialize_identifier, serialize_sig_share,
};
pub use transport::{BIFROST_EVENT_KIND, NostrTransport, TransportConfig};
