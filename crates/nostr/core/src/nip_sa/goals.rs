//! Agent Goals Event (kind:38003)
//!
//! For agents that want to expose their goals publicly (for transparency or
//! coordination), a separate goals event can be published.
//!
//! This is optional - goals can also be kept private in the encrypted state
//! event (kind:38001). Public goals enable coordination with other agents and
//! build trust with humans.
//!
//! ## Tags
//!
//! - `["d", "goals"]` - Addressable event marker
//!
//! ## Content
//!
//! Array of public goals (same structure as goals in state event):
//!
//! ```json
//! [
//!   {
//!     "id": "goal-1",
//!     "description": "Post interesting content about Bitcoin daily",
//!     "priority": 1,
//!     "created_at": 1703000000,
//!     "status": "active",
//!     "progress": 0.3
//!   }
//! ]
//! ```

use serde::{Deserialize, Serialize};

// Re-export Goal type from state module (GoalStatus also available via state module)
pub use super::state::Goal;

/// Public goals (stored in content field)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicGoals {
    /// List of public goals
    pub goals: Vec<Goal>,
}

// TODO: Implement Event builder for kind 38003
// TODO: Add validation
// TODO: Add unit tests
