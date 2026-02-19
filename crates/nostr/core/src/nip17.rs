//! NIP-17: Private Direct Messages
//!
//! This NIP defines an encrypted chat scheme which uses NIP-44 encryption
//! and NIP-59 seals and gift wraps.
//!
//! ## Key Features
//! - No metadata leak (participants, timestamps, kinds hidden)
//! - No public group identifiers
//! - No moderation or admins
//! - Fully recoverable with private key
//! - Optional forward secrecy (disappearing messages)
//! - Works with public relays
//!
//! ## Event Kinds
//! - Kind 14: Chat messages
//! - Kind 15: File messages
//! - Kind 10050: DM relay preferences
//!
//! Messages are sent as unsigned events (rumors), sealed (kind 13),
//! and gift-wrapped (kind 1059) to each recipient.

use crate::nip01::{Event, UnsignedEvent};
use crate::nip59::{Rumor, gift_wrap, unwrap_gift_wrap_full};
use thiserror::Error;

/// Kind for chat messages
pub const KIND_CHAT_MESSAGE: u16 = 14;

/// Kind for file messages
pub const KIND_FILE_MESSAGE: u16 = 15;

/// Kind for DM relay preferences
pub const KIND_DM_RELAY_LIST: u16 = 10050;

/// Errors that can occur during NIP-17 operations.
#[derive(Debug, Error)]
pub enum Nip17Error {
    #[error("invalid kind: expected {expected}, got {got}")]
    InvalidKind { expected: String, got: u16 },

    #[error("missing required tag: {0}")]
    MissingTag(String),

    #[error("invalid tag: {0}")]
    InvalidTag(String),

    #[error("NIP-59 error: {0}")]
    Nip59(#[from] crate::nip59::Nip59Error),

    #[error("NIP-01 error: {0}")]
    Nip01(#[from] crate::nip01::Nip01Error),

    #[error("serialization error: {0}")]
    Serialization(String),
}

/// A chat message (kind 14).
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Message content (plain text)
    pub content: String,
    /// Recipients' public keys
    pub recipients: Vec<String>,
    /// Optional relay URLs for recipients
    pub recipient_relays: Vec<Option<String>>,
    /// Optional parent message ID (for replies)
    pub reply_to: Option<String>,
    /// Optional conversation subject/title
    pub subject: Option<String>,
    /// Optional quoted events (for citations)
    pub quoted_events: Vec<QuotedEvent>,
}

/// A quoted event reference (q tag).
#[derive(Debug, Clone)]
pub struct QuotedEvent {
    /// Event ID or address
    pub id_or_address: String,
    /// Optional relay URL
    pub relay: Option<String>,
    /// Optional pubkey (if regular event)
    pub pubkey: Option<String>,
}

impl ChatMessage {
    /// Create a new chat message.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            recipients: Vec::new(),
            recipient_relays: Vec::new(),
            reply_to: None,
            subject: None,
            quoted_events: Vec::new(),
        }
    }

    /// Add a recipient.
    pub fn add_recipient(mut self, pubkey: impl Into<String>, relay: Option<String>) -> Self {
        self.recipients.push(pubkey.into());
        self.recipient_relays.push(relay);
        self
    }

    /// Set the message this is replying to.
    pub fn reply_to(mut self, event_id: impl Into<String>) -> Self {
        self.reply_to = Some(event_id.into());
        self
    }

    /// Set the conversation subject.
    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    /// Add a quoted event.
    pub fn quote_event(mut self, quoted: QuotedEvent) -> Self {
        self.quoted_events.push(quoted);
        self
    }

    /// Convert to unsigned event (rumor) for wrapping.
    pub fn to_unsigned_event(&self, sender_pubkey: &str, timestamp: u64) -> UnsignedEvent {
        let mut tags = Vec::new();

        // Add p tags for recipients
        for (i, recipient) in self.recipients.iter().enumerate() {
            let mut p_tag = vec!["p".to_string(), recipient.clone()];
            if let Some(Some(relay)) = self.recipient_relays.get(i) {
                p_tag.push(relay.clone());
            }
            tags.push(p_tag);
        }

        // Add e tag for reply
        if let Some(reply_id) = &self.reply_to {
            tags.push(vec!["e".to_string(), reply_id.clone()]);
        }

        // Add subject tag
        if let Some(subject) = &self.subject {
            tags.push(vec!["subject".to_string(), subject.clone()]);
        }

        // Add q tags for quoted events
        for quoted in &self.quoted_events {
            let mut q_tag = vec!["q".to_string(), quoted.id_or_address.clone()];
            if let Some(relay) = &quoted.relay {
                q_tag.push(relay.clone());
            }
            if let Some(pubkey) = &quoted.pubkey {
                q_tag.push(pubkey.clone());
            }
            tags.push(q_tag);
        }

        UnsignedEvent {
            pubkey: sender_pubkey.to_string(),
            created_at: timestamp,
            kind: KIND_CHAT_MESSAGE,
            tags,
            content: self.content.clone(),
        }
    }

    /// Parse from rumor.
    pub fn from_rumor(rumor: &Rumor) -> Result<Self, Nip17Error> {
        if rumor.kind != KIND_CHAT_MESSAGE {
            return Err(Nip17Error::InvalidKind {
                expected: "14".to_string(),
                got: rumor.kind,
            });
        }

        let mut message = ChatMessage::new(&rumor.content);

        for tag in &rumor.tags {
            if tag.is_empty() {
                continue;
            }

            match tag[0].as_str() {
                "p" if tag.len() >= 2 => {
                    let pubkey = tag[1].clone();
                    let relay = tag.get(2).cloned();
                    message.recipients.push(pubkey);
                    message.recipient_relays.push(relay);
                }
                "e" if tag.len() >= 2 => {
                    message.reply_to = Some(tag[1].clone());
                }
                "subject" if tag.len() >= 2 => {
                    message.subject = Some(tag[1].clone());
                }
                "q" if tag.len() >= 2 => {
                    let quoted = QuotedEvent {
                        id_or_address: tag[1].clone(),
                        relay: tag.get(2).cloned(),
                        pubkey: tag.get(3).cloned(),
                    };
                    message.quoted_events.push(quoted);
                }
                _ => {}
            }
        }

        Ok(message)
    }
}

/// A file message (kind 15).
#[derive(Debug, Clone)]
pub struct FileMessage {
    /// URL of the file
    pub file_url: String,
    /// MIME type of the file
    pub file_type: String,
    /// Encryption algorithm used
    pub encryption_algorithm: String,
    /// Decryption key
    pub decryption_key: String,
    /// Decryption nonce
    pub decryption_nonce: String,
    /// SHA-256 hash of encrypted file
    pub hash: String,
    /// Optional SHA-256 hash of original file
    pub original_hash: Option<String>,
    /// Recipients' public keys
    pub recipients: Vec<String>,
    /// Optional relay URLs for recipients
    pub recipient_relays: Vec<Option<String>>,
    /// Optional parent message ID (for replies)
    pub reply_to: Option<String>,
    /// Optional conversation subject/title
    pub subject: Option<String>,
    /// Optional file size in bytes
    pub size: Option<u64>,
    /// Optional dimensions (width x height)
    pub dimensions: Option<String>,
    /// Optional blurhash
    pub blurhash: Option<String>,
    /// Optional thumbnail URL
    pub thumbnail_url: Option<String>,
    /// Optional fallback URLs
    pub fallback_urls: Vec<String>,
}

impl FileMessage {
    /// Create a new file message.
    pub fn new(
        file_url: impl Into<String>,
        file_type: impl Into<String>,
        encryption_algorithm: impl Into<String>,
        decryption_key: impl Into<String>,
        decryption_nonce: impl Into<String>,
        hash: impl Into<String>,
    ) -> Self {
        Self {
            file_url: file_url.into(),
            file_type: file_type.into(),
            encryption_algorithm: encryption_algorithm.into(),
            decryption_key: decryption_key.into(),
            decryption_nonce: decryption_nonce.into(),
            hash: hash.into(),
            original_hash: None,
            recipients: Vec::new(),
            recipient_relays: Vec::new(),
            reply_to: None,
            subject: None,
            size: None,
            dimensions: None,
            blurhash: None,
            thumbnail_url: None,
            fallback_urls: Vec::new(),
        }
    }

    /// Add a recipient.
    pub fn add_recipient(mut self, pubkey: impl Into<String>, relay: Option<String>) -> Self {
        self.recipients.push(pubkey.into());
        self.recipient_relays.push(relay);
        self
    }

    /// Set the message this is replying to.
    pub fn reply_to(mut self, event_id: impl Into<String>) -> Self {
        self.reply_to = Some(event_id.into());
        self
    }

    /// Set the conversation subject.
    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    /// Set the original file hash.
    pub fn original_hash(mut self, hash: impl Into<String>) -> Self {
        self.original_hash = Some(hash.into());
        self
    }

    /// Set the file size.
    pub fn size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    /// Set the dimensions.
    pub fn dimensions(mut self, width: u32, height: u32) -> Self {
        self.dimensions = Some(format!("{}x{}", width, height));
        self
    }

    /// Set the blurhash.
    pub fn blurhash(mut self, blurhash: impl Into<String>) -> Self {
        self.blurhash = Some(blurhash.into());
        self
    }

    /// Set the thumbnail URL.
    pub fn thumbnail_url(mut self, url: impl Into<String>) -> Self {
        self.thumbnail_url = Some(url.into());
        self
    }

    /// Add a fallback URL.
    pub fn add_fallback_url(mut self, url: impl Into<String>) -> Self {
        self.fallback_urls.push(url.into());
        self
    }

    /// Convert to unsigned event (rumor) for wrapping.
    pub fn to_unsigned_event(&self, sender_pubkey: &str, timestamp: u64) -> UnsignedEvent {
        let mut tags = Vec::new();

        // Add p tags for recipients
        for (i, recipient) in self.recipients.iter().enumerate() {
            let mut p_tag = vec!["p".to_string(), recipient.clone()];
            if let Some(Some(relay)) = self.recipient_relays.get(i) {
                p_tag.push(relay.clone());
            }
            tags.push(p_tag);
        }

        // Add e tag for reply
        if let Some(reply_id) = &self.reply_to {
            let mut e_tag = vec!["e".to_string(), reply_id.clone()];
            // Add reply marker
            e_tag.push("".to_string());
            e_tag.push("reply".to_string());
            tags.push(e_tag);
        }

        // Add subject tag
        if let Some(subject) = &self.subject {
            tags.push(vec!["subject".to_string(), subject.clone()]);
        }

        // Add file metadata tags
        tags.push(vec!["file-type".to_string(), self.file_type.clone()]);
        tags.push(vec![
            "encryption-algorithm".to_string(),
            self.encryption_algorithm.clone(),
        ]);
        tags.push(vec![
            "decryption-key".to_string(),
            self.decryption_key.clone(),
        ]);
        tags.push(vec![
            "decryption-nonce".to_string(),
            self.decryption_nonce.clone(),
        ]);
        tags.push(vec!["x".to_string(), self.hash.clone()]);

        if let Some(original_hash) = &self.original_hash {
            tags.push(vec!["ox".to_string(), original_hash.clone()]);
        }

        if let Some(size) = self.size {
            tags.push(vec!["size".to_string(), size.to_string()]);
        }

        if let Some(dimensions) = &self.dimensions {
            tags.push(vec!["dim".to_string(), dimensions.clone()]);
        }

        if let Some(blurhash) = &self.blurhash {
            tags.push(vec!["blurhash".to_string(), blurhash.clone()]);
        }

        if let Some(thumbnail_url) = &self.thumbnail_url {
            tags.push(vec!["thumb".to_string(), thumbnail_url.clone()]);
        }

        for fallback_url in &self.fallback_urls {
            tags.push(vec!["fallback".to_string(), fallback_url.clone()]);
        }

        UnsignedEvent {
            pubkey: sender_pubkey.to_string(),
            created_at: timestamp,
            kind: KIND_FILE_MESSAGE,
            tags,
            content: self.file_url.clone(),
        }
    }

    /// Parse from rumor.
    pub fn from_rumor(rumor: &Rumor) -> Result<Self, Nip17Error> {
        if rumor.kind != KIND_FILE_MESSAGE {
            return Err(Nip17Error::InvalidKind {
                expected: "15".to_string(),
                got: rumor.kind,
            });
        }

        // Extract required tags
        let file_type = rumor
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "file-type")
            .ok_or_else(|| Nip17Error::MissingTag("file-type".to_string()))?[1]
            .clone();

        let encryption_algorithm = rumor
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "encryption-algorithm")
            .ok_or_else(|| Nip17Error::MissingTag("encryption-algorithm".to_string()))?[1]
            .clone();

        let decryption_key = rumor
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "decryption-key")
            .ok_or_else(|| Nip17Error::MissingTag("decryption-key".to_string()))?[1]
            .clone();

        let decryption_nonce = rumor
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "decryption-nonce")
            .ok_or_else(|| Nip17Error::MissingTag("decryption-nonce".to_string()))?[1]
            .clone();

        let hash = rumor
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "x")
            .ok_or_else(|| Nip17Error::MissingTag("x".to_string()))?[1]
            .clone();

        let mut message = FileMessage::new(
            &rumor.content,
            file_type,
            encryption_algorithm,
            decryption_key,
            decryption_nonce,
            hash,
        );

        // Extract optional tags
        for tag in &rumor.tags {
            if tag.is_empty() {
                continue;
            }

            match tag[0].as_str() {
                "p" if tag.len() >= 2 => {
                    let pubkey = tag[1].clone();
                    let relay = tag.get(2).cloned();
                    message.recipients.push(pubkey);
                    message.recipient_relays.push(relay);
                }
                "e" if tag.len() >= 2 => {
                    message.reply_to = Some(tag[1].clone());
                }
                "subject" if tag.len() >= 2 => {
                    message.subject = Some(tag[1].clone());
                }
                "ox" if tag.len() >= 2 => {
                    message.original_hash = Some(tag[1].clone());
                }
                "size" if tag.len() >= 2 => {
                    message.size = tag[1].parse().ok();
                }
                "dim" if tag.len() >= 2 => {
                    message.dimensions = Some(tag[1].clone());
                }
                "blurhash" if tag.len() >= 2 => {
                    message.blurhash = Some(tag[1].clone());
                }
                "thumb" if tag.len() >= 2 => {
                    message.thumbnail_url = Some(tag[1].clone());
                }
                "fallback" if tag.len() >= 2 => {
                    message.fallback_urls.push(tag[1].clone());
                }
                _ => {}
            }
        }

        Ok(message)
    }
}

/// DM relay list (kind 10050).
#[derive(Debug, Clone)]
pub struct DmRelayList {
    /// Relay URLs for receiving DMs
    pub relays: Vec<String>,
}

impl DmRelayList {
    /// Create a new DM relay list.
    pub fn new() -> Self {
        Self { relays: Vec::new() }
    }

    /// Add a relay.
    pub fn add_relay(mut self, relay: impl Into<String>) -> Self {
        self.relays.push(relay.into());
        self
    }

    /// Convert to event for publishing.
    pub fn to_unsigned_event(&self, pubkey: &str, timestamp: u64) -> UnsignedEvent {
        let tags = self
            .relays
            .iter()
            .map(|relay| vec!["relay".to_string(), relay.clone()])
            .collect();

        UnsignedEvent {
            pubkey: pubkey.to_string(),
            created_at: timestamp,
            kind: KIND_DM_RELAY_LIST,
            tags,
            content: String::new(),
        }
    }

    /// Parse from event.
    pub fn from_event(event: &Event) -> Result<Self, Nip17Error> {
        if event.kind != KIND_DM_RELAY_LIST {
            return Err(Nip17Error::InvalidKind {
                expected: "10050".to_string(),
                got: event.kind,
            });
        }

        let relays = event
            .tags
            .iter()
            .filter(|t| t.len() >= 2 && t[0] == "relay")
            .map(|t| t[1].clone())
            .collect();

        Ok(Self { relays })
    }
}

impl Default for DmRelayList {
    fn default() -> Self {
        Self::new()
    }
}

/// Send a chat message to a recipient.
///
/// This wraps the message in a gift wrap and returns the event ready to publish.
///
/// # Arguments
/// * `message` - The chat message to send
/// * `sender_private_key` - Sender's private key (32 bytes)
/// * `recipient_public_key` - Recipient's public key (hex string)
/// * `timestamp` - Timestamp for the message
pub fn send_chat_message(
    message: &ChatMessage,
    sender_private_key: &[u8; 32],
    recipient_public_key: &str,
    timestamp: u64,
) -> Result<Event, Nip17Error> {
    let sender_pubkey = crate::nip01::get_public_key_hex(sender_private_key)?;
    let unsigned = message.to_unsigned_event(&sender_pubkey, timestamp);
    let wrap = gift_wrap(unsigned, sender_private_key, recipient_public_key)?;
    Ok(wrap)
}

/// Receive a chat message from a gift wrap.
///
/// # Arguments
/// * `wrap` - The gift wrap event received
/// * `recipient_private_key` - Recipient's private key (32 bytes)
pub fn receive_chat_message(
    wrap: &Event,
    recipient_private_key: &[u8; 32],
) -> Result<ChatMessage, Nip17Error> {
    let rumor = unwrap_gift_wrap_full(wrap, recipient_private_key)?;
    ChatMessage::from_rumor(&rumor)
}

/// Send a file message to a recipient.
///
/// This wraps the message in a gift wrap and returns the event ready to publish.
///
/// # Arguments
/// * `message` - The file message to send
/// * `sender_private_key` - Sender's private key (32 bytes)
/// * `recipient_public_key` - Recipient's public key (hex string)
/// * `timestamp` - Timestamp for the message
pub fn send_file_message(
    message: &FileMessage,
    sender_private_key: &[u8; 32],
    recipient_public_key: &str,
    timestamp: u64,
) -> Result<Event, Nip17Error> {
    let sender_pubkey = crate::nip01::get_public_key_hex(sender_private_key)?;
    let unsigned = message.to_unsigned_event(&sender_pubkey, timestamp);
    let wrap = gift_wrap(unsigned, sender_private_key, recipient_public_key)?;
    Ok(wrap)
}

/// Receive a file message from a gift wrap.
///
/// # Arguments
/// * `wrap` - The gift wrap event received
/// * `recipient_private_key` - Recipient's private key (32 bytes)
pub fn receive_file_message(
    wrap: &Event,
    recipient_private_key: &[u8; 32],
) -> Result<FileMessage, Nip17Error> {
    let rumor = unwrap_gift_wrap_full(wrap, recipient_private_key)?;
    FileMessage::from_rumor(&rumor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    #[test]
    fn test_chat_message_creation() {
        let message = ChatMessage::new("Hello, world!")
            .add_recipient("recipient1", Some("wss://relay.example.com".to_string()))
            .add_recipient("recipient2", None)
            .subject("Test Conversation");

        assert_eq!(message.content, "Hello, world!");
        assert_eq!(message.recipients.len(), 2);
        assert_eq!(message.subject, Some("Test Conversation".to_string()));
    }

    #[test]
    fn test_chat_message_to_unsigned_event() {
        let message = ChatMessage::new("Test message")
            .add_recipient("pubkey1", Some("wss://relay.com".to_string()))
            .reply_to("parent_event_id")
            .subject("Chat");

        let unsigned = message.to_unsigned_event("sender_pubkey", now());

        assert_eq!(unsigned.kind, KIND_CHAT_MESSAGE);
        assert_eq!(unsigned.content, "Test message");
        assert!(
            unsigned
                .tags
                .iter()
                .any(|t| t[0] == "p" && t[1] == "pubkey1")
        );
        assert!(
            unsigned
                .tags
                .iter()
                .any(|t| t[0] == "e" && t[1] == "parent_event_id")
        );
        assert!(
            unsigned
                .tags
                .iter()
                .any(|t| t[0] == "subject" && t[1] == "Chat")
        );
    }

    #[test]
    fn test_file_message_creation() {
        let message = FileMessage::new(
            "https://example.com/file.jpg",
            "image/jpeg",
            "aes-gcm",
            "decryption_key",
            "nonce",
            "sha256hash",
        )
        .add_recipient("recipient1", None)
        .size(12345)
        .dimensions(1920, 1080)
        .blurhash("LEHV6nWB2yk8pyo0adR*.7kCMdnj");

        assert_eq!(message.file_url, "https://example.com/file.jpg");
        assert_eq!(message.file_type, "image/jpeg");
        assert_eq!(message.size, Some(12345));
        assert_eq!(message.dimensions, Some("1920x1080".to_string()));
    }

    #[test]
    fn test_file_message_to_unsigned_event() {
        let message = FileMessage::new(
            "https://example.com/file.jpg",
            "image/jpeg",
            "aes-gcm",
            "key",
            "nonce",
            "hash",
        )
        .add_recipient("pubkey1", None);

        let unsigned = message.to_unsigned_event("sender_pubkey", now());

        assert_eq!(unsigned.kind, KIND_FILE_MESSAGE);
        assert_eq!(unsigned.content, "https://example.com/file.jpg");
        assert!(
            unsigned
                .tags
                .iter()
                .any(|t| t[0] == "file-type" && t[1] == "image/jpeg")
        );
        assert!(
            unsigned
                .tags
                .iter()
                .any(|t| t[0] == "encryption-algorithm" && t[1] == "aes-gcm")
        );
        assert!(unsigned.tags.iter().any(|t| t[0] == "x" && t[1] == "hash"));
    }

    #[test]
    fn test_dm_relay_list() {
        let relay_list = DmRelayList::new()
            .add_relay("wss://relay1.example.com")
            .add_relay("wss://relay2.example.com")
            .add_relay("wss://relay3.example.com");

        assert_eq!(relay_list.relays.len(), 3);

        let unsigned = relay_list.to_unsigned_event("pubkey", now());
        assert_eq!(unsigned.kind, KIND_DM_RELAY_LIST);
        assert_eq!(unsigned.tags.len(), 3);
        assert!(unsigned.tags.iter().all(|t| t[0] == "relay"));
    }

    #[test]
    fn test_send_and_receive_chat_message() {
        let sender_sk = crate::nip01::generate_secret_key();
        let recipient_sk = crate::nip01::generate_secret_key();
        let recipient_pk = crate::nip01::get_public_key_hex(&recipient_sk).unwrap();

        let message = ChatMessage::new("Hello from sender!")
            .add_recipient(&recipient_pk, None)
            .subject("Test");

        // Send
        let wrap = send_chat_message(&message, &sender_sk, &recipient_pk, now()).unwrap();

        assert_eq!(wrap.kind, crate::nip59::KIND_GIFT_WRAP);

        // Receive
        let received = receive_chat_message(&wrap, &recipient_sk).unwrap();

        assert_eq!(received.content, "Hello from sender!");
        assert_eq!(received.subject, Some("Test".to_string()));
        assert_eq!(received.recipients.len(), 1);
    }

    #[test]
    fn test_send_and_receive_file_message() {
        let sender_sk = crate::nip01::generate_secret_key();
        let recipient_sk = crate::nip01::generate_secret_key();
        let recipient_pk = crate::nip01::get_public_key_hex(&recipient_sk).unwrap();

        let message = FileMessage::new(
            "https://example.com/photo.jpg",
            "image/jpeg",
            "aes-gcm",
            "decryption_key_123",
            "nonce_456",
            "abcdef123456",
        )
        .add_recipient(&recipient_pk, None)
        .size(54321);

        // Send
        let wrap = send_file_message(&message, &sender_sk, &recipient_pk, now()).unwrap();

        assert_eq!(wrap.kind, crate::nip59::KIND_GIFT_WRAP);

        // Receive
        let received = receive_file_message(&wrap, &recipient_sk).unwrap();

        assert_eq!(received.file_url, "https://example.com/photo.jpg");
        assert_eq!(received.file_type, "image/jpeg");
        assert_eq!(received.decryption_key, "decryption_key_123");
        assert_eq!(received.size, Some(54321));
    }

    #[test]
    fn test_quoted_event() {
        let message = ChatMessage::new("Replying to this:").quote_event(QuotedEvent {
            id_or_address: "event123".to_string(),
            relay: Some("wss://relay.com".to_string()),
            pubkey: Some("author_pk".to_string()),
        });

        assert_eq!(message.quoted_events.len(), 1);

        let unsigned = message.to_unsigned_event("sender", now());
        let q_tags: Vec<_> = unsigned.tags.iter().filter(|t| t[0] == "q").collect();
        assert_eq!(q_tags.len(), 1);
        assert_eq!(q_tags[0][1], "event123");
    }
}
