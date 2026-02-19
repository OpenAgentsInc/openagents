//! OpenAgents Relay Protocol
//!
//! Defines the WebSocket message protocol for the tunnel relay system.
//! Used by:
//! - `openagents-web-worker` (Cloudflare Worker relay)
//! - `openagents-connect` (tunnel client CLI)
//! - `openagents-web-client` (browser WASM client)
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐                  ┌─────────────────┐                  ┌─────────────────┐
//! │   Browser   │◄───WebSocket────►│  CF Worker      │◄───WebSocket────►│  Tunnel Client  │
//! │  (WASM)     │                  │  (Relay DO)     │                  │  (CLI)          │
//! └─────────────┘                  └─────────────────┘                  └─────────────────┘
//!       │                                 │                                     │
//!       │  RelayMessage::StartTask        │                                     │
//!       │ ─────────────────────────────► │ ────────────────────────────────► │
//!       │                                 │                                     │
//!       │                                 │  RelayMessage::AutopilotChunk       │
//!       │ ◄───────────────────────────── │ ◄──────────────────────────────── │
//!       │                                 │                                     │
//! ```

mod protocol;

pub use protocol::*;
