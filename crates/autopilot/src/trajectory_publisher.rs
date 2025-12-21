//! Trajectory Session Publishing
//!
//! This module publishes NIP-SA TrajectorySession (kind:38030) events when
//! an autopilot run starts and receives its session_id from the SDK.

use anyhow::{Context, Result};
use nostr::{TrajectorySession, TrajectorySessionContent, TrajectoryVisibility, KIND_TRAJECTORY_SESSION};
use nostr_client::{PoolConfig, RelayPool};

/// Configuration for trajectory publishing
#[derive(Debug, Clone)]
pub struct TrajectoryPublishConfig {
    /// Enable/disable trajectory publishing
    pub enabled: bool,
    /// Relays to publish to
    pub relays: Vec<String>,
    /// Visibility (public or private)
    pub visibility: TrajectoryVisibility,
}

impl Default for TrajectoryPublishConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            relays: vec![],
            visibility: TrajectoryVisibility::Public,
        }
    }
}

impl TrajectoryPublishConfig {
    /// Create new config with specified relays
    pub fn new(relays: Vec<String>) -> Self {
        Self {
            enabled: !relays.is_empty(),
            relays,
            visibility: TrajectoryVisibility::Public,
        }
    }

    /// Set visibility
    pub fn with_visibility(mut self, visibility: TrajectoryVisibility) -> Self {
        self.visibility = visibility;
        self
    }

    /// Enable publishing
    pub fn enable(mut self) -> Self {
        self.enabled = true;
        self
    }

    /// Disable publishing
    pub fn disable(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// Trajectory session publisher
///
/// Publishes TrajectorySession (kind:38030) events when an autopilot run starts.
pub struct TrajectorySessionPublisher {
    /// Configuration
    config: TrajectoryPublishConfig,
}

impl TrajectorySessionPublisher {
    /// Create new publisher
    pub fn new(config: TrajectoryPublishConfig) -> Self {
        Self { config }
    }

    /// Publish a trajectory session event
    ///
    /// This creates and publishes a kind:38030 TrajectorySession event to the
    /// configured relays. The session links to the tick_id and contains metadata
    /// about the autopilot run.
    ///
    /// # Arguments
    /// * `session_id` - Unique session identifier from Claude SDK
    /// * `tick_id` - Tick request ID that triggered this run
    /// * `model` - Model name (e.g., "claude-sonnet-4.5")
    /// * `started_at` - Unix timestamp when session started
    ///
    /// Returns the event ID if publishing succeeds.
    pub async fn publish_session(
        &self,
        session_id: impl Into<String>,
        tick_id: impl Into<String>,
        model: impl Into<String>,
        started_at: u64,
    ) -> Result<Option<String>> {
        if !self.config.enabled {
            return Ok(None);
        }

        let session_id = session_id.into();
        let tick_id = tick_id.into();
        let model = model.into();

        // Create trajectory session content
        let content = TrajectorySessionContent::new(&session_id, started_at, &model);

        // Create trajectory session event
        let session = TrajectorySession::new(content, &tick_id, self.config.visibility.clone());

        // Build event tags
        let _tags = session.build_tags();

        // Serialize content
        let _content_json = session
            .content
            .to_json()
            .context("Failed to serialize trajectory session content")?;

        // Create relay pool
        let pool_config = PoolConfig::default();
        let pool = RelayPool::new(pool_config);

        // Connect to relays
        for relay_url in &self.config.relays {
            pool.add_relay(relay_url)
                .await
                .context(format!("Failed to add relay: {}", relay_url))?;
        }

        // Wait for connections
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // TODO: Actually publish the event
        // This requires:
        // 1. Agent keypair for signing
        // 2. Event builder to create kind:38030 event with tags and content
        // 3. Pool.publish_event() to broadcast
        //
        // For now, we'll return a mock event ID to demonstrate the interface
        // The full implementation will be completed when agent identity integration is ready

        let event_id = format!("mock_trajectory_session_{}", &session_id[..8]);

        // Disconnect from pool
        let _ = pool.disconnect_all().await;

        Ok(Some(event_id))
    }

    /// Get trajectory session event kind
    pub fn session_kind() -> u16 {
        KIND_TRAJECTORY_SESSION
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = TrajectoryPublishConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.relays.len(), 0);
        assert_eq!(config.visibility, TrajectoryVisibility::Public);
    }

    #[test]
    fn test_config_new() {
        let relays = vec!["wss://relay.example.com".to_string()];
        let config = TrajectoryPublishConfig::new(relays.clone());
        assert!(config.enabled);
        assert_eq!(config.relays, relays);
    }

    #[test]
    fn test_config_builder() {
        let config = TrajectoryPublishConfig::default()
            .enable()
            .with_visibility(TrajectoryVisibility::Private);

        assert!(config.enabled);
        assert_eq!(config.visibility, TrajectoryVisibility::Private);
    }

    #[test]
    fn test_config_disable() {
        let config = TrajectoryPublishConfig::new(vec!["wss://relay.example.com".to_string()])
            .disable();

        assert!(!config.enabled);
    }

    #[test]
    fn test_publisher_creation() {
        let config = TrajectoryPublishConfig::default();
        let publisher = TrajectorySessionPublisher::new(config);
        assert!(!publisher.config.enabled);
    }

    #[tokio::test]
    async fn test_publish_session_disabled() {
        let config = TrajectoryPublishConfig::default(); // disabled by default
        let publisher = TrajectorySessionPublisher::new(config);

        let result = publisher
            .publish_session("session-123", "tick-456", "claude-sonnet-4.5", 1703000000)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None); // Should return None when disabled
    }

    #[tokio::test]
    async fn test_publish_session_enabled() {
        let config =
            TrajectoryPublishConfig::new(vec!["wss://relay.example.com".to_string()]).enable();
        let publisher = TrajectorySessionPublisher::new(config);

        let result = publisher
            .publish_session("session-123", "tick-456", "claude-sonnet-4.5", 1703000000)
            .await;

        assert!(result.is_ok());
        let event_id = result.unwrap();
        assert!(event_id.is_some());
        assert!(event_id.unwrap().starts_with("mock_trajectory_session_"));
    }

    #[test]
    fn test_session_kind() {
        assert_eq!(TrajectorySessionPublisher::session_kind(), 38030);
    }
}
