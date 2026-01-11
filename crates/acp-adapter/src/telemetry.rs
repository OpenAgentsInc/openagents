//! APM telemetry hooks for ACP adapter
//!
//! Provides ActionEvent tracking for all tool use events that flow through
//! the ACP adapter, enabling APM (Actions Per Minute) measurement for both
//! Codex Code and Codex interactions.

use agent_client_protocol_schema as acp;
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

/// Action event with timing and success tracking
#[derive(Debug, Clone)]
pub struct ActionEvent {
    pub session_id: String,
    pub action_type: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub duration_ms: u64,
    pub success: bool,
    pub error: Option<String>,
    pub metadata: Option<Value>,
}

impl ActionEvent {
    /// Create a successful action event
    pub fn success(
        session_id: impl Into<String>,
        action_type: impl Into<String>,
        duration_ms: u64,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            action_type: action_type.into(),
            timestamp: chrono::Utc::now(),
            duration_ms,
            success: true,
            error: None,
            metadata: None,
        }
    }

    /// Create a failed action event
    pub fn failure(
        session_id: impl Into<String>,
        action_type: impl Into<String>,
        duration_ms: u64,
        error: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            action_type: action_type.into(),
            timestamp: chrono::Utc::now(),
            duration_ms,
            success: false,
            error: Some(error.into()),
            metadata: None,
        }
    }

    /// Add metadata to the event
    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// APM telemetry tracker for ACP sessions
///
/// Tracks tool use events and emits ActionEvents via an async channel
/// with zero performance overhead (fire-and-forget).
#[derive(Clone)]
pub struct ApmTelemetry {
    session_id: String,
    tx: mpsc::UnboundedSender<ActionEvent>,

    // Track in-flight tool calls by ID for duration measurement
    in_flight: Arc<tokio::sync::RwLock<std::collections::HashMap<String, Instant>>>,
}

impl ApmTelemetry {
    /// Create a new APM telemetry tracker
    pub fn new(session_id: impl Into<String>) -> (Self, mpsc::UnboundedReceiver<ActionEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();

        (
            Self {
                session_id: session_id.into(),
                tx,
                in_flight: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            },
            rx,
        )
    }

    /// Record a tool call start
    pub async fn on_tool_call_start(&self, notification: &acp::SessionNotification) {
        if let acp::SessionUpdate::ToolCall(tool_call) = &notification.update {
            let tool_id = tool_call.tool_call_id.to_string();
            self.in_flight.write().await.insert(tool_id, Instant::now());
        }
    }

    /// Record a tool call completion or failure
    pub async fn on_tool_call_end(&self, notification: &acp::SessionNotification) {
        if let acp::SessionUpdate::ToolCallUpdate(update) = &notification.update {
            let tool_id = update.tool_call_id.to_string();

            // Get the start time if we tracked it
            let start = self.in_flight.write().await.remove(&tool_id);
            let duration_ms = start.map(|s| s.elapsed().as_millis() as u64).unwrap_or(0);

            // Determine action type from tool title
            let action_type = update
                .fields
                .title
                .clone()
                .unwrap_or_else(|| "Unknown".to_string());

            // Determine success/failure from status
            let (success, error) = match update.fields.status {
                Some(acp::ToolCallStatus::Completed) => (true, None),
                Some(acp::ToolCallStatus::Failed) => {
                    // Error message is in raw_output when status is Failed
                    let error_msg = update
                        .fields
                        .raw_output
                        .as_ref()
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    (false, error_msg)
                }
                _ => return, // Only record completed/failed events
            };

            // Create metadata from tool call details
            let metadata = serde_json::json!({
                "tool_call_id": tool_id,
                "title": action_type,
            });

            let event = if success {
                ActionEvent::success(&self.session_id, &action_type, duration_ms)
                    .with_metadata(metadata)
            } else {
                ActionEvent::failure(
                    &self.session_id,
                    &action_type,
                    duration_ms,
                    error.unwrap_or_else(|| "Unknown error".to_string()),
                )
                .with_metadata(metadata)
            };

            // Fire-and-forget send (no performance impact)
            let _ = self.tx.send(event);
        }
    }

    /// Record a message (user or assistant)
    pub fn on_message(&self, notification: &acp::SessionNotification) {
        let message_type = match &notification.update {
            acp::SessionUpdate::UserMessageChunk(_) => "UserMessage",
            acp::SessionUpdate::AgentMessageChunk(_) => "AssistantMessage",
            acp::SessionUpdate::AgentThoughtChunk(_) => "Thinking",
            _ => return,
        };

        // Messages have minimal duration (instant)
        let event = ActionEvent::success(&self.session_id, message_type, 0);

        // Fire-and-forget send
        let _ = self.tx.send(event);
    }

    /// Process a session notification and emit appropriate events
    pub async fn process_notification(&self, notification: &acp::SessionNotification) {
        match &notification.update {
            acp::SessionUpdate::ToolCall(_) => {
                self.on_tool_call_start(notification).await;
            }
            acp::SessionUpdate::ToolCallUpdate(_) => {
                self.on_tool_call_end(notification).await;
            }
            acp::SessionUpdate::UserMessageChunk(_)
            | acp::SessionUpdate::AgentMessageChunk(_)
            | acp::SessionUpdate::AgentThoughtChunk(_) => {
                self.on_message(notification);
            }
            _ => {
                // Ignore other notification types
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_telemetry_creation() {
        let (telemetry, mut rx) = ApmTelemetry::new("test-session");
        assert_eq!(telemetry.session_id, "test-session");

        // Channel should be empty
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_tool_call_tracking() {
        let (telemetry, mut rx) = ApmTelemetry::new("test-session");

        // Simulate tool call start
        let session_id = acp::SessionId::new("test-session");
        let tool_call_id = acp::ToolCallId::new("tool-123");

        let start_notification = acp::SessionNotification::new(
            session_id.clone(),
            acp::SessionUpdate::ToolCall(acp::ToolCall::new(
                tool_call_id.clone(),
                "Read".to_string(),
            )),
        );

        telemetry.process_notification(&start_notification).await;

        // Should not emit event yet
        assert!(rx.try_recv().is_err());

        // Simulate tool call completion
        let end_notification = acp::SessionNotification::new(
            session_id,
            acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                tool_call_id,
                acp::ToolCallUpdateFields::new()
                    .status(acp::ToolCallStatus::Completed)
                    .title("Read".to_string()),
            )),
        );

        telemetry.process_notification(&end_notification).await;

        // Should emit completion event
        let event = rx.try_recv().expect("Should have received event");
        assert_eq!(event.action_type, "Read");
        assert!(event.success);
        assert_eq!(event.session_id, "test-session");
    }

    #[tokio::test]
    async fn test_message_tracking() {
        let (telemetry, mut rx) = ApmTelemetry::new("test-session");

        let session_id = acp::SessionId::new("test-session");
        let notification = acp::SessionNotification::new(
            session_id,
            acp::SessionUpdate::UserMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new("Hello".to_string()),
            ))),
        );

        telemetry.process_notification(&notification).await;

        // Should emit message event
        let event = rx.try_recv().expect("Should have received event");
        assert_eq!(event.action_type, "UserMessage");
        assert!(event.success);
        assert_eq!(event.duration_ms, 0);
    }

    #[tokio::test]
    async fn test_failed_tool_call() {
        let (telemetry, mut rx) = ApmTelemetry::new("test-session");

        let session_id = acp::SessionId::new("test-session");
        let tool_call_id = acp::ToolCallId::new("tool-456");

        // Start
        let start = acp::SessionNotification::new(
            session_id.clone(),
            acp::SessionUpdate::ToolCall(acp::ToolCall::new(
                tool_call_id.clone(),
                "Bash".to_string(),
            )),
        );
        telemetry.process_notification(&start).await;

        // Failure
        let end = acp::SessionNotification::new(
            session_id,
            acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                tool_call_id,
                acp::ToolCallUpdateFields::new()
                    .status(acp::ToolCallStatus::Failed)
                    .title("Bash".to_string())
                    .raw_output(serde_json::json!("Command not found")),
            )),
        );
        telemetry.process_notification(&end).await;

        let event = rx.try_recv().expect("Should have received event");
        assert_eq!(event.action_type, "Bash");
        assert!(!event.success);
        assert_eq!(event.error, Some("Command not found".to_string()));
    }
}
