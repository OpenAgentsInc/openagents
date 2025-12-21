//! Agent State Management
//!
//! This module provides helpers for managing encrypted agent state using NIP-SA.
//! Agent state is encrypted to the agent's pubkey using NIP-44 and published
//! to relays as kind:38001 events.

use anyhow::{Context, Result};
use nostr::{AgentState, AgentStateContent, KIND_AGENT_STATE};

/// Agent state manager
pub struct StateManager {
    /// Agent secret key for encryption/decryption
    agent_secret_key: [u8; 32],
    /// Agent public key
    agent_public_key: [u8; 33],
}

impl StateManager {
    /// Create a new state manager
    pub fn new(agent_secret_key: [u8; 32], agent_public_key: [u8; 33]) -> Self {
        Self {
            agent_secret_key,
            agent_public_key,
        }
    }

    /// Encrypt agent state content
    pub fn encrypt_state(&self, content: &AgentStateContent) -> Result<String> {
        let state = AgentState::new(content.clone());
        state
            .encrypt(&self.agent_secret_key, &self.agent_public_key)
            .context("Failed to encrypt agent state")
    }

    /// Decrypt agent state content from encrypted string
    pub fn decrypt_state(
        &self,
        encrypted_content: &str,
        version: u32,
    ) -> Result<AgentStateContent> {
        let state = AgentState::decrypt(
            encrypted_content,
            &self.agent_secret_key,
            &self.agent_public_key,
            version,
        )
        .context("Failed to decrypt agent state")?;

        Ok(state.content)
    }

    /// Create a new empty state
    pub fn create_empty_state() -> AgentStateContent {
        AgentStateContent::new()
    }

    /// Get the agent state event kind
    pub fn state_kind() -> u16 {
        KIND_AGENT_STATE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::{Goal, MemoryEntry};

    fn create_test_keys() -> ([u8; 32], [u8; 33]) {
        let secret_key = [1u8; 32];
        // For testing, use a mock public key (in production this would be derived from secret key)
        let mut public_key = [0u8; 33];
        public_key[0] = 0x02; // Compressed public key prefix
        (secret_key, public_key)
    }

    #[test]
    fn test_create_empty_state() {
        let state = StateManager::create_empty_state();
        assert_eq!(state.goals.len(), 0);
        assert_eq!(state.memory.len(), 0);
        assert_eq!(state.wallet_balance_sats, 0);
        assert_eq!(state.tick_count, 0);
    }

    #[test]
    fn test_state_manager_creation() {
        let (secret_key, public_key) = create_test_keys();
        let manager = StateManager::new(secret_key, public_key);
        assert_eq!(manager.agent_secret_key, secret_key);
        assert_eq!(manager.agent_public_key, public_key);
    }

    #[test]
    fn test_state_kind() {
        assert_eq!(StateManager::state_kind(), 38001);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let (secret_key, public_key) = create_test_keys();
        let manager = StateManager::new(secret_key, public_key);

        let mut content = AgentStateContent::new();
        content.add_goal(Goal::new("goal-1", "Test goal", 1));
        content.add_memory(MemoryEntry::new("observation", "Test memory"));
        content.update_balance(5000);
        content.record_tick(1703000000);

        // Encrypt
        let encrypted = manager.encrypt_state(&content).unwrap();
        assert!(!encrypted.is_empty());

        // Decrypt
        let decrypted = manager.decrypt_state(&encrypted, 1).unwrap();
        assert_eq!(decrypted.goals.len(), 1);
        assert_eq!(decrypted.memory.len(), 1);
        assert_eq!(decrypted.wallet_balance_sats, 5000);
        assert_eq!(decrypted.tick_count, 1);
        assert_eq!(decrypted.last_tick, 1703000000);
    }

    #[test]
    fn test_encrypt_empty_state() {
        let (secret_key, public_key) = create_test_keys();
        let manager = StateManager::new(secret_key, public_key);

        let content = AgentStateContent::new();
        let encrypted = manager.encrypt_state(&content).unwrap();
        assert!(!encrypted.is_empty());

        let decrypted = manager.decrypt_state(&encrypted, 1).unwrap();
        assert_eq!(decrypted.goals.len(), 0);
        assert_eq!(decrypted.memory.len(), 0);
    }

    #[test]
    fn test_decrypt_invalid_version() {
        let (secret_key, public_key) = create_test_keys();
        let manager = StateManager::new(secret_key, public_key);

        let content = AgentStateContent::new();
        let encrypted = manager.encrypt_state(&content).unwrap();

        // Try to decrypt with unsupported version
        let result = manager.decrypt_state(&encrypted, 999);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to decrypt"));
    }
}
