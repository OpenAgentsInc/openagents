//! Server-to-client messages.

use coder_domain::{event::DomainEvent, projections::chat_view::ChatView, ThreadId};
use serde::{Deserialize, Serialize};

/// Messages sent from server to client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Initial snapshot of a thread's state.
    Snapshot {
        /// The thread ID.
        thread_id: ThreadId,
        /// Current chat view.
        chat_view: ChatView,
    },

    /// Domain events to apply.
    Events {
        /// Events in order.
        events: Vec<DomainEvent>,
    },

    /// Subscription confirmed.
    Subscribed {
        /// Thread that was subscribed to.
        thread_id: ThreadId,
    },

    /// Unsubscription confirmed.
    Unsubscribed {
        /// Thread that was unsubscribed from.
        thread_id: ThreadId,
    },

    /// Error response.
    Error {
        /// Error code.
        code: ErrorCode,
        /// Human-readable message.
        message: String,
    },

    /// Pong response to ping.
    Pong {
        /// Client timestamp from ping.
        client_timestamp: i64,
        /// Server timestamp.
        server_timestamp: i64,
    },
}

/// Error codes for protocol errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Thread not found.
    ThreadNotFound,
    /// Run not found.
    RunNotFound,
    /// Not authorized.
    Unauthorized,
    /// Invalid request.
    InvalidRequest,
    /// Rate limited.
    RateLimited,
    /// Internal server error.
    Internal,
}

impl ServerMessage {
    /// Create a snapshot message.
    pub fn snapshot(thread_id: ThreadId, chat_view: ChatView) -> Self {
        Self::Snapshot {
            thread_id,
            chat_view,
        }
    }

    /// Create an events message.
    pub fn events(events: Vec<DomainEvent>) -> Self {
        Self::Events { events }
    }

    /// Create a subscribed message.
    pub fn subscribed(thread_id: ThreadId) -> Self {
        Self::Subscribed { thread_id }
    }

    /// Create an error message.
    pub fn error(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::Error {
            code,
            message: message.into(),
        }
    }

    /// Create a pong message.
    pub fn pong(client_timestamp: i64) -> Self {
        Self::Pong {
            client_timestamp,
            server_timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCode::ThreadNotFound => write!(f, "thread_not_found"),
            ErrorCode::RunNotFound => write!(f, "run_not_found"),
            ErrorCode::Unauthorized => write!(f, "unauthorized"),
            ErrorCode::InvalidRequest => write!(f, "invalid_request"),
            ErrorCode::RateLimited => write!(f, "rate_limited"),
            ErrorCode::Internal => write!(f, "internal"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_message_serialization() {
        let thread_id = ThreadId::new();
        let chat_view = ChatView::new(thread_id);
        let msg = ServerMessage::snapshot(thread_id, chat_view);

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("snapshot"));

        let deserialized: ServerMessage = serde_json::from_str(&json).unwrap();
        if let ServerMessage::Snapshot { thread_id: tid, .. } = deserialized {
            assert_eq!(tid, thread_id);
        } else {
            panic!("Wrong message type");
        }
    }

    #[test]
    fn test_error_message() {
        let msg = ServerMessage::error(ErrorCode::ThreadNotFound, "Thread does not exist");

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("error"));
        assert!(json.contains("thread_not_found"));
    }
}
