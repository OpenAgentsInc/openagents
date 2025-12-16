//! Internal bridge for translating events to ChatUpdate.
//!
//! This module handles the conversion of:
//! - `SessionEvent` -> `ChatUpdate`
//! - `PermissionEvent` -> `ChatUpdate`

use crate::update::{ChatUpdate, MessageRole, SessionStatus};
use coder_domain::ids::{MessageId, SessionId, ThreadId};
use coder_permission::PermissionEvent;
use coder_session::SessionEvent;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::debug;

/// Translates session and permission events to ChatUpdate.
pub struct Bridge {
    /// Session ID being bridged.
    session_id: SessionId,
    /// Thread ID for this session.
    thread_id: ThreadId,
    /// Channel to send ChatUpdate to consumers.
    update_tx: mpsc::UnboundedSender<ChatUpdate>,
    /// Tracks tool execution start times for duration calculation.
    tool_start_times: std::collections::HashMap<String, Instant>,
}

impl Bridge {
    /// Create a new bridge for a session.
    pub fn new(
        session_id: SessionId,
        thread_id: ThreadId,
        update_tx: mpsc::UnboundedSender<ChatUpdate>,
    ) -> Self {
        Self {
            session_id,
            thread_id,
            update_tx,
            tool_start_times: std::collections::HashMap::new(),
        }
    }

    /// Handle a session event and emit ChatUpdate(s).
    pub fn handle_session_event(&mut self, event: SessionEvent) {
        match event {
            SessionEvent::StatusChanged { session_id, status } => {
                let update_status = match status {
                    coder_session::SessionStatus::Idle => SessionStatus::Idle,
                    coder_session::SessionStatus::Busy => SessionStatus::Processing,
                    coder_session::SessionStatus::WaitingForPermission => {
                        SessionStatus::WaitingForPermission
                    }
                    coder_session::SessionStatus::Retrying { .. } => SessionStatus::Processing,
                    coder_session::SessionStatus::Error => SessionStatus::Error,
                };
                self.emit(ChatUpdate::SessionStatusChanged {
                    session_id,
                    status: update_status,
                });
            }

            SessionEvent::MessageStarted {
                session_id,
                message_id,
            } => {
                self.emit(ChatUpdate::MessageStarted {
                    session_id,
                    message_id,
                    role: MessageRole::Assistant,
                });
            }

            SessionEvent::TextDelta {
                session_id,
                message_id,
                delta,
            } => {
                self.emit(ChatUpdate::TextDelta {
                    session_id,
                    message_id,
                    delta,
                });
            }

            SessionEvent::ToolStarted {
                session_id,
                message_id,
                tool_name,
                tool_call_id,
            } => {
                // Track start time for duration calculation
                self.tool_start_times
                    .insert(tool_call_id.clone(), Instant::now());

                self.emit(ChatUpdate::ToolStarted {
                    session_id,
                    message_id,
                    tool_call_id,
                    tool_name,
                });
            }

            SessionEvent::ToolCompleted {
                session_id,
                message_id: _,
                tool_call_id,
                success,
            } => {
                // Calculate duration
                let duration_ms = self
                    .tool_start_times
                    .remove(&tool_call_id)
                    .map(|start| start.elapsed().as_millis() as u64)
                    .unwrap_or(0);

                self.emit(ChatUpdate::ToolCompleted {
                    session_id,
                    tool_call_id,
                    output: String::new(), // Output provided separately if needed
                    is_error: !success,
                    duration_ms,
                });
            }

            SessionEvent::MessageCompleted {
                session_id,
                message_id,
                finish_reason,
            } => {
                self.emit(ChatUpdate::MessageCompleted {
                    session_id,
                    message_id,
                    finish_reason,
                });
            }

            SessionEvent::Error {
                session_id,
                message_id: _,
                error,
            } => {
                self.emit(ChatUpdate::Error {
                    session_id,
                    message: error,
                    code: None,
                    recoverable: false, // Assume not recoverable; can be refined later
                });
            }
        }
    }

    /// Handle a permission event and emit ChatUpdate(s).
    pub fn handle_permission_event(&mut self, event: PermissionEvent) {
        match event {
            PermissionEvent::RequestPending(request) => {
                let session_id = request.session_id;
                let permission_id = request.id;

                self.emit(ChatUpdate::PermissionRequired {
                    session_id,
                    permission_id,
                    request,
                });

                // Also update status
                self.emit(ChatUpdate::SessionStatusChanged {
                    session_id,
                    status: SessionStatus::WaitingForPermission,
                });
            }

            PermissionEvent::RequestResponded {
                session_id,
                permission_id,
                response,
            } => {
                let granted = matches!(
                    response,
                    coder_permission::Response::Once | coder_permission::Response::Always
                );

                self.emit(ChatUpdate::PermissionResolved {
                    session_id,
                    permission_id,
                    granted,
                });

                // Resume processing if granted
                if granted {
                    self.emit(ChatUpdate::SessionStatusChanged {
                        session_id,
                        status: SessionStatus::Processing,
                    });
                }
            }
        }
    }

    /// Emit a session started update.
    pub fn emit_session_started(&self) {
        self.emit(ChatUpdate::SessionStarted {
            session_id: self.session_id,
            thread_id: self.thread_id,
        });
    }

    /// Emit a session ended update.
    pub fn emit_session_ended(&self, success: bool, error: Option<String>) {
        self.emit(ChatUpdate::SessionEnded {
            session_id: self.session_id,
            success,
            error,
        });
    }

    /// Emit agent info update.
    pub fn emit_agent_info(&self, agent_id: &str, model_id: &str, provider_id: &str) {
        self.emit(ChatUpdate::AgentInfo {
            session_id: self.session_id,
            agent_id: agent_id.to_string(),
            model_id: model_id.to_string(),
            provider_id: provider_id.to_string(),
        });
    }

    /// Emit a usage update.
    pub fn emit_usage(&self, total_tokens: u64, cost_usd: f64) {
        self.emit(ChatUpdate::UsageUpdate {
            session_id: self.session_id,
            total_tokens,
            cost_usd,
        });
    }

    /// Emit a tool input delta.
    pub fn emit_tool_input_delta(&self, tool_call_id: &str, delta: &str) {
        self.emit(ChatUpdate::ToolInputDelta {
            session_id: self.session_id,
            tool_call_id: tool_call_id.to_string(),
            delta: delta.to_string(),
        });
    }

    /// Emit a tool executing update.
    pub fn emit_tool_executing(&self, tool_call_id: &str, input: serde_json::Value) {
        self.emit(ChatUpdate::ToolExecuting {
            session_id: self.session_id,
            tool_call_id: tool_call_id.to_string(),
            input,
        });
    }

    /// Emit a reasoning delta.
    pub fn emit_reasoning_delta(&self, message_id: MessageId, delta: &str) {
        self.emit(ChatUpdate::ReasoningDelta {
            session_id: self.session_id,
            message_id,
            delta: delta.to_string(),
        });
    }

    /// Emit a ChatUpdate.
    fn emit(&self, update: ChatUpdate) {
        debug!(?update, "Bridge emitting update");
        if self.update_tx.send(update).is_err() {
            debug!("Update receiver dropped");
        }
    }
}

/// Convert SessionStatus from session crate to service crate.
impl From<coder_session::SessionStatus> for SessionStatus {
    fn from(status: coder_session::SessionStatus) -> Self {
        match status {
            coder_session::SessionStatus::Idle => SessionStatus::Idle,
            coder_session::SessionStatus::Busy => SessionStatus::Processing,
            coder_session::SessionStatus::WaitingForPermission => {
                SessionStatus::WaitingForPermission
            }
            coder_session::SessionStatus::Retrying { .. } => SessionStatus::Processing,
            coder_session::SessionStatus::Error => SessionStatus::Error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_session_event_translation() {
        let session_id = SessionId::new();
        let thread_id = ThreadId::new();
        let (tx, mut rx) = mpsc::unbounded_channel();

        let mut bridge = Bridge::new(session_id, thread_id, tx);

        // Send a session event
        let message_id = MessageId::new();
        bridge.handle_session_event(SessionEvent::MessageStarted {
            session_id,
            message_id,
        });

        // Check the translated update
        let update = rx.try_recv().unwrap();
        match update {
            ChatUpdate::MessageStarted {
                session_id: sid,
                message_id: mid,
                role,
            } => {
                assert_eq!(sid, session_id);
                assert_eq!(mid, message_id);
                assert_eq!(role, MessageRole::Assistant);
            }
            _ => panic!("Expected MessageStarted"),
        }
    }

    #[test]
    fn test_bridge_tool_duration_tracking() {
        let session_id = SessionId::new();
        let thread_id = ThreadId::new();
        let message_id = MessageId::new();
        let (tx, mut rx) = mpsc::unbounded_channel();

        let mut bridge = Bridge::new(session_id, thread_id, tx);

        // Start tool
        bridge.handle_session_event(SessionEvent::ToolStarted {
            session_id,
            message_id,
            tool_name: "test".into(),
            tool_call_id: "call_1".into(),
        });

        // Small delay
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Complete tool
        bridge.handle_session_event(SessionEvent::ToolCompleted {
            session_id,
            message_id,
            tool_call_id: "call_1".into(),
            success: true,
        });

        // Skip ToolStarted update
        let _ = rx.try_recv();

        // Check ToolCompleted has duration
        let update = rx.try_recv().unwrap();
        match update {
            ChatUpdate::ToolCompleted { duration_ms, .. } => {
                assert!(duration_ms >= 10, "Duration should be at least 10ms");
            }
            _ => panic!("Expected ToolCompleted"),
        }
    }
}
