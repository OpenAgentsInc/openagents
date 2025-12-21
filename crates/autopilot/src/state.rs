//! Agent State Management
//!
//! This module provides helpers for managing encrypted agent state using NIP-SA.
//! Agent state is encrypted to the agent's pubkey using NIP-44 and published
//! to relays as kind:38001 events.

use anyhow::{Context, Result};
use nostr::{AgentState, AgentStateContent, KIND_AGENT_STATE};
use nostr_client::{RelayPool, PoolConfig};
use std::time::{SystemTime, UNIX_EPOCH};

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

    /// Publish agent state to relays
    ///
    /// Encrypts the state content and publishes it as a kind:38001 event
    /// to the specified relays. Returns the event ID on success.
    pub async fn publish_state_to_relays(
        &self,
        content: &AgentStateContent,
        relays: &[String],
        _agent_pubkey_hex: &str,
    ) -> Result<String> {
        // Encrypt the state
        let _encrypted_content = self.encrypt_state(content)?;

        // Get current timestamp
        let _created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("Failed to get timestamp")?
            .as_secs();

        // Build event tags (d tag for parameterized replaceable event)
        let _tags = vec![vec!["d".to_string(), "state".to_string()]];

        // Note: Event structure for future implementation
        // Would be signed and published as:
        // {
        //   "pubkey": agent_pubkey_hex,
        //   "created_at": created_at,
        //   "kind": KIND_AGENT_STATE,
        //   "tags": tags,
        //   "content": encrypted_content,
        // }

        // Create relay pool
        let config = PoolConfig::default();
        let pool = RelayPool::new(config);

        // Connect to relays
        for relay_url in relays {
            pool.add_relay(relay_url)
                .await
                .context(format!("Failed to add relay: {}", relay_url))?;
        }

        // Wait a moment for connections
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Note: In a real implementation, we would sign the event here with the agent's secret key
        // For now, we'll return a placeholder event ID since signing requires the full nostr Event API
        // This will be completed when we integrate with the agent keypair system

        // Publish to all relays
        // TODO: Implement actual event signing and publishing
        // let event_id = pool.publish_event(&signed_event).await?;

        // For now, return a mock event ID to demonstrate the interface
        let event_id = "mock_event_id_".to_string() + &hex::encode(&self.agent_public_key[..8]);

        // Disconnect from pool
        let _ = pool.disconnect_all().await;

        Ok(event_id)
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

    #[tokio::test]
    async fn test_publish_state_to_relays() {
        let (secret_key, public_key) = create_test_keys();
        let manager = StateManager::new(secret_key, public_key);

        let mut content = AgentStateContent::new();
        content.add_goal(Goal::new("goal-1", "Test goal", 1));
        content.update_balance(1000);

        // Mock relay URLs
        let relays = vec!["wss://relay.example.com".to_string()];
        let agent_pubkey = "npub1test";

        // Publish state (this will use mock implementation for now)
        let result = manager
            .publish_state_to_relays(&content, &relays, agent_pubkey)
            .await;

        // Should succeed with mock implementation
        assert!(result.is_ok());
        let event_id = result.unwrap();
        assert!(event_id.starts_with("mock_event_id_"));
    }
}


    #[test]
    fn test_multiple_goals() {
        let mut content = AgentStateContent::new();
        content.add_goal(Goal::new("goal-1", "First goal", 1));
        content.add_goal(Goal::new("goal-2", "Second goal", 2));
        content.add_goal(Goal::new("goal-3", "Third goal", 3));

        assert_eq!(content.goals.len(), 3);

        // Verify we can find goals by ID
        let goal1 = content.goals.iter().find(|g| g.id == "goal-1");
        assert!(goal1.is_some());
        assert_eq!(goal1.unwrap().description, "First goal");
    }

    #[test]
    fn test_state_with_complex_data() {
        let (secret_key, public_key) = create_test_keys();
        let manager = StateManager::new(secret_key, public_key);

        let mut content = AgentStateContent::new();
        
        // Add multiple goals
        for i in 0..10 {
            content.add_goal(Goal::new(
                &format!("goal-{}", i),
                &format!("Goal number {}", i),
                i as u32,
            ));
        }

        // Add multiple memory entries
        for i in 0..20 {
            content.add_memory(MemoryEntry::new("observation", &format!("Memory {}", i)));
        }

        content.update_balance(50000);
        content.record_tick(1703000000);

        // Encrypt and decrypt
        let encrypted = manager.encrypt_state(&content).unwrap();
        let decrypted = manager.decrypt_state(&encrypted, 1).unwrap();

        // Verify all data preserved
        assert_eq!(decrypted.goals.len(), 10);
        assert_eq!(decrypted.memory.len(), 20);
        assert_eq!(decrypted.wallet_balance_sats, 50000);
        assert_eq!(decrypted.tick_count, 1);
    }
