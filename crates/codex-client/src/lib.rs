//! Codex app-server client library.
//!
//! Provides a JSON-RPC client for communicating with the Codex app-server
//! via stdin/stdout.

mod client;
mod types;

pub use client::*;
pub use types::*;
