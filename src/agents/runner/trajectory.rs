//! Trajectory Publisher
//!
//! Publishes transparent execution records for agent ticks.
//! Trajectories enable:
//! - Verifiable execution history
//! - Debugging and analysis
//! - Trust and transparency for agent actions

use crate::agents::SharedRelay;
use anyhow::Result;
use openagents_runtime::UnifiedIdentity;
use nostr::nip_sa::{
    KIND_TRAJECTORY_EVENT, KIND_TRAJECTORY_SESSION, StepType, TrajectoryEvent,
    TrajectoryEventContent, TrajectorySession, TrajectorySessionContent, TrajectoryVisibility,
};
use nostr::{Event, EventTemplate, finalize_event};
use std::time::Duration;

/// Trajectory publisher for agent tick execution
pub struct TrajectoryPublisher {
    identity: UnifiedIdentity,
    relay: SharedRelay,
    /// Current session ID (set when session starts)
    current_session_id: Option<String>,
    /// Current tick ID
    current_tick_id: Option<String>,
    /// Sequence counter for events within session
    sequence: u32,
    /// Collected event JSONs for hash calculation
    event_jsons: Vec<String>,
    /// Session start timestamp
    session_start: u64,
}

impl TrajectoryPublisher {
    /// Create a new trajectory publisher
    pub fn new(identity: UnifiedIdentity, relay: SharedRelay) -> Self {
        Self {
            identity,
            relay,
            current_session_id: None,
            current_tick_id: None,
            sequence: 0,
            event_jsons: Vec::new(),
            session_start: 0,
        }
    }

    /// Start a new trajectory session for a tick
    pub async fn start_session(&mut self, tick_id: &str, model: &str) -> Result<String> {
        let now = chrono::Utc::now().timestamp() as u64;
        let session_id = format!("session-{}-{}", tick_id, now);

        self.current_session_id = Some(session_id.clone());
        self.current_tick_id = Some(tick_id.to_string());
        self.sequence = 0;
        self.event_jsons.clear();
        self.session_start = now;

        // Create initial session content (will be updated at end)
        let content = TrajectorySessionContent::new(&session_id, now, model);

        let session = TrajectorySession::new(content, tick_id, TrajectoryVisibility::Public);

        // Build and publish session event
        let template = EventTemplate {
            created_at: now,
            kind: KIND_TRAJECTORY_SESSION,
            tags: session.build_tags(),
            content: session.content.to_json()?,
        };

        let event = finalize_event(&template, self.identity.private_key_bytes())?;
        self.relay
            .publish_event(&event, Duration::from_secs(10))
            .await?;

        tracing::debug!("Started trajectory session: {}", session_id);
        Ok(session_id)
    }

    /// Record an observation step
    pub async fn record_observation(&mut self, observation: &Event) -> Result<()> {
        let content = TrajectoryEventContent::new(StepType::Message)
            .with_data("subtype", serde_json::json!("observation"))
            .with_data("event_kind", serde_json::json!(observation.kind))
            .with_data("event_id", serde_json::json!(&observation.id))
            .with_data("pubkey", serde_json::json!(&observation.pubkey))
            .with_data(
                "content_preview",
                serde_json::json!(observation.content.chars().take(100).collect::<String>()),
            );

        self.publish_trajectory_event(content).await
    }

    /// Record multiple observations
    pub async fn record_observations(&mut self, observations: &[Event]) -> Result<()> {
        for obs in observations {
            self.record_observation(obs).await?;
        }
        Ok(())
    }

    /// Record the reasoning/thinking step (redacted for privacy)
    pub async fn record_thinking(&mut self, reasoning: &str) -> Result<()> {
        let content = TrajectoryEventContent::new(StepType::Thinking)
            .with_data("content", serde_json::json!(reasoning));

        // Redact sensitive content before publishing
        let redacted = content.redact_sensitive();
        self.publish_trajectory_event(redacted).await
    }

    /// Record a tool use step
    pub async fn record_tool_use(
        &mut self,
        tool_name: &str,
        input: serde_json::Value,
    ) -> Result<()> {
        let content = TrajectoryEventContent::new(StepType::ToolUse)
            .with_data("tool", serde_json::json!(tool_name))
            .with_data("input", input);

        // Redact any sensitive data
        let redacted = content.redact_sensitive();
        self.publish_trajectory_event(redacted).await
    }

    /// Record a tool result step
    pub async fn record_tool_result(
        &mut self,
        tool_name: &str,
        output: serde_json::Value,
        success: bool,
    ) -> Result<()> {
        let content = TrajectoryEventContent::new(StepType::ToolResult)
            .with_data("tool", serde_json::json!(tool_name))
            .with_data("output", output)
            .with_data("success", serde_json::json!(success));

        // Redact any sensitive data
        let redacted = content.redact_sensitive();
        self.publish_trajectory_event(redacted).await
    }

    /// Record an action taken by the agent
    pub async fn record_action(
        &mut self,
        action_type: &str,
        details: serde_json::Value,
    ) -> Result<()> {
        let content = TrajectoryEventContent::new(StepType::Message)
            .with_data("subtype", serde_json::json!("action"))
            .with_data("action_type", serde_json::json!(action_type))
            .with_data("details", details);

        self.publish_trajectory_event(content).await
    }

    /// Record a message (general purpose)
    pub async fn record_message(&mut self, message: &str) -> Result<()> {
        let content = TrajectoryEventContent::new(StepType::Message)
            .with_data("content", serde_json::json!(message));

        self.publish_trajectory_event(content).await
    }

    /// End the trajectory session and publish final summary
    ///
    /// Returns the trajectory hash for verification
    pub async fn end_session(&mut self) -> Result<String> {
        let session_id = self
            .current_session_id
            .take()
            .ok_or_else(|| anyhow::anyhow!("No active session to end"))?;

        let tick_id = self
            .current_tick_id
            .take()
            .ok_or_else(|| anyhow::anyhow!("No tick ID set"))?;

        let now = chrono::Utc::now().timestamp() as u64;

        // Calculate trajectory hash from all events
        let trajectory_hash = if !self.event_jsons.is_empty() {
            TrajectorySessionContent::calculate_hash(&self.event_jsons)?
        } else {
            // Empty trajectory - use placeholder hash
            "0".repeat(64)
        };

        // Create final session content with hash
        let content = TrajectorySessionContent::new(&session_id, self.session_start, "claude")
            .with_end_time(now)
            .with_total_events(self.sequence)
            .with_hash(&trajectory_hash);

        let session = TrajectorySession::new(content, &tick_id, TrajectoryVisibility::Public);

        // Build and publish updated session event
        let template = EventTemplate {
            created_at: now,
            kind: KIND_TRAJECTORY_SESSION,
            tags: session.build_tags(),
            content: session.content.to_json()?,
        };

        let event = finalize_event(&template, self.identity.private_key_bytes())?;
        self.relay
            .publish_event(&event, Duration::from_secs(10))
            .await?;

        tracing::debug!(
            "Ended trajectory session: {} with {} events, hash: {}",
            session_id,
            self.sequence,
            &trajectory_hash[..16]
        );

        // Reset state
        self.sequence = 0;
        self.event_jsons.clear();

        Ok(trajectory_hash)
    }

    /// Publish a trajectory event and track for hash calculation
    async fn publish_trajectory_event(&mut self, content: TrajectoryEventContent) -> Result<()> {
        let session_id = self
            .current_session_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No active session"))?
            .clone();

        let tick_id = self
            .current_tick_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No tick ID set"))?
            .clone();

        self.sequence += 1;
        let sequence = self.sequence;

        let event = TrajectoryEvent::new(content.clone(), &session_id, &tick_id, sequence);

        let now = chrono::Utc::now().timestamp() as u64;

        // Build and publish event
        let template = EventTemplate {
            created_at: now,
            kind: KIND_TRAJECTORY_EVENT,
            tags: event.build_tags(),
            content: content.to_json()?,
        };

        // Store JSON for hash calculation
        self.event_jsons.push(content.to_json()?);

        let nostr_event = finalize_event(&template, self.identity.private_key_bytes())?;
        self.relay
            .publish_event(&nostr_event, Duration::from_secs(10))
            .await?;

        tracing::trace!(
            "Published trajectory event #{} for session {}",
            sequence,
            session_id
        );

        Ok(())
    }

    /// Check if there's an active session
    pub fn has_active_session(&self) -> bool {
        self.current_session_id.is_some()
    }

    /// Get the current session ID if active
    pub fn current_session_id(&self) -> Option<&str> {
        self.current_session_id.as_deref()
    }
}
