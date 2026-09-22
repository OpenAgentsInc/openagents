//! Bounded NIP-01 WebSocket and NIP-11 HTTP gateway.

mod auth;
mod config;
mod db;
mod error;
mod management;
mod media;
mod push;
mod query;
mod rate;
mod server;
mod socket;
mod subscription;
mod wire;

pub use config::{GatewayConfig, GatewayLimits, MediaConfig, PushExecutor, RelayIdentity};
pub use error::GatewayError;
pub use server::{GIFT_WRAP_RECIPIENT_RATE_EXCEEDED, Gateway, ShutdownHandle};
