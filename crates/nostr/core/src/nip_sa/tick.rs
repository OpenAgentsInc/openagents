//! Tick Events (kinds:38010, 38011)
//!
//! Tick events track agent execution cycles. Each "tick" represents one run of
//! the agent - processing inputs, updating state, taking actions.
//!
//! ## Tick Request (kind:38010)
//!
//! Published by the runner at the start of a tick to signal execution.
//!
//! Tags:
//! - `["runner", "<runner-pubkey>"]` - Runner identity
//! - `["trigger", "heartbeat|mention|dm|zap"]` - What triggered this tick
//!
//! ## Tick Result (kind:38011)
//!
//! Published by the runner at the end of a tick with outcome metrics.
//!
//! Tags:
//! - `["request", "<tick-request-event-id>"]` - Links to request event
//! - `["runner", "<runner-pubkey>"]` - Runner identity
//! - `["status", "success|failure|timeout"]` - Tick outcome
//! - `["duration_ms", "1234"]` - Execution time
//! - `["actions", "3"]` - Number of actions taken
//!
//! Content: JSON metrics
//!
//! ```json
//! {
//!   "tokens_in": 1000,
//!   "tokens_out": 500,
//!   "cost_usd": 0.05,
//!   "goals_updated": 2,
//!   "actions": [
//!     {"type": "post", "id": "event-id-1"},
//!     {"type": "dm", "recipient": "npub..."}
//!   ]
//! }
//! ```

use serde::{Deserialize, Serialize};

/// What triggered a tick
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TickTrigger {
    /// Regular heartbeat
    Heartbeat,
    /// Mentioned in a note
    Mention,
    /// Received a DM
    Dm,
    /// Received a zap
    Zap,
    /// Manual trigger
    Manual,
}

/// Tick outcome status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TickStatus {
    /// Tick completed successfully
    Success,
    /// Tick failed with error
    Failure,
    /// Tick exceeded time limit
    Timeout,
}

/// Action taken during a tick
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickAction {
    /// Action type (post, dm, zap, etc.)
    #[serde(rename = "type")]
    pub action_type: String,
    /// Event ID if action resulted in an event
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Additional action metadata
    #[serde(flatten)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

/// Tick result metrics (stored in content field)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickResultContent {
    /// Tokens consumed (input)
    pub tokens_in: u64,
    /// Tokens generated (output)
    pub tokens_out: u64,
    /// Cost in USD
    pub cost_usd: f64,
    /// Number of goals updated
    pub goals_updated: u32,
    /// Actions taken during this tick
    pub actions: Vec<TickAction>,
}

// TODO: Implement Event builders for kinds 38010, 38011
// TODO: Add validation
// TODO: Add unit tests
