//! The account's chats as the site's JSON API serves them.
//!
//! [`types`] re-exports the SDK's chat values (`openagents_sdk::chats::wire`,
//! the same ones the desktop and phone apps read) and adds the site-only
//! shapes (`projects/api.rs`, the refusal body). On wasm32,
//! [`web::Client`] reads and writes them with `fetch` and follows an answer
//! with `EventSource`, signed in by the site's own cookie; writes carry the
//! page's CSRF value in `x-openagents-csrf`.

pub mod event;
pub mod types;

#[cfg(target_family = "wasm")]
pub mod web;

/// The browser as a client of one of the account's computers (#12264).
#[cfg(target_family = "wasm")]
pub mod computer;

pub use types::*;
