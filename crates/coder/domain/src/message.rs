//! Message entity for chat threads.
//!
//! Messages represent individual entries in a conversation thread,
//! including user messages, assistant responses, and tool interactions.

use crate::ids::{MessageId, ToolUseId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// The role of a message sender.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Message from the user.
    User,
    /// Message from the assistant/agent.
    Assistant,
    /// System message (instructions, context).
    System,
}

impl Role {
    /// Returns true if this is a user message.
    pub fn is_user(&self) -> bool {
        matches!(self, Role::User)
    }

    /// Returns true if this is an assistant message.
    pub fn is_assistant(&self) -> bool {
        matches!(self, Role::Assistant)
    }

    /// Returns true if this is a system message.
    pub fn is_system(&self) -> bool {
        matches!(self, Role::System)
    }
}

/// A message in a conversation thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier for this message.
    pub id: MessageId,

    /// Role of the message sender.
    pub role: Role,

    /// Message content (markdown).
    pub content: String,

    /// Tool uses initiated by this message.
    /// Using SmallVec since most messages have 0-2 tool uses.
    pub tool_uses: SmallVec<[ToolUseId; 4]>,

    /// When the message was created.
    pub created_at: DateTime<Utc>,

    /// When the message was last updated (for streaming).
    pub updated_at: DateTime<Utc>,

    /// Whether the message content is complete (false during streaming).
    pub is_complete: bool,
}

impl Message {
    /// Create a new message.
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: MessageId::new(),
            role,
            content: content.into(),
            tool_uses: SmallVec::new(),
            created_at: now,
            updated_at: now,
            is_complete: true,
        }
    }

    /// Create a new message with a specific ID.
    pub fn with_id(id: MessageId, role: Role, content: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id,
            role,
            content: content.into(),
            tool_uses: SmallVec::new(),
            created_at: now,
            updated_at: now,
            is_complete: true,
        }
    }

    /// Create a new streaming message (incomplete).
    pub fn streaming(role: Role) -> Self {
        let now = Utc::now();
        Self {
            id: MessageId::new(),
            role,
            content: String::new(),
            tool_uses: SmallVec::new(),
            created_at: now,
            updated_at: now,
            is_complete: false,
        }
    }

    /// Append content to a streaming message.
    pub fn append_content(&mut self, delta: &str) {
        self.content.push_str(delta);
        self.updated_at = Utc::now();
    }

    /// Mark the message as complete.
    pub fn complete(&mut self) {
        self.is_complete = true;
        self.updated_at = Utc::now();
    }

    /// Add a tool use reference to this message.
    pub fn add_tool_use(&mut self, tool_use_id: ToolUseId) {
        self.tool_uses.push(tool_use_id);
        self.updated_at = Utc::now();
    }

    /// Check if this message has any tool uses.
    pub fn has_tool_uses(&self) -> bool {
        !self.tool_uses.is_empty()
    }
}

/// Builder for creating messages with more options.
pub struct MessageBuilder {
    message: Message,
}

impl MessageBuilder {
    /// Create a new builder.
    pub fn new(role: Role) -> Self {
        Self {
            message: Message::new(role, ""),
        }
    }

    /// Set the message ID.
    pub fn id(mut self, id: MessageId) -> Self {
        self.message.id = id;
        self
    }

    /// Set the message content.
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.message.content = content.into();
        self
    }

    /// Add tool uses.
    pub fn tool_uses(mut self, tool_uses: impl IntoIterator<Item = ToolUseId>) -> Self {
        self.message.tool_uses.extend(tool_uses);
        self
    }

    /// Set the creation timestamp.
    pub fn created_at(mut self, timestamp: DateTime<Utc>) -> Self {
        self.message.created_at = timestamp;
        self.message.updated_at = timestamp;
        self
    }

    /// Set whether the message is complete.
    pub fn is_complete(mut self, complete: bool) -> Self {
        self.message.is_complete = complete;
        self
    }

    /// Build the message.
    pub fn build(self) -> Message {
        self.message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = Message::new(Role::User, "Hello, world!");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello, world!");
        assert!(msg.is_complete);
        assert!(msg.tool_uses.is_empty());
    }

    #[test]
    fn test_streaming_message() {
        let mut msg = Message::streaming(Role::Assistant);
        assert!(!msg.is_complete);
        assert!(msg.content.is_empty());

        msg.append_content("Hello");
        msg.append_content(", world!");
        assert_eq!(msg.content, "Hello, world!");
        assert!(!msg.is_complete);

        msg.complete();
        assert!(msg.is_complete);
    }

    #[test]
    fn test_message_builder() {
        let id = MessageId::new();
        let msg = MessageBuilder::new(Role::Assistant)
            .id(id)
            .content("Test content")
            .is_complete(true)
            .build();

        assert_eq!(msg.id, id);
        assert_eq!(msg.content, "Test content");
        assert!(msg.is_complete);
    }
}
