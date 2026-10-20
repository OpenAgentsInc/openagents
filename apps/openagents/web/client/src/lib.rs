//! The account's chats as the site's JSON API serves them.
//!
//! [`types`] holds the shapes `crates/openagents-web` answers with
//! (`phone_api.rs`, `chat_api.rs`, `projects/api.rs`). On wasm32,
//! [`web::Client`] reads and writes them with `fetch` and follows an answer
//! with `EventSource`, signed in by the site's own cookie; writes carry the
//! page's CSRF value in `x-openagents-csrf`.

pub mod types;

#[cfg(target_family = "wasm")]
pub mod web;

pub use types::*;
