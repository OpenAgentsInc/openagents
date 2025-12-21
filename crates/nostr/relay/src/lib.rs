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

mod db;
mod server;
mod error;
mod subscription;
mod broadcast;
mod rate_limit;
mod relay_info;
mod metrics;
mod admin;
mod validation;

pub use db::{Database, DatabaseConfig, ConnectionPool};
pub use server::{RelayServer, RelayConfig};
pub use error::{RelayError, Result};
pub use subscription::{Filter, Subscription, SubscriptionManager};
pub use broadcast::{BroadcastEvent, create_broadcast_channel};
pub use rate_limit::{RateLimiter, RateLimitConfig};
pub use relay_info::{RelayInformation, Limitation, RetentionPolicy, Fees, FeeSchedule, KindOrRange};
pub use metrics::{RelayMetrics, MetricsSnapshot};
pub use admin::{AdminConfig, start_admin_server, HealthResponse, StatsResponse};
pub use validation::{
    validate_event, validate_event_structure, validate_filter, validate_subscription_id,
    validate_event_message, validate_req_message, validate_close_message, ValidationError,
    MAX_EVENT_SIZE, MAX_SUBSCRIPTION_ID_LENGTH, MAX_CONTENT_LENGTH, MAX_TAGS, MAX_TAG_LENGTH,
};
