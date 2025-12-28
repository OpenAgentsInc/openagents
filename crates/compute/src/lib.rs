//! Compute Provider - Sell compute via NIP-90 DVMs with Bitcoin payments
//!
//! This crate provides the infrastructure for running a NIP-90 compute provider:
//! - Multiple inference backends (Ollama, Apple FM, Llama.cpp)
//! - Auto-detection of available backends
//! - NIP-90 job request/result loop
//! - Identity management (BIP39 → Nostr + Lightning)

pub mod backends;
pub mod domain;
pub mod services;
pub mod storage;

// UI modules disabled until wgpui dependency is added
// #[cfg(feature = "ui")]
// pub mod app;
// #[cfg(feature = "ui")]
// pub mod state;
// #[cfg(feature = "ui")]
// pub mod ui;
// #[cfg(feature = "ui")]
// pub use app::ComputeApp;
