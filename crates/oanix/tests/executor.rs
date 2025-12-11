//! E2E tests for OANIX executors (HttpExecutor, WsConnector, NostrRelayConnector)
//!
//! ## Test Categories
//!
//! - **Local mock tests**: Fast, deterministic tests using mock servers
//! - **Smoke tests**: Live endpoint tests (run with `--ignored`)
//!
//! ## Running Tests
//!
//! ```bash
//! # Fast CI tests (mock servers only)
//! cargo test --features "net-executor,nostr" -p oanix --test executor
//!
//! # Include live smoke tests
//! cargo test --features "net-executor,nostr" -p oanix --test executor -- --ignored
//! ```

#[path = "executor/fixtures/mod.rs"]
pub mod fixtures;

#[path = "executor/http_tests.rs"]
pub mod http_tests;

#[path = "executor/ws_tests.rs"]
pub mod ws_tests;

#[cfg(feature = "nostr")]
#[path = "executor/nostr_tests.rs"]
pub mod nostr_tests;

#[path = "executor/smoke/mod.rs"]
pub mod smoke;
