//! Agent Profile Event (kind:38000)
//!
//! Agents publish a profile event similar to `kind:0` user metadata, but with
//! additional agent-specific fields.
//!
//! ## Fields
//!
//! - `name` - Agent display name
//! - `about` - Agent description
//! - `picture` - Avatar URL
//! - `capabilities` - List of supported features
//! - `autonomy_level` - supervised | bounded | autonomous
//! - `version` - Agent version string
//!
//! ## Tags
//!
//! - `["d", "profile"]` - Addressable event marker
//! - `["threshold", "2", "3"]` - Threshold signature scheme (2-of-3)
//! - `["signer", "<pubkey>"]` - Marketplace/guardian signer pubkeys
//! - `["operator", "<pubkey>"]` - Human operator pubkey
//! - `["lud16", "<address>"]` - Lightning address for payments
//!
//! ## Example
//!
//! ```json
//! {
//!   "kind": 38000,
//!   "pubkey": "<agent-pubkey>",
//!   "content": "{\"name\":\"ResearchBot\",\"about\":\"I research topics\"}",
//!   "tags": [
//!     ["d", "profile"],
//!     ["threshold", "2", "3"],
//!     ["signer", "<marketplace-pubkey>"],
//!     ["operator", "<operator-pubkey>"]
//!   ]
//! }
//! ```

use serde::{Deserialize, Serialize};

/// Autonomy level of the agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyLevel {
    /// Agent requests approval before major actions
    Supervised,
    /// Agent acts within defined constraints without approval
    Bounded,
    /// Agent acts freely toward goals
    Autonomous,
}

/// Agent profile metadata (stored in content field)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfileContent {
    /// Agent display name
    pub name: String,
    /// Agent description
    pub about: String,
    /// Avatar URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    /// List of supported capabilities
    pub capabilities: Vec<String>,
    /// Autonomy level
    pub autonomy_level: AutonomyLevel,
    /// Agent version
    pub version: String,
}

// TODO: Implement Event builder for kind 38000
// TODO: Add validation per NIP-SA rules
// TODO: Add unit tests for serialization/deserialization
