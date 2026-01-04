//! Compute Provider - Sell compute via NIP-90 DVMs with Bitcoin payments
//!
//! This crate provides the infrastructure for running a NIP-90 compute provider:
//! - Multiple inference backends (Ollama, Apple FM, Llama.cpp)
//! - Auto-detection of available backends
//! - NIP-90 job request/result loop
//! - Identity management (BIP39 → Nostr + Lightning)
//! - FRLM tool integration for recursive LLM calls

pub mod backends;
pub mod domain;
pub mod frlm_tool_handler;
pub mod frlm_tools;
pub mod services;
pub mod storage;

// Re-exports for FRLM tools
pub use frlm_tool_handler::FrlmToolHandler;
pub use frlm_tools::{create_frlm_tools, frlm_tool_names};

// UI modules disabled until wgpui dependency is added
// #[cfg(feature = "ui")]
// pub mod app;
// #[cfg(feature = "ui")]
// pub mod state;
// #[cfg(feature = "ui")]
// pub mod ui;
// #[cfg(feature = "ui")]
// pub use app::ComputeApp;
