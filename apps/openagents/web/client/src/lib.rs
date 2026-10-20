//! The account's chats as the site's JSON API serves them.
//!
//! [`types`] re-exports the SDK's chat values (`openagents_sdk::chats::wire`,
//! the same ones the desktop and phone apps read) and adds the site-only
//! shapes (`projects/api.rs`, the refusal body). On wasm32,
//! [`web::Client`] reads and writes them with `fetch` and follows an answer
//! with `EventSource`, signed in by the site's own cookie; writes carry the
//! page's CSRF value in `x-openagents-csrf`.

/// One chat's chat2 room, the protocol half (#12224).
pub mod chat_replica;
pub mod cloud_computers;
pub mod event;
/// Moving a chat to a cloud computer and back (#12282).
pub mod moves;
pub mod types;
/// Every computer's chats, from the account's registry (#12265).
pub mod workspace;

#[cfg(target_family = "wasm")]
pub mod web;

/// The browser as a client of one of the account's computers (#12264).
#[cfg(target_family = "wasm")]
pub mod computer;

/// One chat's chat2 room over a WebSocket (#12224).
#[cfg(target_family = "wasm")]
pub mod chat_room;

/// The registry room over a WebSocket (#12265).
#[cfg(target_family = "wasm")]
pub mod registry;

pub use types::*;
