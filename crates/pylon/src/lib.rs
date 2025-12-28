//! Pylon - NIP-90 compute provider node
//!
//! Earn Bitcoin by running local AI inference over Nostr.
//!
//! Pylon supports multiple inference backends:
//! - Ollama (localhost:11434) - any platform
//! - Llama.cpp/GPT-OSS (localhost:8080) - any platform
//! - Apple Foundation Models (localhost:11435) - macOS Apple Silicon only

pub mod cli;
pub mod config;
pub mod provider;

pub use config::PylonConfig;
pub use provider::PylonProvider;
