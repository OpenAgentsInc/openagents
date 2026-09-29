//! A reproducible benchmark of how fast the OpenAgents phone apps load
//! chats: the chat list, opening a chat, and laying it out.
//!
//! It drives the real `coder_connect` client, observer host, relay
//! transport, and direct transport, and the real `coder_history` reader and
//! `rust_native` transcript layout. Only the phone's read loops are
//! mirrored (`phone`), because `crates/openagents-mobile` is its own Cargo
//! workspace. Results and how to read them are in
//! `docs/coder/runtime/chat-load-benchmark.md`.

pub mod bench;
pub mod cj;
pub mod dataset;
pub mod phone;
pub mod stats;

pub mod relay;
