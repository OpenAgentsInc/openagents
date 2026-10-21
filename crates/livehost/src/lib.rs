//! livehost: a prototype of web deploys in seconds.
//!
//! A stable host serves an immutable, content-addressed manifest. A deploy
//! uploads only the objects that changed, writes a new manifest, and
//! publishes a signed Nostr pointer to it; hosts follow the pointer and
//! swap the manifest in memory. Every site or game is a tenant with its own
//! key, manifest history, and channels; objects live in one store that all
//! tenants share. See `docs/audits/2026-10-21-web-deploys-in-seconds.md`.

pub mod bench;
pub mod deploy;
pub mod handler;
pub mod host;
pub mod keys;
pub mod load;
pub mod manifest;
pub mod pointer;
pub mod relay;
pub mod store;
pub mod template;

use sha2::{Digest, Sha256};

/// The lowercase hex SHA-256 of `bytes`, the name of every stored object.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Milliseconds since `start`, with microsecond precision.
pub fn ms_since(start: std::time::Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}
