//! NIP-SA Trajectory Publishing
//!
//! This module maps autopilot's TrajectoryCollector events to NIP-SA trajectory events
//! (kinds 38030, 38031) for transparent work records.

use crate::trajectory::{Step, StepType as AutopilotStepType, Trajectory};
use nostr::{
    StepType, TrajectoryEvent, TrajectoryEventContent, TrajectorySession,
    TrajectorySessionContent, TrajectoryVisibility, KIND_TRAJECTORY_EVENT,
    KIND_TRAJECTORY_SESSION,
};
use sha2::{Digest, Sha256};

/// Trajectory publisher for NIP-SA events
pub struct TrajectoryPublisher {
    /// Session ID for this trajectory
    session_id: String,
    /// Tick ID for linking to tick events
    tick_id: String,
}

impl TrajectoryPublisher {
    /// Create a new trajectory publisher
    pub fn new(session_id: impl Into<String>, tick_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            tick_id: tick_id.into(),
        }
    }

    /// Create a trajectory session from autopilot trajectory
    pub fn create_session(
        &self,
        trajectory: &Trajectory,
        visibility: TrajectoryVisibility,
    ) -> TrajectorySession {
        let started_at = trajectory.started_at.timestamp() as u64;
        let ended_at = trajectory.ended_at.map(|t| t.timestamp() as u64);

        let content = TrajectorySessionContent {
            session_id: self.session_id.clone(),
            started_at,
            ended_at,
            model: trajectory.model.clone(),
            total_events: trajectory.steps.len() as u32,
            trajectory_hash: None, // Calculated separately
        };

        TrajectorySession::new(content, &self.tick_id, visibility)
    }

    /// Convert a trajectory step to NIP-SA trajectory event
    pub fn step_to_event(&self, step: &Step, sequence: u32) -> TrajectoryEvent {
        let step_type = map_step_type(&step.step_type);
        let mut content = TrajectoryEventContent::new(step_type);

        // Add step-specific data
        match &step.step_type {
            AutopilotStepType::User { content: text } => {
                content = content.with_data("content", serde_json::json!(text));
            }
            AutopilotStepType::Assistant { content: text } => {
                content = content.with_data("content", serde_json::json!(text));
            }
            AutopilotStepType::Thinking {
                content: text,
                signature,
            } => {
                content = content.with_data("content", serde_json::json!(text));
                if let Some(sig) = signature {
                    content = content.with_data("signature", serde_json::json!(sig));
                }
            }
            AutopilotStepType::ToolCall { tool, tool_id, input } => {
                content = content
                    .with_data("tool", serde_json::json!(tool))
                    .with_data("tool_id", serde_json::json!(tool_id))
                    .with_data("input", input.clone());
            }
            AutopilotStepType::ToolResult {
                tool_id,
                success,
                output,
            } => {
                content = content
                    .with_data("tool_id", serde_json::json!(tool_id))
                    .with_data("success", serde_json::json!(success));
                if let Some(out) = output {
                    content = content.with_data("output", serde_json::json!(out));
                }
            }
            AutopilotStepType::SystemInit { model } => {
                content = content.with_data("model", serde_json::json!(model));
            }
            AutopilotStepType::SystemStatus { status } => {
                content = content.with_data("status", serde_json::json!(status));
            }
            AutopilotStepType::Subagent { agent_id, agent_type, status, summary } => {
                content = content
                    .with_data("agent_id", serde_json::json!(agent_id))
                    .with_data("agent_type", serde_json::json!(agent_type))
                    .with_data("status", serde_json::json!(status))
                    .with_data("summary", serde_json::json!(summary));
            }
        }

        // Add token metrics if available
        if let Some(tokens_in) = step.tokens_in {
            content = content.with_data("tokens_in", serde_json::json!(tokens_in));
        }
        if let Some(tokens_out) = step.tokens_out {
            content = content.with_data("tokens_out", serde_json::json!(tokens_out));
        }
        if let Some(tokens_cached) = step.tokens_cached {
            content = content.with_data("tokens_cached", serde_json::json!(tokens_cached));
        }

        TrajectoryEvent::new(content, &self.session_id, &self.tick_id, sequence)
    }

    /// Convert entire trajectory to NIP-SA events
    pub fn trajectory_to_events(&self, trajectory: &Trajectory) -> Vec<TrajectoryEvent> {
        trajectory
            .steps
            .iter()
            .enumerate()
            .map(|(i, step)| self.step_to_event(step, i as u32))
            .collect()
    }

    /// Calculate trajectory hash from events
    pub fn calculate_hash(&self, events: &[TrajectoryEvent]) -> String {
        let mut hasher = Sha256::new();

        for event in events {
            // Hash session_id, sequence, step_type
            hasher.update(event.session_id.as_bytes());
            hasher.update(&event.sequence.to_le_bytes());
            hasher.update(format!("{:?}", event.content.step_type).as_bytes());

            // Hash the content JSON for deterministic ordering
            if let Ok(json) = event.content.to_json() {
                hasher.update(json.as_bytes());
            }
        }

        let result = hasher.finalize();
        hex::encode(result)
    }

    /// Create session with calculated hash
    pub fn create_session_with_hash(
        &self,
        trajectory: &Trajectory,
        events: &[TrajectoryEvent],
        visibility: TrajectoryVisibility,
    ) -> TrajectorySession {
        let mut session = self.create_session(trajectory, visibility);
        let hash = self.calculate_hash(events);
        session.content.trajectory_hash = Some(hash);
        session
    }

    /// Get trajectory session event kind
    pub fn session_kind() -> u16 {
        KIND_TRAJECTORY_SESSION
    }

    /// Get trajectory event kind
    pub fn event_kind() -> u16 {
        KIND_TRAJECTORY_EVENT
    }
}

/// Map autopilot StepType to NIP-SA StepType
fn map_step_type(step: &AutopilotStepType) -> StepType {
    match step {
        AutopilotStepType::ToolCall { .. } => StepType::ToolUse,
        AutopilotStepType::ToolResult { .. } => StepType::ToolResult,
        AutopilotStepType::Assistant { .. } => StepType::Message,
        AutopilotStepType::Thinking { .. } => StepType::Thinking,
        AutopilotStepType::User { .. } => StepType::Message,
        AutopilotStepType::SystemInit { .. } => StepType::Message,
        AutopilotStepType::SystemStatus { .. } => StepType::Message,
        AutopilotStepType::Subagent { .. } => StepType::Message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_trajectory() -> Trajectory {
        let mut trajectory = Trajectory::new(
            "test prompt".to_string(),
            "claude-3-5-sonnet".to_string(),
            "/test/cwd".to_string(),
            "abc123".to_string(),
            Some("main".to_string()),
        );

        trajectory.session_id = "test-session".to_string();
        trajectory
    }

    #[test]
    fn test_publisher_creation() {
        let publisher = TrajectoryPublisher::new("session-123", "tick-456");
        assert_eq!(publisher.session_id, "session-123");
        assert_eq!(publisher.tick_id, "tick-456");
    }

    #[test]
    fn test_create_session() {
        let trajectory = create_test_trajectory();
        let publisher = TrajectoryPublisher::new("test-session", "tick-1");
        let session = publisher.create_session(&trajectory, TrajectoryVisibility::Public);

        assert_eq!(session.content.session_id, "test-session");
        assert_eq!(session.content.model, "claude-3-5-sonnet");
        assert!(session.content.started_at > 0);
        assert_eq!(session.tick_id, "tick-1");
        assert_eq!(session.visibility, TrajectoryVisibility::Public);
    }

    #[test]
    fn test_step_to_event_tool_call() {
        let publisher = TrajectoryPublisher::new("test-session", "tick-1");
        let step = Step {
            step_id: 1,
            timestamp: Utc::now(),
            step_type: AutopilotStepType::ToolCall {
                tool: "Read".to_string(),
                tool_id: "tool-1".to_string(),
                input: serde_json::json!({"file": "test.rs"}),
            },
            tokens_in: Some(100),
            tokens_out: Some(50),
            tokens_cached: None,
        };

        let event = publisher.step_to_event(&step, 0);

        assert_eq!(event.session_id, "test-session");
        assert_eq!(event.tick_id, "tick-1");
        assert_eq!(event.sequence, 0);
        assert_eq!(event.content.step_type, StepType::ToolUse);
        assert_eq!(
            event.content.data.get("tool").unwrap().as_str().unwrap(),
            "Read"
        );
        assert_eq!(
            event.content.data.get("tokens_in").unwrap().as_u64().unwrap(),
            100
        );
    }

    #[test]
    fn test_step_to_event_thinking() {
        let publisher = TrajectoryPublisher::new("test-session", "tick-1");
        let step = Step {
            step_id: 1,
            timestamp: Utc::now(),
            step_type: AutopilotStepType::Thinking {
                content: "Let me think...".to_string(),
                signature: Some("sig123".to_string()),
            },
            tokens_in: None,
            tokens_out: None,
            tokens_cached: None,
        };

        let event = publisher.step_to_event(&step, 0);

        assert_eq!(event.content.step_type, StepType::Thinking);
        assert_eq!(
            event
                .content
                .data
                .get("content")
                .unwrap()
                .as_str()
                .unwrap(),
            "Let me think..."
        );
        assert_eq!(
            event
                .content
                .data
                .get("signature")
                .unwrap()
                .as_str()
                .unwrap(),
            "sig123"
        );
    }

    #[test]
    fn test_trajectory_to_events() {
        let mut trajectory = create_test_trajectory();
        trajectory.add_step(AutopilotStepType::ToolCall {
            tool: "Read".to_string(),
            tool_id: "tool-1".to_string(),
            input: serde_json::json!({}),
        });
        trajectory.add_step(AutopilotStepType::ToolResult {
            tool_id: "tool-1".to_string(),
            success: true,
            output: Some("file contents".to_string()),
        });

        let publisher = TrajectoryPublisher::new("test-session", "tick-1");
        let events = publisher.trajectory_to_events(&trajectory);

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence, 0);
        assert_eq!(events[1].sequence, 1);
        assert_eq!(events[0].content.step_type, StepType::ToolUse);
        assert_eq!(events[1].content.step_type, StepType::ToolResult);
    }

    #[test]
    fn test_calculate_hash() {
        let publisher = TrajectoryPublisher::new("test-session", "tick-1");
        let content1 = TrajectoryEventContent::new(StepType::ToolUse);
        let content2 = TrajectoryEventContent::new(StepType::ToolResult);

        let event1 = TrajectoryEvent::new(content1, "test-session", "tick-1", 0);
        let event2 = TrajectoryEvent::new(content2, "test-session", "tick-1", 1);

        let hash = publisher.calculate_hash(&[event1, event2]);

        assert_eq!(hash.len(), 64); // SHA256 hex = 64 chars
    }

    #[test]
    fn test_create_session_with_hash() {
        let mut trajectory = create_test_trajectory();
        trajectory.add_step(AutopilotStepType::ToolCall {
            tool: "Read".to_string(),
            tool_id: "tool-1".to_string(),
            input: serde_json::json!({}),
        });

        let publisher = TrajectoryPublisher::new("test-session", "tick-1");
        let events = publisher.trajectory_to_events(&trajectory);
        let session =
            publisher.create_session_with_hash(&trajectory, &events, TrajectoryVisibility::Public);

        assert!(session.content.trajectory_hash.is_some());
        let hash = session.content.trajectory_hash.unwrap();
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_event_kinds() {
        assert_eq!(TrajectoryPublisher::session_kind(), 38030);
        assert_eq!(TrajectoryPublisher::event_kind(), 38031);
    }
}
