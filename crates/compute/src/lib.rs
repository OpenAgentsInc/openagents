//! Compute Provider - Sell compute via NIP-90 DVMs with Bitcoin payments
//!
//! This crate provides a desktop application that allows users to:
//! - Generate or import a BIP32 seed phrase
//! - Derive both Nostr identity (NIP-06) and Spark wallet from the same seed
//! - Go online to receive NIP-90 job requests
//! - Execute inference via Ollama
//! - Receive Bitcoin payments via Lightning/Spark

pub mod domain;
pub mod services;
pub mod storage;
