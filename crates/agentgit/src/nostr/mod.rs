//! Nostr integration for AgentGit
//!
//! Connects to Nostr relays to monitor and publish NIP-34 git events.

pub mod client;

pub use client::NostrClient;
