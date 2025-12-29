//! Agent State Management
//!
//! Fetches, decrypts, encrypts, and publishes agent state (kind:38001).

use anyhow::Result;
use compute::domain::UnifiedIdentity;
use nostr::nip_sa::{AgentState, AgentStateContent, KIND_AGENT_STATE, STATE_D_TAG, STATE_VERSION};
use nostr::{finalize_event, Event, EventTemplate};
use crate::agents::SharedRelay;
use std::time::Duration;
use uuid::Uuid;

/// Manages agent state on Nostr
pub struct StateManager {
    identity: UnifiedIdentity,
    pub relay: SharedRelay,
}

impl StateManager {
    /// Create a new state manager
    pub fn new(identity: UnifiedIdentity, relay: SharedRelay) -> Self {
        Self { identity, relay }
    }

    /// Connect to the relay
    pub async fn connect(&self) -> Result<()> {
        self.relay.connect().await?;
        Ok(())
    }

    /// Fetch the agent's current state from relays
    pub async fn fetch_state(&self) -> Result<Option<AgentStateContent>> {
        let pubkey = self.identity.public_key_hex();

        // Query for the agent's state event
        let filters = vec![serde_json::json!({
            "kinds": [KIND_AGENT_STATE as u64],
            "authors": [pubkey],
            "#d": [STATE_D_TAG],
            "limit": 1
        })];

        // Subscribe and collect events
        let subscription_id = format!("state-fetch-{}", Uuid::new_v4());
        let mut rx = self
            .relay
            .subscribe_with_channel(&subscription_id, &filters)
            .await?;

        // Collect events with timeout
        let mut events: Vec<Event> = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);

        while std::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            match tokio::time::timeout(remaining.max(Duration::from_millis(100)), rx.recv()).await {
                Ok(Some(event)) => events.push(event),
                Ok(None) => break,
                Err(_) => break, // Timeout reached
            }
        }

        if events.is_empty() {
            return Ok(None);
        }

        // Get the most recent event
        let event = events.into_iter().max_by_key(|e| e.created_at).unwrap();

        // Decrypt the state
        let state = self.decrypt_state(&event)?;
        Ok(Some(state.content))
    }

    /// Decrypt a state event
    fn decrypt_state(&self, event: &Event) -> Result<AgentState> {
        // Get version from tags
        let version = event
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "state_version")
            .and_then(|t| t[1].parse::<u32>().ok())
            .unwrap_or(STATE_VERSION);

        // Get public key bytes from hex
        let pubkey_bytes = hex::decode(&event.pubkey)?;

        // Decrypt state
        let state = AgentState::decrypt(
            &event.content,
            self.identity.private_key_bytes(),
            &pubkey_bytes,
            version,
        )?;

        Ok(state)
    }

    /// Publish updated state to relays
    pub async fn publish_state(&self, content: &AgentStateContent) -> Result<String> {
        let state = AgentState::new(content.clone());

        // Get agent's public key bytes
        let pubkey_bytes = hex::decode(&self.identity.public_key_hex())?;

        // Encrypt state to self
        let encrypted = state.encrypt(self.identity.private_key_bytes(), &pubkey_bytes)?;

        let now = chrono::Utc::now().timestamp() as u64;

        // Build event
        let template = EventTemplate {
            created_at: now,
            kind: KIND_AGENT_STATE,
            tags: state.build_tags(),
            content: encrypted,
        };

        let event = finalize_event(&template, self.identity.private_key_bytes())?;
        let event_id = event.id.clone();

        // Publish
        self.relay
            .publish_event(&event, Duration::from_secs(10))
            .await?;

        Ok(event_id)
    }

    /// Get or create initial state
    pub async fn get_or_create_state(&self) -> Result<AgentStateContent> {
        match self.fetch_state().await? {
            Some(state) => Ok(state),
            None => {
                // Create initial state
                let state = AgentStateContent::new();
                self.publish_state(&state).await?;
                Ok(state)
            }
        }
    }
}
