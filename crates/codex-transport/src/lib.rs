//! The operator's Codex login and the ChatGPT Codex Responses transport.
//!
//! - [`transport`] is the one seam to the model: a [`Transport`] takes one
//!   [`Request`] and returns one [`Reply`] with the usage the provider
//!   reported.
//! - [`codex`] reads the Codex login (`~/.codex/auth.json`), never
//!   refreshing it, and [`codex::CodexTransport`] calls the ChatGPT Codex
//!   Responses endpoint on it. [`codex::UsageLimit`] is the typed HTTP 429
//!   `usage_limit_reached` refusal the capacity book records.
//! - [`price`] turns usage into dollars at list prices.
//! - [`oneshot`] asks for one structured value as one tool call, for
//!   callers such as the Microcoder loop and `kb harvest`.
//! - [`fake::FakeTransport`] answers from a script in tests.
//!
//! These moved here from `crates/microluna` on 2026-09-28 (issue #9880),
//! when Microcoder replaced Microluna, so that the Microcoder loop, Coder,
//! and the knowledge base reach a Codex login without depending on
//! Microluna. `crates/microluna` re-exports them for its retained session
//! harness.

pub mod codex;
pub mod fake;
pub mod oneshot;
pub mod price;
pub mod transport;

pub use transport::{Reply, Request, TokenUsage, Transport, TransportError};
