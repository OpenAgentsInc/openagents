//! Agent communication module for NIP-90 compute marketplace
//!
//! This module implements the NIP-90 Data Vending Machine protocol for compute jobs.
//! The primary flow uses direct Nostr events. NIP-28 channels are optional for coordination.
//!
//! # Primary Flow (Direct NIP-90 Events)
//!
//! ```text
//! Customer                    Relay                       Provider
//!    |                          |                            |
//!    | -- kind:5050 request --> | --> kind:5050 request ---> |
//!    |                          |                            |
//!    | <-- kind:7000 feedback --|<--- kind:7000 + invoice ---|
//!    |                          |                            |
//!    | [pay Lightning invoice]  |                            |
//!    |                          |                            |
//!    | <-- kind:6050 result ----|<--- kind:6050 result ------|
//! ```
//!
//! # Optional: NIP-28 Channel Coordination
//!
//! For multi-party scenarios or real-time discussion, agents can use NIP-28 channels.
//! This is NOT required for the core compute flow.

pub mod protocol;
pub mod relay;
pub mod runner;

pub use protocol::*;
pub use relay::{RelayApi, RelayHub, SharedRelay};
pub use runner::{
    ComputeClient, Scheduler, StateManager, TickExecutor, TickResult, TickTrigger,
    TrajectoryPublisher,
};
