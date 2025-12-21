//! Nostr integration for AgentGit
//!
//! Connects to Nostr relays to monitor and publish NIP-34 git events.

pub mod cache;
pub mod client;
pub mod events;

pub use client::NostrClient;
