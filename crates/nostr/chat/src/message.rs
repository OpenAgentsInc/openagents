//! Chat message types.
//!
//! This module defines the unified message type for display in the chat UI,
//! combining NIP-28 channel messages with NIP-90 DVM events.

use nostr::Event;

/// A chat message for display.
#[derive(Debug, Clone)]
pub enum ChatMessage {
    /// A channel message (NIP-28 kind 42)
    Channel(ChannelMessage),
    /// A DVM job request (NIP-90 kinds 5000-5999)
    JobRequest(JobRequestMessage),
    /// A DVM job result (NIP-90 kinds 6000-6999)
    JobResult(JobResultMessage),
    /// A system notice
    System(SystemMessage),
}

impl ChatMessage {
    /// Create a channel message from a Nostr event.
    pub fn from_event(event: Event, is_own: bool) -> Self {
        let is_reply = Self::is_reply(&event);
        let reply_to = Self::extract_reply_to(&event);

        ChatMessage::Channel(ChannelMessage {
            id: event.id,
            author_pubkey: event.pubkey,
            content: event.content,
            timestamp: event.created_at,
            is_reply,
            reply_to,
            is_own,
        })
    }

    /// Check if this is the user's own message.
    pub fn is_own(&self) -> bool {
        match self {
            ChatMessage::Channel(m) => m.is_own,
            ChatMessage::JobRequest(m) => m.is_own,
            ChatMessage::JobResult(_) => false,
            ChatMessage::System(_) => false,
        }
    }

    /// Get the timestamp.
    pub fn timestamp(&self) -> u64 {
        match self {
            ChatMessage::Channel(m) => m.timestamp,
            ChatMessage::JobRequest(m) => m.timestamp,
            ChatMessage::JobResult(m) => m.timestamp,
            ChatMessage::System(m) => m.timestamp,
        }
    }

    /// Get the message content for display.
    pub fn content(&self) -> &str {
        match self {
            ChatMessage::Channel(m) => &m.content,
            ChatMessage::JobRequest(m) => &m.input,
            ChatMessage::JobResult(m) => &m.content,
            ChatMessage::System(m) => &m.message,
        }
    }

    /// Check if event is a reply (has reply marker in e tag).
    fn is_reply(event: &Event) -> bool {
        event
            .tags
            .iter()
            .any(|tag| tag.len() >= 4 && tag[0] == "e" && tag[3] == "reply")
    }

    /// Extract the reply-to event ID if this is a reply.
    fn extract_reply_to(event: &Event) -> Option<String> {
        for tag in &event.tags {
            if tag.len() >= 4 && tag[0] == "e" && tag[3] == "reply" {
                return Some(tag[1].clone());
            }
        }
        None
    }
}

/// A channel message (NIP-28 kind 42).
#[derive(Debug, Clone)]
pub struct ChannelMessage {
    /// Event ID
    pub id: String,
    /// Author's public key
    pub author_pubkey: String,
    /// Message content
    pub content: String,
    /// Unix timestamp
    pub timestamp: u64,
    /// Whether this is a reply to another message
    pub is_reply: bool,
    /// Event ID being replied to
    pub reply_to: Option<String>,
    /// Whether this is the user's own message
    pub is_own: bool,
}

impl ChannelMessage {
    /// Get a short author identifier (first 8 chars of pubkey).
    pub fn author_short(&self) -> &str {
        if self.author_pubkey.len() >= 8 {
            &self.author_pubkey[..8]
        } else {
            &self.author_pubkey
        }
    }
}

/// A DVM job request message (NIP-90 kinds 5000-5999).
#[derive(Debug, Clone)]
pub struct JobRequestMessage {
    /// Job event ID
    pub id: String,
    /// Job kind
    pub kind: u16,
    /// Input text/description
    pub input: String,
    /// Unix timestamp
    pub timestamp: u64,
    /// Whether this is the user's own job
    pub is_own: bool,
}

impl JobRequestMessage {
    /// Get a display label for the job kind.
    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            5000 => "text-extract",
            5001 => "summarize",
            5002 => "translate",
            5050 => "text-gen",
            5100 => "image-gen",
            5250 => "speech-to-text",
            _ => "job",
        }
    }
}

/// A DVM job result message (NIP-90 kinds 6000-6999).
#[derive(Debug, Clone)]
pub struct JobResultMessage {
    /// Result event ID
    pub id: String,
    /// Original job ID
    pub job_id: String,
    /// Result kind (= request kind + 1000)
    pub kind: u16,
    /// Result content
    pub content: String,
    /// Service provider pubkey
    pub provider_pubkey: String,
    /// Unix timestamp
    pub timestamp: u64,
}

/// A system message (notices, errors, etc.).
#[derive(Debug, Clone)]
pub struct SystemMessage {
    /// Message text
    pub message: String,
    /// Message type
    pub message_type: SystemMessageType,
    /// Unix timestamp
    pub timestamp: u64,
}

/// Type of system message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemMessageType {
    /// Informational notice
    Info,
    /// Warning
    Warning,
    /// Error
    Error,
    /// Success
    Success,
}

impl SystemMessage {
    /// Create an info message.
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            message_type: SystemMessageType::Info,
            timestamp: chrono::Utc::now().timestamp() as u64,
        }
    }

    /// Create an error message.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            message_type: SystemMessageType::Error,
            timestamp: chrono::Utc::now().timestamp() as u64,
        }
    }

    /// Create a success message.
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            message_type: SystemMessageType::Success,
            timestamp: chrono::Utc::now().timestamp() as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_message() {
        let msg = ChannelMessage {
            id: "abc123".to_string(),
            author_pubkey: "0123456789abcdef".to_string(),
            content: "Hello, world!".to_string(),
            timestamp: 1234567890,
            is_reply: false,
            reply_to: None,
            is_own: false,
        };

        assert_eq!(msg.author_short(), "01234567");
        assert!(!msg.is_reply);
    }

    #[test]
    fn test_job_request_kind_label() {
        let job = JobRequestMessage {
            id: "job123".to_string(),
            kind: 5050,
            input: "Generate a poem".to_string(),
            timestamp: 1234567890,
            is_own: true,
        };

        assert_eq!(job.kind_label(), "text-gen");
    }

    #[test]
    fn test_system_message_info() {
        let msg = SystemMessage::info("Connected to relay");
        assert_eq!(msg.message_type, SystemMessageType::Info);
        assert_eq!(msg.message, "Connected to relay");
    }

    #[test]
    fn test_system_message_error() {
        let msg = SystemMessage::error("Connection failed");
        assert_eq!(msg.message_type, SystemMessageType::Error);
    }

    #[test]
    fn test_chat_message_timestamp() {
        let msg = ChatMessage::Channel(ChannelMessage {
            id: "abc".to_string(),
            author_pubkey: "pubkey".to_string(),
            content: "test".to_string(),
            timestamp: 1234567890,
            is_reply: false,
            reply_to: None,
            is_own: false,
        });

        assert_eq!(msg.timestamp(), 1234567890);
    }

    #[test]
    fn test_chat_message_content() {
        let msg = ChatMessage::System(SystemMessage::info("test message"));
        assert_eq!(msg.content(), "test message");
    }
}
