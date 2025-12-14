//! Client-to-server messages.

use coder_domain::{RunId, ThreadId};
use serde::{Deserialize, Serialize};

/// Messages sent from client to server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Subscribe to a thread's event stream.
    Subscribe {
        /// Thread to subscribe to.
        thread_id: ThreadId,
        /// Whether to include history.
        include_history: bool,
    },

    /// Unsubscribe from a thread's event stream.
    Unsubscribe {
        /// Thread to unsubscribe from.
        thread_id: ThreadId,
    },

    /// Send a user message.
    SendMessage {
        /// Target thread.
        thread_id: ThreadId,
        /// Message content (markdown).
        content: String,
    },

    /// Cancel a running workflow.
    CancelRun {
        /// Run to cancel.
        run_id: RunId,
    },

    /// Approve a step that's waiting for approval.
    ApproveStep {
        /// Run containing the step.
        run_id: RunId,
        /// Step to approve.
        step_id: coder_domain::StepId,
    },

    /// Reject a step that's waiting for approval.
    RejectStep {
        /// Run containing the step.
        run_id: RunId,
        /// Step to reject.
        step_id: coder_domain::StepId,
        /// Reason for rejection.
        reason: Option<String>,
    },

    /// Request a ping (keepalive).
    Ping {
        /// Client timestamp for latency measurement.
        timestamp: i64,
    },
}

impl ClientMessage {
    /// Create a subscribe message.
    pub fn subscribe(thread_id: ThreadId) -> Self {
        Self::Subscribe {
            thread_id,
            include_history: true,
        }
    }

    /// Create an unsubscribe message.
    pub fn unsubscribe(thread_id: ThreadId) -> Self {
        Self::Unsubscribe { thread_id }
    }

    /// Create a send message.
    pub fn send_message(thread_id: ThreadId, content: impl Into<String>) -> Self {
        Self::SendMessage {
            thread_id,
            content: content.into(),
        }
    }

    /// Create a cancel run message.
    pub fn cancel_run(run_id: RunId) -> Self {
        Self::CancelRun { run_id }
    }

    /// Create a ping message.
    pub fn ping() -> Self {
        Self::Ping {
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_message_serialization() {
        let thread_id = ThreadId::new();
        let msg = ClientMessage::subscribe(thread_id);

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("subscribe"));
        assert!(json.contains("thread_id"));

        let deserialized: ClientMessage = serde_json::from_str(&json).unwrap();
        if let ClientMessage::Subscribe { include_history, .. } = deserialized {
            assert!(include_history);
        } else {
            panic!("Wrong message type");
        }
    }
}
