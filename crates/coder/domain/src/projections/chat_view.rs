//! Chat view projection for rendering conversation threads.
//!
//! This projection is optimized for the chat UI, providing a linear
//! view of messages with embedded tool uses and streaming support.

use crate::event::DomainEvent;
use crate::ids::{MessageId, ThreadId, ToolUseId};
use crate::message::Role;
use crate::tool::{ToolResult, ToolUseStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Maximum characters to include in thread previews.
const THREAD_PREVIEW_MAX_LEN: usize = 120;

/// A projected view of a chat thread optimized for UI rendering.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChatView {
    /// The thread ID.
    pub thread_id: ThreadId,

    /// Entries in the chat (messages and tool uses).
    pub entries: Vec<ChatEntry>,

    /// Currently streaming message, if any.
    pub streaming_message: Option<StreamingMessage>,

    /// Total message count.
    pub message_count: usize,

    /// When the thread was last updated.
    pub last_updated: Option<DateTime<Utc>>,
}

/// Lightweight summary metadata for thread lists.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThreadSummary {
    /// The thread ID.
    pub thread_id: ThreadId,

    /// Preview of the last message content (truncated).
    pub last_message_preview: Option<String>,

    /// Role of the last message author.
    pub last_message_role: Option<Role>,

    /// Total number of messages in the thread.
    pub message_count: usize,

    /// Count of messages not yet marked as read.
    pub unread_count: usize,

    /// When the last message was added/updated.
    pub last_updated: Option<DateTime<Utc>>,
}

impl ThreadSummary {
    /// Create an empty summary for a thread.
    pub fn new(thread_id: ThreadId) -> Self {
        Self {
            thread_id,
            last_message_preview: None,
            last_message_role: None,
            message_count: 0,
            unread_count: 0,
            last_updated: None,
        }
    }
}

/// An entry in the chat view.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatEntry {
    /// A complete message.
    Message(MessageView),

    /// A tool use (shown inline with its result).
    ToolUse(ToolUseView),
}

/// A message as viewed in the chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageView {
    /// Message ID.
    pub id: MessageId,

    /// Role of the sender.
    pub role: Role,

    /// Message content (markdown).
    pub content: String,

    /// When the message was sent.
    pub timestamp: DateTime<Utc>,

    /// Whether this message has tool uses.
    pub has_tool_uses: bool,
}

/// A tool use as viewed in the chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseView {
    /// Tool use ID.
    pub id: ToolUseId,

    /// Parent message ID.
    pub message_id: MessageId,

    /// Tool name.
    pub tool_name: String,

    /// Tool input (simplified for display).
    pub input_summary: String,

    /// Current status.
    pub status: ToolUseStatus,

    /// Result summary (if complete).
    pub result_summary: Option<String>,

    /// Duration in milliseconds (if complete).
    pub duration_ms: Option<u64>,
}

/// A message that is currently being streamed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingMessage {
    /// Message ID.
    pub id: MessageId,

    /// Content received so far.
    pub content_so_far: String,

    /// Whether streaming is complete.
    pub is_complete: bool,

    /// When streaming started.
    pub started_at: DateTime<Utc>,
}

impl ChatView {
    /// Create a new empty chat view.
    pub fn new(thread_id: ThreadId) -> Self {
        Self {
            thread_id,
            entries: Vec::new(),
            streaming_message: None,
            message_count: 0,
            last_updated: None,
        }
    }

    /// Apply a domain event to update this view.
    pub fn apply(&mut self, event: &DomainEvent) {
        match event {
            DomainEvent::MessageAdded { thread_id, message } => {
                if *thread_id != self.thread_id {
                    return;
                }

                // If there's a streaming message that matches, finalize it
                if self
                    .streaming_message
                    .as_ref()
                    .is_some_and(|streaming| streaming.id == message.id)
                {
                    self.streaming_message = None;
                }

                let view = MessageView {
                    id: message.id,
                    role: message.role,
                    content: message.content.clone(),
                    timestamp: message.created_at,
                    has_tool_uses: message.has_tool_uses(),
                };
                self.entries.push(ChatEntry::Message(view));
                self.message_count += 1;
                self.last_updated = Some(message.created_at);
            }

            DomainEvent::MessageStreaming {
                thread_id,
                message_id,
                delta,
                timestamp,
            } => {
                if *thread_id != self.thread_id {
                    return;
                }

                match &mut self.streaming_message {
                    Some(streaming) if streaming.id == *message_id => {
                        streaming.content_so_far.push_str(delta);
                    }
                    _ => {
                        // Start a new streaming message
                        self.streaming_message = Some(StreamingMessage {
                            id: *message_id,
                            content_so_far: delta.clone(),
                            is_complete: false,
                            started_at: *timestamp,
                        });
                    }
                }
                self.last_updated = Some(*timestamp);
            }

            DomainEvent::MessageComplete {
                thread_id,
                message_id,
                timestamp,
            } => {
                if *thread_id != self.thread_id {
                    return;
                }

                if let Some(streaming) = self
                    .streaming_message
                    .as_mut()
                    .filter(|streaming| streaming.id == *message_id)
                {
                    streaming.is_complete = true;
                }
                self.last_updated = Some(*timestamp);
            }

            DomainEvent::ToolUseStarted {
                thread_id,
                message_id,
                tool_use,
            } => {
                if *thread_id != self.thread_id {
                    return;
                }

                let view = ToolUseView {
                    id: tool_use.id,
                    message_id: *message_id,
                    tool_name: tool_use.tool_name.clone(),
                    input_summary: summarize_input(&tool_use.input),
                    status: tool_use.status,
                    result_summary: None,
                    duration_ms: None,
                };
                self.entries.push(ChatEntry::ToolUse(view));
                self.last_updated = Some(tool_use.started_at);
            }

            DomainEvent::ToolUseComplete {
                thread_id,
                tool_use_id,
                result,
                ..
            } => {
                if *thread_id != self.thread_id {
                    return;
                }

                // Find and update the tool use entry
                for entry in &mut self.entries {
                    if let ChatEntry::ToolUse(view) = entry
                        && view.id == *tool_use_id
                    {
                        view.status = if result.success {
                            ToolUseStatus::Success
                        } else {
                            ToolUseStatus::Failed
                        };
                        view.result_summary = Some(summarize_result(result));
                        view.duration_ms = Some(result.duration_ms);
                        break;
                    }
                }
                self.last_updated = Some(Utc::now());
            }

            _ => {}
        }
    }

    /// Get visible entries (for virtual scrolling).
    pub fn visible_entries(&self, start: usize, count: usize) -> &[ChatEntry] {
        let end = (start + count).min(self.entries.len());
        &self.entries[start..end]
    }

    /// Get the current streaming content for display.
    pub fn streaming_content(&self) -> Option<&str> {
        self.streaming_message
            .as_ref()
            .map(|s| s.content_so_far.as_str())
    }

    /// Check if there is active streaming.
    pub fn is_streaming(&self) -> bool {
        self.streaming_message
            .as_ref()
            .is_some_and(|s| !s.is_complete)
    }

    /// Get entry count.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get a lightweight summary suitable for thread lists.
    pub fn summary(&self) -> ThreadSummary {
        let last_message = self.entries.iter().rev().find_map(|entry| match entry {
            ChatEntry::Message(message) => Some(message),
            _ => None,
        });

        let (last_message_preview, last_message_role, last_updated) =
            if let Some(message) = last_message {
                (
                    Some(truncate(&message.content, THREAD_PREVIEW_MAX_LEN)),
                    Some(message.role),
                    Some(message.timestamp),
                )
            } else if let Some(streaming) = &self.streaming_message {
                if streaming.content_so_far.is_empty() {
                    (None, None, self.last_updated)
                } else {
                    (
                        Some(truncate(
                            streaming.content_so_far.as_str(),
                            THREAD_PREVIEW_MAX_LEN,
                        )),
                        None,
                        Some(streaming.started_at),
                    )
                }
            } else {
                (None, None, self.last_updated)
            };

        ThreadSummary {
            thread_id: self.thread_id,
            last_message_preview,
            last_message_role,
            message_count: self.message_count,
            unread_count: self.message_count,
            last_updated,
        }
    }
}

/// Summarize tool input for display.
fn summarize_input(input: &serde_json::Value) -> String {
    match input {
        serde_json::Value::Object(obj) => {
            // Show first few keys
            let keys: Vec<&str> = obj.keys().take(3).map(|s| s.as_str()).collect();
            if keys.is_empty() {
                "{}".to_string()
            } else {
                format!("{{{}}}", keys.join(", "))
            }
        }
        serde_json::Value::String(s) => {
            if s.len() > 50 {
                format!("{}...", &s[..50])
            } else {
                s.clone()
            }
        }
        other => other.to_string(),
    }
}

/// Summarize tool result for display.
fn summarize_result(result: &ToolResult) -> String {
    if let Some(error) = &result.error {
        format!("Error: {}", truncate(error, 100))
    } else {
        match &result.output {
            crate::tool::ToolOutput::Text(s) => truncate(s, 100),
            crate::tool::ToolOutput::Json(v) => truncate(&v.to_string(), 100),
            crate::tool::ToolOutput::File { path, .. } => format!("File: {}", path),
            crate::tool::ToolOutput::Binary { mime_type, .. } => format!("Binary: {}", mime_type),
            crate::tool::ToolOutput::Empty => "Done".to_string(),
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Message;

    #[test]
    fn test_chat_view_apply_message() {
        let thread_id = ThreadId::new();
        let mut view = ChatView::new(thread_id);

        let message = Message::new(Role::User, "Hello!");
        let event = DomainEvent::MessageAdded {
            thread_id,
            message: message.clone(),
        };

        view.apply(&event);

        assert_eq!(view.entries.len(), 1);
        assert_eq!(view.message_count, 1);

        if let ChatEntry::Message(m) = &view.entries[0] {
            assert_eq!(m.content, "Hello!");
            assert_eq!(m.role, Role::User);
        } else {
            panic!("Expected message entry");
        }
    }

    #[test]
    fn test_chat_view_summary_tracks_latest_message() {
        let thread_id = ThreadId::new();
        let mut view = ChatView::new(thread_id);

        // Initial summary is empty.
        let initial = view.summary();
        assert_eq!(initial.thread_id, thread_id);
        assert!(initial.last_message_preview.is_none());
        assert!(initial.last_message_role.is_none());
        assert_eq!(initial.message_count, 0);
        assert_eq!(initial.unread_count, 0);
        assert!(initial.last_updated.is_none());

        // Add a user message and verify summary updates.
        let first_message = Message::new(Role::User, "First message");
        let first_timestamp = first_message.created_at;
        view.apply(&DomainEvent::MessageAdded {
            thread_id,
            message: first_message.clone(),
        });

        let after_first = view.summary();
        assert_eq!(after_first.message_count, 1);
        assert_eq!(after_first.unread_count, 1);
        assert_eq!(after_first.last_message_role, Some(Role::User));
        assert_eq!(
            after_first.last_message_preview.as_deref(),
            Some("First message")
        );
        assert_eq!(after_first.last_updated, Some(first_timestamp));

        // Add a long assistant message and ensure preview is truncated and role updates.
        let long_content = "x".repeat(THREAD_PREVIEW_MAX_LEN + 10);
        let assistant_message = Message::new(Role::Assistant, long_content.clone());
        view.apply(&DomainEvent::MessageAdded {
            thread_id,
            message: assistant_message.clone(),
        });

        let expected_preview = format!("{}...", &long_content[..THREAD_PREVIEW_MAX_LEN]);
        let after_second = view.summary();
        assert_eq!(after_second.message_count, 2);
        assert_eq!(after_second.unread_count, 2);
        assert_eq!(after_second.last_message_role, Some(Role::Assistant));
        assert_eq!(
            after_second.last_message_preview.as_deref(),
            Some(expected_preview.as_str())
        );
        assert_eq!(
            after_second.last_updated,
            Some(assistant_message.created_at)
        );
    }

    #[test]
    fn test_chat_view_streaming() {
        let thread_id = ThreadId::new();
        let message_id = MessageId::new();
        let mut view = ChatView::new(thread_id);

        // Start streaming
        view.apply(&DomainEvent::MessageStreaming {
            thread_id,
            message_id,
            delta: "Hello".to_string(),
            timestamp: Utc::now(),
        });

        assert!(view.is_streaming());
        assert_eq!(view.streaming_content(), Some("Hello"));

        // Continue streaming
        view.apply(&DomainEvent::MessageStreaming {
            thread_id,
            message_id,
            delta: ", world!".to_string(),
            timestamp: Utc::now(),
        });

        assert_eq!(view.streaming_content(), Some("Hello, world!"));

        // Complete streaming
        view.apply(&DomainEvent::MessageComplete {
            thread_id,
            message_id,
            timestamp: Utc::now(),
        });

        // Should still have streaming message but marked complete
        assert!(!view.is_streaming());
    }

    #[test]
    fn test_chat_view_ignores_other_threads() {
        let thread_id = ThreadId::new();
        let other_thread_id = ThreadId::new();
        let mut view = ChatView::new(thread_id);

        let message = Message::new(Role::User, "Hello!");
        let event = DomainEvent::MessageAdded {
            thread_id: other_thread_id,
            message,
        };

        view.apply(&event);

        assert!(view.entries.is_empty());
    }
}
