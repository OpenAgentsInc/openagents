//! Nostr relay implementation for OpenAgents
//!
//! This crate provides a complete Nostr relay implementation with:
//! - SQLite storage with connection pooling
//! - WebSocket server for client connections
//! - Subscription management and event filtering
//! - Event validation and signature verification
//! - Broadcast system for real-time updates
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │         WebSocket Server (warp)             │
//! └────────────────┬────────────────────────────┘
//!                  │
//! ┌────────────────▼────────────────────────────┐
//! │         Connection Manager                  │
//! │    Per-client state, subscription tracking  │
//! └────────────────┬────────────────────────────┘
//!                  │
//!   ┌──────────────┼──────────────┐
//!   ▼              ▼              ▼
//! ┌────────┐  ┌────────┐  ┌────────┐
//! │ Writer │  │ Reader │  │ Meta   │
//! │  Pool  │  │  Pool  │  │  Pool  │
//! └────────┘  └────────┘  └────────┘
//!      │           │           │
//!      └───────────┼───────────┘
//!                  ▼
//!        ┌─────────────────┐
//!        │  Broadcast Bus  │
//!        └─────────────────┘
//! ```

mod admin;
mod broadcast;
mod db;
mod error;
mod metrics;
mod negentropy;
mod rate_limit;
mod relay_info;
mod server;
mod subscription;
mod validation;

#[cfg(test)]
mod tests;

pub use admin::{AdminConfig, HealthResponse, StatsResponse, start_admin_server};
pub use broadcast::{BroadcastEvent, create_broadcast_channel};
pub use db::{ConnectionPool, Database, DatabaseConfig};
pub use error::{RelayError, Result};
pub use metrics::{MetricsSnapshot, RelayMetrics};
pub use negentropy::{NegentropySession, NegentropySessionManager, SessionId};
pub use rate_limit::{RateLimitConfig, RateLimiter};
pub use relay_info::{
    FeeSchedule, Fees, KindOrRange, Limitation, RelayInformation, RetentionPolicy,
};
pub use server::{RelayConfig, RelayServer};
pub use subscription::{Filter, Subscription, SubscriptionManager};
pub use validation::{
    MAX_CONTENT_LENGTH, MAX_EVENT_SIZE, MAX_SUBSCRIPTION_ID_LENGTH, MAX_TAG_LENGTH, MAX_TAGS,
    ValidationError, validate_close_message, validate_event, validate_event_message,
    validate_event_structure, validate_filter, validate_req_message, validate_subscription_id,
};
