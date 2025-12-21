//! Agent State Event (kind:38001)
//!
//! Agent state is stored as an addressable event with NIP-44 encrypted content.
//! State includes goals, memory, pending tasks, beliefs, and other persistent data.
//!
//! ## Security
//!
//! State is encrypted to the agent's pubkey using NIP-44. Decryption requires
//! threshold ECDH with the marketplace signer, which enforces that only legitimate
//! agent ticks can access state.
//!
//! ## Tags
//!
//! - `["d", "state"]` - Addressable event marker
//! - `["encrypted"]` - Indicates encrypted content
//! - `["state_version", "1"]` - State schema version for migration
//!
//! ## Encrypted Content
//!
//! The decrypted state contains:
//!
//! ```json
//! {
//!   "goals": [
//!     {
//!       "id": "goal-1",
//!       "description": "Post interesting content about Bitcoin daily",
//!       "priority": 1,
//!       "created_at": 1703000000,
//!       "status": "active",
//!       "progress": 0.3
//!     }
//!   ],
//!   "memory": [
//!     {
//!       "type": "observation",
//!       "content": "Last post received 50 reactions",
//!       "timestamp": 1703001000
//!     }
//!   ],
//!   "pending_tasks": [],
//!   "beliefs": {
//!     "follower_count": 1500,
//!     "avg_engagement": 0.03
//!   },
//!   "wallet_balance_sats": 50000,
//!   "last_tick": 1703002000,
//!   "tick_count": 42
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Agent goal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    /// Unique goal identifier
    pub id: String,
    /// Goal description
    pub description: String,
    /// Priority (lower number = higher priority)
    pub priority: u32,
    /// Creation timestamp (Unix seconds)
    pub created_at: u64,
    /// Goal status
    pub status: GoalStatus,
    /// Progress (0.0 to 1.0)
    pub progress: f64,
}

/// Goal status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    /// Goal is active
    Active,
    /// Goal is paused
    Paused,
    /// Goal is completed
    Completed,
    /// Goal is cancelled
    Cancelled,
}

/// Memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Memory type (observation, action, reflection, etc.)
    #[serde(rename = "type")]
    pub memory_type: String,
    /// Memory content
    pub content: String,
    /// Timestamp (Unix seconds)
    pub timestamp: u64,
}

/// Agent state (decrypted content)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStateContent {
    /// Active goals with progress
    pub goals: Vec<Goal>,
    /// Agent memories
    pub memory: Vec<MemoryEntry>,
    /// Pending tasks
    pub pending_tasks: Vec<String>,
    /// Agent beliefs (key-value store)
    pub beliefs: HashMap<String, serde_json::Value>,
    /// Wallet balance in satoshis
    pub wallet_balance_sats: u64,
    /// Last tick timestamp (Unix seconds)
    pub last_tick: u64,
    /// Total tick count
    pub tick_count: u64,
}

// TODO: Implement NIP-44 encryption to agent pubkey
// TODO: Implement state versioning for migration support
// TODO: Implement Event builder for kind 38001
// TODO: Add encryption/decryption helpers
// TODO: Add unit tests with mock keys
