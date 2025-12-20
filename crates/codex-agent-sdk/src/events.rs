//! Event types for the Codex Agent SDK.
//!
//! These types represent the JSONL events emitted by `codex exec --experimental-json`.

use serde::{Deserialize, Serialize};

use crate::items::ThreadItem;

/// Top-level JSONL events emitted by codex exec.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ThreadEvent {
    /// Emitted when a new thread is started as the first event.
    #[serde(rename = "thread.started")]
    ThreadStarted(ThreadStartedEvent),

    /// Emitted when a turn is started by sending a new prompt to the model.
    #[serde(rename = "turn.started")]
    TurnStarted(TurnStartedEvent),

    /// Emitted when a turn is completed.
    #[serde(rename = "turn.completed")]
    TurnCompleted(TurnCompletedEvent),

    /// Indicates that a turn failed with an error.
    #[serde(rename = "turn.failed")]
    TurnFailed(TurnFailedEvent),

    /// Emitted when a new item is added to the thread.
    #[serde(rename = "item.started")]
    ItemStarted(ItemStartedEvent),

    /// Emitted when an item is updated.
    #[serde(rename = "item.updated")]
    ItemUpdated(ItemUpdatedEvent),

    /// Signals that an item has reached a terminal state.
    #[serde(rename = "item.completed")]
    ItemCompleted(ItemCompletedEvent),

    /// Represents an unrecoverable error emitted directly by the event stream.
    #[serde(rename = "error")]
    Error(ThreadErrorEvent),
}

/// Event emitted when a new thread is started.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThreadStartedEvent {
    /// The identifier of the new thread. Can be used to resume the thread later.
    pub thread_id: String,
}

/// Event emitted when a turn is started.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TurnStartedEvent {}

/// Event emitted when a turn is completed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TurnCompletedEvent {
    /// Token usage for this turn.
    pub usage: Usage,
}

/// Event emitted when a turn fails.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TurnFailedEvent {
    /// The error that caused the turn to fail.
    pub error: ThreadErrorEvent,
}

/// Token usage information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Usage {
    /// The number of input tokens used during the turn.
    pub input_tokens: i64,

    /// The number of cached input tokens used during the turn.
    #[serde(default)]
    pub cached_input_tokens: i64,

    /// The number of output tokens used during the turn.
    pub output_tokens: i64,
}

/// Event emitted when a new item is started.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemStartedEvent {
    /// The item that was started.
    pub item: ThreadItem,
}

/// Event emitted when an item is updated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemUpdatedEvent {
    /// The updated item.
    pub item: ThreadItem,
}

/// Event emitted when an item is completed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemCompletedEvent {
    /// The completed item.
    pub item: ThreadItem,
}

/// Fatal error emitted by the stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThreadErrorEvent {
    /// Error message.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_thread_started() {
        let json = r#"{"type":"thread.started","thread_id":"abc123"}"#;
        let event: ThreadEvent = serde_json::from_str(json).unwrap();

        match event {
            ThreadEvent::ThreadStarted(e) => {
                assert_eq!(e.thread_id, "abc123");
            }
            _ => panic!("Expected ThreadStarted"),
        }
    }

    #[test]
    fn test_deserialize_turn_completed() {
        let json = r#"{"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":50,"output_tokens":25}}"#;
        let event: ThreadEvent = serde_json::from_str(json).unwrap();

        match event {
            ThreadEvent::TurnCompleted(e) => {
                assert_eq!(e.usage.input_tokens, 100);
                assert_eq!(e.usage.cached_input_tokens, 50);
                assert_eq!(e.usage.output_tokens, 25);
            }
            _ => panic!("Expected TurnCompleted"),
        }
    }

    #[test]
    fn test_deserialize_item_completed() {
        let json = r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"Hello"}}"#;
        let event: ThreadEvent = serde_json::from_str(json).unwrap();

        match event {
            ThreadEvent::ItemCompleted(e) => {
                assert_eq!(e.item.id, "item_0");
            }
            _ => panic!("Expected ItemCompleted"),
        }
    }

    #[test]
    fn test_deserialize_error() {
        let json = r#"{"type":"error","message":"Something went wrong"}"#;
        let event: ThreadEvent = serde_json::from_str(json).unwrap();

        match event {
            ThreadEvent::Error(e) => {
                assert_eq!(e.message, "Something went wrong");
            }
            _ => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_deserialize_turn_failed() {
        let json = r#"{"type":"turn.failed","error":{"message":"API error"}}"#;
        let event: ThreadEvent = serde_json::from_str(json).unwrap();

        match event {
            ThreadEvent::TurnFailed(e) => {
                assert_eq!(e.error.message, "API error");
            }
            _ => panic!("Expected TurnFailed"),
        }
    }
}
