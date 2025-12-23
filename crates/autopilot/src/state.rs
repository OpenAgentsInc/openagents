//! Agent State Management
//!
//! This module provides helpers for managing encrypted agent state using NIP-SA.
//! Agent state is encrypted to the agent's pubkey using NIP-44 and published
//! to relays as kind:38001 events.

use anyhow::{Context, Result};
use nostr::{AgentState, AgentStateContent, KIND_AGENT_STATE};
use nostr_client::{RelayPool, PoolConfig};
use std::time::{SystemTime, UNIX_EPOCH};
use wallet::core::UnifiedIdentity;
use std::sync::Arc;

/// Agent state manager
pub struct StateManager {
    /// Agent secret key for encryption/decryption
    agent_secret_key: [u8; 32],
    /// Agent public key
    agent_public_key: [u8; 33],
    /// Optional identity for signing events
    identity: Option<Arc<UnifiedIdentity>>,
}

impl StateManager {
    /// Create a new state manager
    pub fn new(agent_secret_key: [u8; 32], agent_public_key: [u8; 33]) -> Self {
        Self {
            agent_secret_key,
            agent_public_key,
            identity: None,
        }
    }

    /// Create a new state manager with identity for signing
    pub fn with_identity(
        agent_secret_key: [u8; 32],
        agent_public_key: [u8; 33],
        identity: Arc<UnifiedIdentity>,
    ) -> Self {
        Self {
            agent_secret_key,
            agent_public_key,
            identity: Some(identity),
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

        // Check if we have identity for signing
        let identity = match &self.identity {
            Some(id) => id,
            None => {
                // No identity - cannot publish state
                // Per d-012 (No Stubs), return an error instead of a mock event ID
                let _ = pool.disconnect_all().await;
                return Err(anyhow::anyhow!(
                    "Cannot publish agent state: No identity configured. Use StateManager::with_identity() to provide an identity for signing."
                ));
            }
        };

        // Build Nostr event template
        let template = nostr::EventTemplate {
            created_at: _created_at,
            kind: KIND_AGENT_STATE,
            tags: _tags,
            content: _encrypted_content.clone(),
        };

        // Sign event
        let event = identity
            .sign_event(template)
            .context("Failed to sign agent state event")?;

        let event_id = event.id.clone();

        // Publish to all relays
        let publish_result = pool.publish(&event).await;

        match publish_result {
            Ok(results) => {
                let success_count = results.iter().filter(|r| r.accepted).count();
                let total_count = results.len();

                if success_count > 0 {
                    eprintln!(
                        "✓ Published agent state {} to {}/{} relays",
                        event_id,
                        success_count,
                        total_count
                    );
                } else {
                    eprintln!(
                        "⚠ Failed to publish agent state {} to any relays",
                        event_id
                    );
                }
            }
            Err(e) => {
                eprintln!("✗ Failed to publish agent state: {}", e);
            }
        }

        // Disconnect from pool
        let _ = pool.disconnect_all().await;

        Ok(event_id)
    }

    /// Fetch agent state from relays
    ///
    /// Queries relays for the most recent kind:38001 event by the agent's pubkey,
    /// decrypts it, and returns the state content.
    ///
    /// Returns None if no state is found on the relays.
    pub async fn fetch_state_from_relays(
        &self,
        relays: &[String],
        agent_pubkey_hex: &str,
    ) -> Result<Option<AgentStateContent>> {
        use serde_json::json;

        // Create relay pool
        let config = PoolConfig::default();
        let pool = RelayPool::new(config);

        // Connect to relays
        for relay_url in relays {
            pool.add_relay(relay_url)
                .await
                .context(format!("Failed to add relay: {}", relay_url))?;
        }

        // Wait for connections
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Create filter for kind:38001 events by agent pubkey
        let filter = json!({
            "kinds": [KIND_AGENT_STATE],
            "authors": [agent_pubkey_hex],
            "limit": 1  // Only fetch most recent
        });

        // Subscribe to fetch events
        let mut rx = pool
            .subscribe("state-fetch", &[filter])
            .await
            .context("Failed to subscribe to state events")?;

        // Wait for first event with timeout
        let event = tokio::time::timeout(
            tokio::time::Duration::from_secs(5),
            rx.recv()
        ).await;

        // Disconnect from pool
        let _ = pool.disconnect_all().await;

        // Process the event if we got one
        if let Ok(Some(event)) = event {
            // Extract encrypted content from event (content is already a String)
            let encrypted_content = &event.content;

            // Determine version from event tags (d tag contains "state")
            // For now, assume version 1
            let version = 1;

            // Decrypt the state
            let content = self.decrypt_state(encrypted_content, version)?;
            Ok(Some(content))
        } else {
            // No event found or timeout
            Ok(None)
        }
    }

    /// Update agent state on relays
    ///
    /// Fetches current state from relays, applies the provided update function,
    /// then publishes the updated state back to relays.
    ///
    /// If no existing state is found, starts with an empty state.
    pub async fn update_state_on_relays<F>(
        &self,
        relays: &[String],
        agent_pubkey_hex: &str,
        update_fn: F,
    ) -> Result<String>
    where
        F: FnOnce(&mut AgentStateContent),
    {
        // Fetch current state
        let mut content = self
            .fetch_state_from_relays(relays, agent_pubkey_hex)
            .await?
            .unwrap_or_else(AgentStateContent::new);

        // Apply update
        update_fn(&mut content);

        // Publish updated state
        self.publish_state_to_relays(&content, relays, agent_pubkey_hex)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::{Goal, MemoryEntry};

    fn create_test_keys() -> ([u8; 32], [u8; 33]) {
        // Use a fixed deterministic keypair for testing
        // This is a valid secp256k1 private key
        let secret_key: [u8; 32] = [
            0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
            0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
            0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
            0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
        ];

        // Corresponding compressed public key (02 prefix + x-coordinate)
        let public_key: [u8; 33] = [
            0x02, 0x50, 0x92, 0x9b, 0x74, 0xc1, 0xa0, 0x49,
            0x54, 0xb7, 0x8b, 0x4b, 0x60, 0x35, 0xe9, 0x7a,
            0x5e, 0x07, 0x8a, 0x5a, 0x0f, 0x28, 0xec, 0x96,
            0xd5, 0x47, 0xbf, 0xee, 0x9a, 0xce, 0x80, 0x3a,
            0xc0,
        ];

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

    #[tokio::test]
    #[ignore] // Requires real relay infrastructure
    async fn test_fetch_state_from_relays_no_state() {
        let (secret_key, public_key) = create_test_keys();
        let manager = StateManager::new(secret_key, public_key);

        // Mock relay URLs
        let relays = vec!["wss://relay.example.com".to_string()];
        let agent_pubkey = "npub1test";

        // Try to fetch (will timeout since no real relay)
        let result = manager
            .fetch_state_from_relays(&relays, agent_pubkey)
            .await;

        // Should succeed with None (timeout)
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    #[ignore] // Requires real relay infrastructure
    async fn test_update_state_on_relays() {
        let (secret_key, public_key) = create_test_keys();
        let manager = StateManager::new(secret_key, public_key);

        // Mock relay URLs
        let relays = vec!["wss://relay.example.com".to_string()];
        let agent_pubkey = "npub1test";

        // Update state (will start with empty state since no relay data)
        let result = manager
            .update_state_on_relays(&relays, agent_pubkey, |state| {
                state.add_goal(Goal::new("goal-1", "Test goal", 1));
                state.update_balance(5000);
            })
            .await;

        // Should succeed with mock implementation
        assert!(result.is_ok());
        let event_id = result.unwrap();
        assert!(event_id.starts_with("mock_event_id_"));
    }
}


