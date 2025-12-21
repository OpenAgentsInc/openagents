//! Agent Schedule Event (kind:38002)
//!
//! Defines when and how an agent should be triggered to run.
//!
//! ## Trigger Types
//!
//! - **Heartbeat** - Regular interval (e.g., every 15 minutes)
//! - **Event** - Triggered by Nostr events (mentions, DMs, zaps)
//! - **Condition** - Triggered when a condition is met (e.g., price threshold)
//!
//! ## Tags
//!
//! - `["d", "schedule"]` - Addressable event marker
//! - `["heartbeat", "900"]` - Heartbeat interval in seconds
//! - `["trigger", "mention"]` - Event trigger type
//! - `["trigger", "dm"]` - Event trigger type
//! - `["trigger", "zap"]` - Event trigger type
//!
//! ## Example
//!
//! ```json
//! {
//!   "kind": 38002,
//!   "pubkey": "<agent-pubkey>",
//!   "content": "",
//!   "tags": [
//!     ["d", "schedule"],
//!     ["heartbeat", "900"],
//!     ["trigger", "mention"],
//!     ["trigger", "dm"]
//!   ]
//! }
//! ```

use serde::{Deserialize, Serialize};

/// Event trigger type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerType {
    /// Mentioned in a note
    Mention,
    /// Received a DM
    Dm,
    /// Received a zap
    Zap,
    /// Custom event kind
    Custom(u32),
}

/// Agent schedule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSchedule {
    /// Heartbeat interval in seconds (None = no heartbeat)
    pub heartbeat_seconds: Option<u64>,
    /// Event triggers
    pub triggers: Vec<TriggerType>,
}

// TODO: Implement Event builder for kind 38002
// TODO: Add validation (heartbeat > 0 if Some)
// TODO: Add unit tests
