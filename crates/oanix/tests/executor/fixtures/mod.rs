//! Test fixtures for executor E2E tests
//!
//! Provides mock servers and test utilities:
//! - `HttpMockServer` - wiremock-based HTTP mock
//! - `WsEchoServer` - WebSocket echo server
//! - `NostrMockRelay` - NIP-01 mock relay
//! - `ExecutorTestFixture` - Test harness with pre-configured executors

pub mod helpers;
pub mod http_mock;
pub mod ws_echo;

#[cfg(feature = "nostr")]
pub mod nostr_relay;

pub use helpers::*;
pub use http_mock::*;
pub use ws_echo::*;

#[cfg(feature = "nostr")]
pub use nostr_relay::*;
