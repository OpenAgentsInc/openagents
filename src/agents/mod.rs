//! Agent communication module for NIP-28 based agent-to-agent messaging
//!
//! This module provides shared functionality for provider and customer agents
//! to communicate via Nostr NIP-28 public chat channels.

pub mod protocol;
pub mod runner;

pub use protocol::*;
pub use runner::{ComputeClient, Scheduler, StateManager, TickExecutor, TickResult, TickTrigger};
