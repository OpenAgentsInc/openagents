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
use thiserror::Error;

#[cfg(feature = "full")]
use crate::nip44;

/// Kind for agent state event
pub const KIND_AGENT_STATE: u16 = 38001;

/// Current state schema version
pub const STATE_VERSION: u32 = 1;

/// Errors that can occur during NIP-SA state operations
#[derive(Debug, Error)]
pub enum StateError {
    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),

    #[error("encryption error: {0}")]
    Encryption(String),

    #[error("decryption error: {0}")]
    Decryption(String),

    #[error("unsupported state version: {0}")]
    UnsupportedVersion(u32),

    #[error("missing required field: {0}")]
    MissingField(String),
}

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
    /// Budget tracker for spending enforcement
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget: Option<super::budget::BudgetTracker>,
}

impl AgentStateContent {
    /// Create new agent state
    pub fn new() -> Self {
        Self {
            goals: Vec::new(),
            memory: Vec::new(),
            pending_tasks: Vec::new(),
            beliefs: HashMap::new(),
            wallet_balance_sats: 0,
            last_tick: 0,
            tick_count: 0,
            budget: None,
        }
    }

    /// Create new agent state with budget enforcement
    pub fn with_budget(budget_limits: super::budget::BudgetLimits) -> Self {
        Self {
            goals: Vec::new(),
            memory: Vec::new(),
            pending_tasks: Vec::new(),
            beliefs: HashMap::new(),
            wallet_balance_sats: 0,
            last_tick: 0,
            tick_count: 0,
            budget: Some(super::budget::BudgetTracker::with_limits(budget_limits)),
        }
    }

    /// Enable budget enforcement with default limits
    pub fn enable_budget(&mut self) {
        if self.budget.is_none() {
            self.budget = Some(super::budget::BudgetTracker::new());
        }
    }

    /// Get budget tracker (mutable)
    pub fn budget_mut(&mut self) -> Option<&mut super::budget::BudgetTracker> {
        self.budget.as_mut()
    }

    /// Check if a spend operation is allowed
    ///
    /// Returns Ok(()) if spend is allowed, Err with reason if not.
    ///
    /// # Arguments
    /// * `amount_sats` - Amount to spend in satoshis
    ///
    /// # Example
    /// ```
    /// use nostr::nip_sa::AgentStateContent;
    ///
    /// let mut state = AgentStateContent::new();
    /// state.enable_budget();
    /// state.update_balance(100_000);
    ///
    /// // Check if we can spend 500 sats
    /// assert!(state.check_spend(500).is_ok());
    /// ```
    pub fn check_spend(&self, amount_sats: u64) -> Result<(), super::budget::BudgetError> {
        if let Some(budget) = &self.budget {
            budget.check_spend(amount_sats, self.wallet_balance_sats)
        } else {
            // No budget enforcement - only check balance
            if amount_sats > self.wallet_balance_sats {
                Err(super::budget::BudgetError::InsufficientBalance {
                    needed: amount_sats,
                    available: self.wallet_balance_sats,
                })
            } else {
                Ok(())
            }
        }
    }

    /// Record a spend operation
    ///
    /// Should be called after a spend succeeds to update budget counters.
    ///
    /// # Arguments
    /// * `amount_sats` - Amount spent in satoshis
    pub fn record_spend(&mut self, amount_sats: u64) {
        if let Some(budget) = &mut self.budget {
            budget.record_spend(amount_sats);
        }
        // Also update balance
        self.wallet_balance_sats = self.wallet_balance_sats.saturating_sub(amount_sats);
    }

    /// Add a goal
    pub fn add_goal(&mut self, goal: Goal) {
        self.goals.push(goal);
    }

    /// Add a memory entry
    pub fn add_memory(&mut self, entry: MemoryEntry) {
        self.memory.push(entry);
    }

    /// Update wallet balance
    ///
    /// # Arguments
    /// * `balance_sats` - Total balance in satoshis across all layers (Spark L2, Lightning, on-chain)
    ///
    /// # Example
    /// ```
    /// use nostr::nip_sa::AgentStateContent;
    ///
    /// let mut state = AgentStateContent::new();
    /// state.update_balance(100_000);
    /// assert_eq!(state.wallet_balance_sats, 100_000);
    /// ```
    pub fn update_balance(&mut self, balance_sats: u64) {
        self.wallet_balance_sats = balance_sats;
    }

    /// Increment tick count and update timestamp
    ///
    /// This also resets the per-tick budget counter and checks for daily budget reset.
    pub fn record_tick(&mut self, timestamp: u64) {
        self.tick_count += 1;
        self.last_tick = timestamp;

        // Reset tick budget and check for daily reset
        if let Some(budget) = &mut self.budget {
            budget.reset_tick();
            budget.check_and_reset_daily();
        }
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, StateError> {
        serde_json::to_string(self).map_err(|e| StateError::Serialization(e.to_string()))
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, StateError> {
        serde_json::from_str(json).map_err(|e| StateError::Deserialization(e.to_string()))
    }
}

impl Default for AgentStateContent {
    fn default() -> Self {
        Self::new()
    }
}

impl Goal {
    /// Create a new goal
    pub fn new(id: impl Into<String>, description: impl Into<String>, priority: u32) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            id: id.into(),
            description: description.into(),
            priority,
            created_at: now,
            status: GoalStatus::Active,
            progress: 0.0,
        }
    }

    /// Update goal progress
    pub fn update_progress(&mut self, progress: f64) {
        self.progress = progress.clamp(0.0, 1.0);
        if self.progress >= 1.0 {
            self.status = GoalStatus::Completed;
        }
    }

    /// Pause the goal
    pub fn pause(&mut self) {
        self.status = GoalStatus::Paused;
    }

    /// Resume the goal
    pub fn resume(&mut self) {
        if self.status == GoalStatus::Paused {
            self.status = GoalStatus::Active;
        }
    }

    /// Cancel the goal
    pub fn cancel(&mut self) {
        self.status = GoalStatus::Cancelled;
    }
}

impl MemoryEntry {
    /// Create a new memory entry
    pub fn new(memory_type: impl Into<String>, content: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            memory_type: memory_type.into(),
            content: content.into(),
            timestamp: now,
        }
    }

    /// Create a memory entry with specific timestamp
    pub fn with_timestamp(
        memory_type: impl Into<String>,
        content: impl Into<String>,
        timestamp: u64,
    ) -> Self {
        Self {
            memory_type: memory_type.into(),
            content: content.into(),
            timestamp,
        }
    }
}

/// Agent state with encryption support
#[derive(Debug, Clone)]
pub struct AgentState {
    /// State content
    pub content: AgentStateContent,
    /// State version
    pub version: u32,
}

impl AgentState {
    /// Create new agent state
    pub fn new(content: AgentStateContent) -> Self {
        Self {
            content,
            version: STATE_VERSION,
        }
    }

    /// Build tags for the event
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        vec![
            vec!["d".to_string(), "state".to_string()],
            vec!["encrypted".to_string()],
            vec!["state_version".to_string(), self.version.to_string()],
        ]
    }

    /// Encrypt state content using NIP-44
    #[cfg(feature = "full")]
    pub fn encrypt(
        &self,
        sender_secret_key: &[u8; 32],
        agent_public_key: &[u8],
    ) -> Result<String, StateError> {
        let json = self.content.to_json()?;
        nip44::encrypt(sender_secret_key, agent_public_key, &json)
            .map_err(|e| StateError::Encryption(e.to_string()))
    }

    /// Decrypt state content using NIP-44
    #[cfg(feature = "full")]
    pub fn decrypt(
        encrypted_content: &str,
        recipient_secret_key: &[u8; 32],
        sender_public_key: &[u8],
        version: u32,
    ) -> Result<Self, StateError> {
        if version > STATE_VERSION {
            return Err(StateError::UnsupportedVersion(version));
        }

        let json = nip44::decrypt(recipient_secret_key, sender_public_key, encrypted_content)
            .map_err(|e| StateError::Decryption(e.to_string()))?;

        let content = AgentStateContent::from_json(&json)?;

        Ok(Self { content, version })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goal_creation() {
        let goal = Goal::new("goal-1", "Test goal", 1);
        assert_eq!(goal.id, "goal-1");
        assert_eq!(goal.description, "Test goal");
        assert_eq!(goal.priority, 1);
        assert_eq!(goal.status, GoalStatus::Active);
        assert_eq!(goal.progress, 0.0);
    }

    #[test]
    fn test_goal_progress() {
        let mut goal = Goal::new("goal-1", "Test goal", 1);
        goal.update_progress(0.5);
        assert_eq!(goal.progress, 0.5);
        assert_eq!(goal.status, GoalStatus::Active);

        goal.update_progress(1.0);
        assert_eq!(goal.progress, 1.0);
        assert_eq!(goal.status, GoalStatus::Completed);

        // Test clamping
        goal.status = GoalStatus::Active;
        goal.update_progress(1.5);
        assert_eq!(goal.progress, 1.0);
    }

    #[test]
    fn test_goal_pause_resume() {
        let mut goal = Goal::new("goal-1", "Test goal", 1);
        goal.pause();
        assert_eq!(goal.status, GoalStatus::Paused);

        goal.resume();
        assert_eq!(goal.status, GoalStatus::Active);
    }

    #[test]
    fn test_goal_cancel() {
        let mut goal = Goal::new("goal-1", "Test goal", 1);
        goal.cancel();
        assert_eq!(goal.status, GoalStatus::Cancelled);
    }

    #[test]
    fn test_memory_entry() {
        let entry = MemoryEntry::new("observation", "Test memory");
        assert_eq!(entry.memory_type, "observation");
        assert_eq!(entry.content, "Test memory");
        assert!(entry.timestamp > 0);
    }

    #[test]
    fn test_memory_entry_with_timestamp() {
        let entry = MemoryEntry::with_timestamp("observation", "Test memory", 1703000000);
        assert_eq!(entry.timestamp, 1703000000);
    }

    #[test]
    fn test_agent_state_content() {
        let mut state = AgentStateContent::new();
        assert_eq!(state.goals.len(), 0);
        assert_eq!(state.memory.len(), 0);
        assert_eq!(state.wallet_balance_sats, 0);
        assert_eq!(state.tick_count, 0);

        state.add_goal(Goal::new("goal-1", "Test goal", 1));
        assert_eq!(state.goals.len(), 1);

        state.add_memory(MemoryEntry::new("observation", "Test memory"));
        assert_eq!(state.memory.len(), 1);

        state.update_balance(1000);
        assert_eq!(state.wallet_balance_sats, 1000);

        state.record_tick(1703000000);
        assert_eq!(state.tick_count, 1);
        assert_eq!(state.last_tick, 1703000000);
    }

    #[test]
    fn test_agent_state_content_serialization() {
        let mut state = AgentStateContent::new();
        state.add_goal(Goal::new("goal-1", "Test goal", 1));
        state.add_memory(MemoryEntry::with_timestamp("observation", "Test memory", 1703000000));
        state.update_balance(1000);

        let json = state.to_json().unwrap();
        let parsed = AgentStateContent::from_json(&json).unwrap();

        assert_eq!(parsed.goals.len(), 1);
        assert_eq!(parsed.memory.len(), 1);
        assert_eq!(parsed.wallet_balance_sats, 1000);
    }

    #[test]
    fn test_agent_state_tags() {
        let state = AgentState::new(AgentStateContent::new());
        let tags = state.build_tags();

        assert_eq!(tags[0], vec!["d", "state"]);
        assert_eq!(tags[1], vec!["encrypted"]);
        assert_eq!(tags[2], vec!["state_version", "1"]);
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_agent_state_encryption_roundtrip() {
        use bitcoin::secp256k1::{PublicKey as Secp256k1PubKey, Secp256k1, SecretKey};

        let secp = Secp256k1::new();

        let mut state_content = AgentStateContent::new();
        state_content.add_goal(Goal::new("goal-1", "Test goal", 1));
        state_content.add_memory(MemoryEntry::with_timestamp(
            "observation",
            "Test memory",
            1703000000,
        ));
        state_content.update_balance(5000);

        let state = AgentState::new(state_content);

        // Generate keys for testing
        let sender_sk = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let recipient_sk = SecretKey::from_slice(&[2u8; 32]).unwrap();
        let sender_pk = Secp256k1PubKey::from_secret_key(&secp, &sender_sk);
        let recipient_pk = Secp256k1PubKey::from_secret_key(&secp, &recipient_sk);

        // Encrypt
        let encrypted = state
            .encrypt(&sender_sk.secret_bytes(), &recipient_pk.serialize())
            .unwrap();
        assert!(!encrypted.is_empty());

        // Decrypt
        let decrypted = AgentState::decrypt(
            &encrypted,
            &recipient_sk.secret_bytes(),
            &sender_pk.serialize(),
            STATE_VERSION,
        )
        .unwrap();

        assert_eq!(decrypted.content.goals.len(), 1);
        assert_eq!(decrypted.content.memory.len(), 1);
        assert_eq!(decrypted.content.wallet_balance_sats, 5000);
        assert_eq!(decrypted.version, STATE_VERSION);
    }

    #[test]
    fn test_goal_status_serialization() {
        let active = GoalStatus::Active;
        let json = serde_json::to_string(&active).unwrap();
        assert_eq!(json, "\"active\"");

        let paused = GoalStatus::Paused;
        let json = serde_json::to_string(&paused).unwrap();
        assert_eq!(json, "\"paused\"");

        let completed = GoalStatus::Completed;
        let json = serde_json::to_string(&completed).unwrap();
        assert_eq!(json, "\"completed\"");

        let cancelled = GoalStatus::Cancelled;
        let json = serde_json::to_string(&cancelled).unwrap();
        assert_eq!(json, "\"cancelled\"");
    }
}
