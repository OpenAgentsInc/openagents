//! # coder_protocol - Wire Protocol for Coder
//!
//! This crate defines the wire protocol for communication between
//! Coder clients and servers. It provides message types for:
//!
//! - Subscriptions to domain event streams
//! - Sending user messages
//! - Receiving domain events and snapshots
//!
//! ## Protocol Overview
//!
//! The protocol uses a simple request/response + streaming model:
//!
//! 1. Client subscribes to a thread
//! 2. Server sends initial snapshot
//! 3. Server streams domain events as they occur
//! 4. Client can send messages/commands at any time
//!
//! ## Example
//!
//! ```rust,ignore
//! use coder_protocol::{ClientMessage, ServerMessage};
//!
//! // Subscribe to a thread
//! let subscribe = ClientMessage::Subscribe { thread_id };
//!
//! // Handle server response
//! match server_message {
//!     ServerMessage::Snapshot { chat_view } => { /* initial state */ }
//!     ServerMessage::Events { events } => { /* incremental updates */ }
//!     ServerMessage::Error { code, message } => { /* handle error */ }
//! }
//! ```

pub mod client;
pub mod server;

pub use client::ClientMessage;
pub use server::{ErrorCode, ServerMessage};
