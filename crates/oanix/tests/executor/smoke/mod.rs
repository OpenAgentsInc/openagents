//! Live endpoint smoke tests
//!
//! These tests hit real public services and are marked `#[ignore]`.
//! Run with: `cargo test --features "net-executor,nostr" -p oanix -- --ignored`

pub mod http_live;
pub mod ws_live;

#[cfg(feature = "nostr")]
pub mod nostr_live;
