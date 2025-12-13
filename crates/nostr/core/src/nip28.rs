//! NIP-28: Public Chat
//!
//! This NIP defines event kinds for public chat channels, channel messages,
//! and basic client-side moderation.
//!
//! ## Kinds
//! - 40: Channel creation
//! - 41: Channel metadata update
//! - 42: Channel message
//! - 43: Hide message (moderation)
//! - 44: Mute user (moderation)

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind for channel creation
pub const KIND_CHANNEL_CREATION: u16 = 40;
/// Kind for channel metadata update
pub const KIND_CHANNEL_METADATA: u16 = 41;
/// Kind for channel message
pub const KIND_CHANNEL_MESSAGE: u16 = 42;
/// Kind for hiding a message
pub const KIND_CHANNEL_HIDE_MESSAGE: u16 = 43;
/// Kind for muting a user
pub const KIND_CHANNEL_MUTE_USER: u16 = 44;

/// Errors that can occur during NIP-28 operations.
#[derive(Debug, Error)]
pub enum Nip28Error {
    #[error("invalid channel metadata: {0}")]
    InvalidMetadata(String),

    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Channel metadata (name, about, picture, relays).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelMetadata {
    /// Channel name
    pub name: String,
    /// Channel description
    pub about: String,
    /// URL of channel picture
    pub picture: String,
    /// List of relays for the channel
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relays: Vec<String>,
}

impl ChannelMetadata {
    /// Create new channel metadata.
    pub fn new(
        name: impl Into<String>,
        about: impl Into<String>,
        picture: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            about: about.into(),
            picture: picture.into(),
            relays: Vec::new(),
        }
    }

    /// Add relays to the metadata.
    pub fn with_relays(mut self, relays: Vec<String>) -> Self {
        self.relays = relays;
        self
    }

    /// Serialize to JSON string.
    pub fn to_json(&self) -> Result<String, Nip28Error> {
        serde_json::to_string(self).map_err(|e| Nip28Error::Serialization(e.to_string()))
    }

    /// Parse from JSON string.
    pub fn from_json(json: &str) -> Result<Self, Nip28Error> {
        serde_json::from_str(json).map_err(|e| Nip28Error::InvalidMetadata(e.to_string()))
    }
}

/// Reason for hiding a message or muting a user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerationReason {
    /// The reason for the moderation action
    pub reason: String,
}

impl ModerationReason {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    /// Serialize to JSON string.
    pub fn to_json(&self) -> Result<String, Nip28Error> {
        serde_json::to_string(self).map_err(|e| Nip28Error::Serialization(e.to_string()))
    }

    /// Parse from JSON string.
    pub fn from_json(json: &str) -> Result<Self, Nip28Error> {
        serde_json::from_str(json).map_err(|e| Nip28Error::InvalidMetadata(e.to_string()))
    }
}

/// Template for creating a channel (kind 40).
#[derive(Debug, Clone)]
pub struct ChannelCreateEvent {
    /// Channel metadata
    pub metadata: ChannelMetadata,
    /// Unix timestamp
    pub created_at: u64,
    /// Additional tags
    pub tags: Vec<Vec<String>>,
}

impl ChannelCreateEvent {
    /// Create a new channel creation event.
    pub fn new(metadata: ChannelMetadata, created_at: u64) -> Self {
        Self {
            metadata,
            created_at,
            tags: Vec::new(),
        }
    }

    /// Add additional tags.
    pub fn with_tags(mut self, tags: Vec<Vec<String>>) -> Self {
        self.tags = tags;
        self
    }

    /// Get the content (JSON-serialized metadata).
    pub fn content(&self) -> Result<String, Nip28Error> {
        self.metadata.to_json()
    }

    /// Get all tags for the event.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        self.tags.clone()
    }
}

/// Template for updating channel metadata (kind 41).
#[derive(Debug, Clone)]
pub struct ChannelMetadataEvent {
    /// The channel creation event ID
    pub channel_create_event_id: String,
    /// Relay hint for the channel
    pub relay_url: Option<String>,
    /// Updated channel metadata
    pub metadata: ChannelMetadata,
    /// Unix timestamp
    pub created_at: u64,
    /// Category tags
    pub categories: Vec<String>,
    /// Additional tags
    pub tags: Vec<Vec<String>>,
}

impl ChannelMetadataEvent {
    /// Create a new channel metadata update event.
    pub fn new(
        channel_create_event_id: impl Into<String>,
        metadata: ChannelMetadata,
        created_at: u64,
    ) -> Self {
        Self {
            channel_create_event_id: channel_create_event_id.into(),
            relay_url: None,
            metadata,
            created_at,
            categories: Vec::new(),
            tags: Vec::new(),
        }
    }

    /// Set the relay URL hint.
    pub fn with_relay_url(mut self, relay_url: impl Into<String>) -> Self {
        self.relay_url = Some(relay_url.into());
        self
    }

    /// Add category tags.
    pub fn with_categories(mut self, categories: Vec<String>) -> Self {
        self.categories = categories;
        self
    }

    /// Add additional tags.
    pub fn with_tags(mut self, tags: Vec<Vec<String>>) -> Self {
        self.tags = tags;
        self
    }

    /// Get the content (JSON-serialized metadata).
    pub fn content(&self) -> Result<String, Nip28Error> {
        self.metadata.to_json()
    }

    /// Get all tags for the event.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // Add e tag with NIP-10 marker
        let mut e_tag = vec!["e".to_string(), self.channel_create_event_id.clone()];
        if let Some(relay) = &self.relay_url {
            e_tag.push(relay.clone());
            e_tag.push("root".to_string());
        }
        tags.push(e_tag);

        // Add category tags
        for category in &self.categories {
            tags.push(vec!["t".to_string(), category.clone()]);
        }

        // Add additional tags
        tags.extend(self.tags.clone());

        tags
    }
}

/// Template for a channel message (kind 42).
#[derive(Debug, Clone)]
pub struct ChannelMessageEvent {
    /// The channel creation event ID
    pub channel_create_event_id: String,
    /// If replying, the message event ID being replied to
    pub reply_to_event_id: Option<String>,
    /// Relay URL for the e tags
    pub relay_url: String,
    /// Message content
    pub content: String,
    /// Unix timestamp
    pub created_at: u64,
    /// Pubkeys to tag (for replies)
    pub mentioned_pubkeys: Vec<(String, Option<String>)>, // (pubkey, relay_url)
    /// Additional tags
    pub tags: Vec<Vec<String>>,
}

impl ChannelMessageEvent {
    /// Create a new root channel message.
    pub fn new(
        channel_create_event_id: impl Into<String>,
        relay_url: impl Into<String>,
        content: impl Into<String>,
        created_at: u64,
    ) -> Self {
        Self {
            channel_create_event_id: channel_create_event_id.into(),
            reply_to_event_id: None,
            relay_url: relay_url.into(),
            content: content.into(),
            created_at,
            mentioned_pubkeys: Vec::new(),
            tags: Vec::new(),
        }
    }

    /// Create a reply to another message.
    pub fn reply(
        channel_create_event_id: impl Into<String>,
        reply_to_event_id: impl Into<String>,
        relay_url: impl Into<String>,
        content: impl Into<String>,
        created_at: u64,
    ) -> Self {
        Self {
            channel_create_event_id: channel_create_event_id.into(),
            reply_to_event_id: Some(reply_to_event_id.into()),
            relay_url: relay_url.into(),
            content: content.into(),
            created_at,
            mentioned_pubkeys: Vec::new(),
            tags: Vec::new(),
        }
    }

    /// Add a mentioned pubkey (for replies).
    pub fn mention_pubkey(mut self, pubkey: impl Into<String>, relay_url: Option<String>) -> Self {
        self.mentioned_pubkeys.push((pubkey.into(), relay_url));
        self
    }

    /// Add additional tags.
    pub fn with_tags(mut self, tags: Vec<Vec<String>>) -> Self {
        self.tags = tags;
        self
    }

    /// Check if this is a reply.
    pub fn is_reply(&self) -> bool {
        self.reply_to_event_id.is_some()
    }

    /// Get all tags for the event.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // Root e tag (channel creation event)
        tags.push(vec![
            "e".to_string(),
            self.channel_create_event_id.clone(),
            self.relay_url.clone(),
            "root".to_string(),
        ]);

        // Reply e tag if this is a reply
        if let Some(reply_to) = &self.reply_to_event_id {
            tags.push(vec![
                "e".to_string(),
                reply_to.clone(),
                self.relay_url.clone(),
                "reply".to_string(),
            ]);
        }

        // p tags for mentioned pubkeys
        for (pubkey, relay) in &self.mentioned_pubkeys {
            let mut p_tag = vec!["p".to_string(), pubkey.clone()];
            if let Some(relay_url) = relay {
                p_tag.push(relay_url.clone());
            }
            tags.push(p_tag);
        }

        // Add additional tags
        tags.extend(self.tags.clone());

        tags
    }
}

/// Template for hiding a message (kind 43).
#[derive(Debug, Clone)]
pub struct ChannelHideMessageEvent {
    /// The message event ID to hide
    pub message_event_id: String,
    /// Reason for hiding (optional)
    pub reason: Option<ModerationReason>,
    /// Unix timestamp
    pub created_at: u64,
    /// Additional tags
    pub tags: Vec<Vec<String>>,
}

impl ChannelHideMessageEvent {
    /// Create a new hide message event.
    pub fn new(message_event_id: impl Into<String>, created_at: u64) -> Self {
        Self {
            message_event_id: message_event_id.into(),
            reason: None,
            created_at,
            tags: Vec::new(),
        }
    }

    /// Set the reason for hiding.
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(ModerationReason::new(reason));
        self
    }

    /// Add additional tags.
    pub fn with_tags(mut self, tags: Vec<Vec<String>>) -> Self {
        self.tags = tags;
        self
    }

    /// Get the content (JSON-serialized reason or empty).
    pub fn content(&self) -> Result<String, Nip28Error> {
        match &self.reason {
            Some(reason) => reason.to_json(),
            None => Ok(String::new()),
        }
    }

    /// Get all tags for the event.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec!["e".to_string(), self.message_event_id.clone()]];
        tags.extend(self.tags.clone());
        tags
    }
}

/// Template for muting a user (kind 44).
#[derive(Debug, Clone)]
pub struct ChannelMuteUserEvent {
    /// The pubkey to mute
    pub pubkey_to_mute: String,
    /// Reason for muting (optional)
    pub reason: Option<ModerationReason>,
    /// Unix timestamp
    pub created_at: u64,
    /// Additional tags
    pub tags: Vec<Vec<String>>,
}

impl ChannelMuteUserEvent {
    /// Create a new mute user event.
    pub fn new(pubkey_to_mute: impl Into<String>, created_at: u64) -> Self {
        Self {
            pubkey_to_mute: pubkey_to_mute.into(),
            reason: None,
            created_at,
            tags: Vec::new(),
        }
    }

    /// Set the reason for muting.
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(ModerationReason::new(reason));
        self
    }

    /// Add additional tags.
    pub fn with_tags(mut self, tags: Vec<Vec<String>>) -> Self {
        self.tags = tags;
        self
    }

    /// Get the content (JSON-serialized reason or empty).
    pub fn content(&self) -> Result<String, Nip28Error> {
        match &self.reason {
            Some(reason) => reason.to_json(),
            None => Ok(String::new()),
        }
    }

    /// Get all tags for the event.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec!["p".to_string(), self.pubkey_to_mute.clone()]];
        tags.extend(self.tags.clone());
        tags
    }
}

/// Check if a kind is a channel-related kind (40-44).
pub fn is_channel_kind(kind: u16) -> bool {
    (KIND_CHANNEL_CREATION..=KIND_CHANNEL_MUTE_USER).contains(&kind)
}

/// Check if a kind is a channel creation kind.
pub fn is_channel_creation_kind(kind: u16) -> bool {
    kind == KIND_CHANNEL_CREATION
}

/// Check if a kind is a channel metadata kind.
pub fn is_channel_metadata_kind(kind: u16) -> bool {
    kind == KIND_CHANNEL_METADATA
}

/// Check if a kind is a channel message kind.
pub fn is_channel_message_kind(kind: u16) -> bool {
    kind == KIND_CHANNEL_MESSAGE
}

/// Check if a kind is a moderation kind (hide message or mute user).
pub fn is_moderation_kind(kind: u16) -> bool {
    kind == KIND_CHANNEL_HIDE_MESSAGE || kind == KIND_CHANNEL_MUTE_USER
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Kind validation tests
    // =========================================================================

    #[test]
    fn test_channel_kinds() {
        assert_eq!(KIND_CHANNEL_CREATION, 40);
        assert_eq!(KIND_CHANNEL_METADATA, 41);
        assert_eq!(KIND_CHANNEL_MESSAGE, 42);
        assert_eq!(KIND_CHANNEL_HIDE_MESSAGE, 43);
        assert_eq!(KIND_CHANNEL_MUTE_USER, 44);
    }

    #[test]
    fn test_is_channel_kind() {
        assert!(is_channel_kind(40));
        assert!(is_channel_kind(41));
        assert!(is_channel_kind(42));
        assert!(is_channel_kind(43));
        assert!(is_channel_kind(44));

        assert!(!is_channel_kind(39));
        assert!(!is_channel_kind(45));
        assert!(!is_channel_kind(1));
    }

    #[test]
    fn test_is_channel_creation_kind() {
        assert!(is_channel_creation_kind(40));
        assert!(!is_channel_creation_kind(41));
    }

    #[test]
    fn test_is_channel_metadata_kind() {
        assert!(is_channel_metadata_kind(41));
        assert!(!is_channel_metadata_kind(40));
    }

    #[test]
    fn test_is_channel_message_kind() {
        assert!(is_channel_message_kind(42));
        assert!(!is_channel_message_kind(40));
    }

    #[test]
    fn test_is_moderation_kind() {
        assert!(is_moderation_kind(43));
        assert!(is_moderation_kind(44));
        assert!(!is_moderation_kind(42));
    }

    // =========================================================================
    // ChannelMetadata tests
    // =========================================================================

    #[test]
    fn test_channel_metadata_new() {
        let metadata = ChannelMetadata::new(
            "Test Channel",
            "A test channel",
            "https://example.com/pic.jpg",
        );

        assert_eq!(metadata.name, "Test Channel");
        assert_eq!(metadata.about, "A test channel");
        assert_eq!(metadata.picture, "https://example.com/pic.jpg");
        assert!(metadata.relays.is_empty());
    }

    #[test]
    fn test_channel_metadata_with_relays() {
        let metadata = ChannelMetadata::new("Test", "About", "https://pic.com").with_relays(vec![
            "wss://relay1.com".to_string(),
            "wss://relay2.com".to_string(),
        ]);

        assert_eq!(metadata.relays.len(), 2);
        assert_eq!(metadata.relays[0], "wss://relay1.com");
    }

    #[test]
    fn test_channel_metadata_json_roundtrip() {
        let metadata = ChannelMetadata::new(
            "Test Channel",
            "This is a test channel",
            "https://placekitten.com/200/200",
        )
        .with_relays(vec!["wss://nos.lol".to_string()]);

        let json = metadata.to_json().unwrap();
        let parsed = ChannelMetadata::from_json(&json).unwrap();

        assert_eq!(metadata, parsed);
    }

    #[test]
    fn test_channel_metadata_json_format() {
        let metadata = ChannelMetadata::new(
            "Demo Channel",
            "A test channel.",
            "https://placekitten.com/200/200",
        )
        .with_relays(vec![
            "wss://nos.lol".to_string(),
            "wss://nostr.mom".to_string(),
        ]);

        let json = metadata.to_json().unwrap();

        // Should contain all fields
        assert!(json.contains("\"name\":\"Demo Channel\""));
        assert!(json.contains("\"about\":\"A test channel.\""));
        assert!(json.contains("\"picture\":\"https://placekitten.com/200/200\""));
        assert!(json.contains("\"relays\":[\"wss://nos.lol\",\"wss://nostr.mom\"]"));
    }

    // =========================================================================
    // ModerationReason tests
    // =========================================================================

    #[test]
    fn test_moderation_reason() {
        let reason = ModerationReason::new("Inappropriate content");
        assert_eq!(reason.reason, "Inappropriate content");
    }

    #[test]
    fn test_moderation_reason_json() {
        let reason = ModerationReason::new("Dick pic");
        let json = reason.to_json().unwrap();
        assert_eq!(json, r#"{"reason":"Dick pic"}"#);

        let parsed = ModerationReason::from_json(&json).unwrap();
        assert_eq!(parsed.reason, "Dick pic");
    }

    // =========================================================================
    // ChannelCreateEvent tests (mirrors nostr-tools)
    // =========================================================================

    #[test]
    fn test_channel_create_event() {
        let metadata = ChannelMetadata::new(
            "Test Channel",
            "This is a test channel",
            "https://example.com/picture.jpg",
        );

        let event = ChannelCreateEvent::new(metadata.clone(), 1617932115);

        assert_eq!(event.metadata, metadata);
        assert_eq!(event.created_at, 1617932115);
        assert!(event.tags.is_empty());
    }

    #[test]
    fn test_channel_create_event_content() {
        let metadata = ChannelMetadata::new(
            "Test Channel",
            "This is a test channel",
            "https://example.com/picture.jpg",
        );

        let event = ChannelCreateEvent::new(metadata.clone(), 1617932115);
        let content = event.content().unwrap();

        assert_eq!(content, serde_json::to_string(&metadata).unwrap());
    }

    // =========================================================================
    // ChannelMetadataEvent tests (mirrors nostr-tools)
    // =========================================================================

    #[test]
    fn test_channel_metadata_event() {
        let metadata = ChannelMetadata::new(
            "Test Channel",
            "This is a test channel",
            "https://example.com/picture.jpg",
        );

        let event =
            ChannelMetadataEvent::new("channel_creation_event_id", metadata.clone(), 1617932115);

        assert_eq!(event.channel_create_event_id, "channel_creation_event_id");
        assert_eq!(event.metadata, metadata);
    }

    #[test]
    fn test_channel_metadata_event_tags() {
        let metadata = ChannelMetadata::new("Test", "About", "https://pic.com");

        let event = ChannelMetadataEvent::new("channel_id", metadata, 1617932115);
        let tags = event.to_tags();

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], vec!["e", "channel_id"]);
    }

    #[test]
    fn test_channel_metadata_event_with_categories() {
        let metadata = ChannelMetadata::new("Test", "About", "https://pic.com");

        let event = ChannelMetadataEvent::new("channel_id", metadata, 1617932115)
            .with_categories(vec!["bitcoin".to_string(), "nostr".to_string()]);

        let tags = event.to_tags();

        assert!(tags.iter().any(|t| t == &vec!["t", "bitcoin"]));
        assert!(tags.iter().any(|t| t == &vec!["t", "nostr"]));
    }

    // =========================================================================
    // ChannelMessageEvent tests (mirrors nostr-tools)
    // =========================================================================

    #[test]
    fn test_channel_message_event_root() {
        let event = ChannelMessageEvent::new(
            "channel_creation_event_id",
            "https://relay.example.com",
            "Hello, world!",
            1617932115,
        );

        assert_eq!(event.channel_create_event_id, "channel_creation_event_id");
        assert_eq!(event.content, "Hello, world!");
        assert!(!event.is_reply());
    }

    #[test]
    fn test_channel_message_event_root_tags() {
        let event = ChannelMessageEvent::new(
            "channel_creation_event_id",
            "https://relay.example.com",
            "Hello, world!",
            1617932115,
        );

        let tags = event.to_tags();

        assert_eq!(tags.len(), 1);
        assert_eq!(
            tags[0],
            vec![
                "e",
                "channel_creation_event_id",
                "https://relay.example.com",
                "root"
            ]
        );
    }

    #[test]
    fn test_channel_message_event_reply() {
        let event = ChannelMessageEvent::reply(
            "channel_creation_event_id",
            "message_event_id",
            "https://relay.example.com",
            "This is a reply!",
            1617932115,
        );

        assert!(event.is_reply());
        assert_eq!(
            event.reply_to_event_id,
            Some("message_event_id".to_string())
        );
    }

    #[test]
    fn test_channel_message_event_reply_tags() {
        let event = ChannelMessageEvent::reply(
            "channel_creation_event_id",
            "channel_message_event_id",
            "https://relay.example.com",
            "Hello, world!",
            1617932115,
        );

        let tags = event.to_tags();

        // Should have root tag
        let root_tag = tags.iter().find(|t| t.len() >= 4 && t[3] == "root");
        assert!(root_tag.is_some());
        assert_eq!(root_tag.unwrap()[1], "channel_creation_event_id");

        // Should have reply tag
        let reply_tag = tags.iter().find(|t| t.len() >= 4 && t[3] == "reply");
        assert!(reply_tag.is_some());
        assert_eq!(reply_tag.unwrap()[1], "channel_message_event_id");
    }

    #[test]
    fn test_channel_message_event_with_mentions() {
        let event = ChannelMessageEvent::reply(
            "channel_id",
            "reply_to_id",
            "wss://relay.com",
            "Hey @someone!",
            1617932115,
        )
        .mention_pubkey("pubkey123", Some("wss://relay.com".to_string()));

        let tags = event.to_tags();

        let p_tag = tags.iter().find(|t| t[0] == "p");
        assert!(p_tag.is_some());
        assert_eq!(p_tag.unwrap()[1], "pubkey123");
    }

    // =========================================================================
    // ChannelHideMessageEvent tests (mirrors nostr-tools)
    // =========================================================================

    #[test]
    fn test_channel_hide_message_event() {
        let event = ChannelHideMessageEvent::new("channel_message_event_id", 1617932115)
            .with_reason("Inappropriate content");

        assert_eq!(event.message_event_id, "channel_message_event_id");
        assert!(event.reason.is_some());
        assert_eq!(
            event.reason.as_ref().unwrap().reason,
            "Inappropriate content"
        );
    }

    #[test]
    fn test_channel_hide_message_event_tags() {
        let event = ChannelHideMessageEvent::new("channel_message_event_id", 1617932115);
        let tags = event.to_tags();

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], vec!["e", "channel_message_event_id"]);
    }

    #[test]
    fn test_channel_hide_message_event_content() {
        let event =
            ChannelHideMessageEvent::new("msg_id", 1617932115).with_reason("Inappropriate content");

        let content = event.content().unwrap();
        assert_eq!(content, r#"{"reason":"Inappropriate content"}"#);
    }

    #[test]
    fn test_channel_hide_message_event_no_reason() {
        let event = ChannelHideMessageEvent::new("msg_id", 1617932115);
        let content = event.content().unwrap();
        assert_eq!(content, "");
    }

    // =========================================================================
    // ChannelMuteUserEvent tests (mirrors nostr-tools)
    // =========================================================================

    #[test]
    fn test_channel_mute_user_event() {
        let event = ChannelMuteUserEvent::new("pubkey_to_mute", 1617932115).with_reason("Spamming");

        assert_eq!(event.pubkey_to_mute, "pubkey_to_mute");
        assert!(event.reason.is_some());
        assert_eq!(event.reason.as_ref().unwrap().reason, "Spamming");
    }

    #[test]
    fn test_channel_mute_user_event_tags() {
        let event = ChannelMuteUserEvent::new("pubkey_to_mute", 1617932115);
        let tags = event.to_tags();

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], vec!["p", "pubkey_to_mute"]);
    }

    #[test]
    fn test_channel_mute_user_event_content() {
        let event =
            ChannelMuteUserEvent::new("pubkey", 1617932115).with_reason("Posting dick pics");

        let content = event.content().unwrap();
        assert_eq!(content, r#"{"reason":"Posting dick pics"}"#);
    }

    // =========================================================================
    // Integration tests
    // =========================================================================

    #[test]
    fn test_channel_workflow() {
        // 1. Create a channel
        let metadata = ChannelMetadata::new(
            "Bitcoin Discussion",
            "A channel for discussing Bitcoin",
            "https://bitcoin.org/img/icons/logotop.svg",
        )
        .with_relays(vec!["wss://relay.damus.io".to_string()]);

        let create_event = ChannelCreateEvent::new(metadata.clone(), 1617932115);
        assert_eq!(create_event.content().unwrap(), metadata.to_json().unwrap());

        // 2. Update channel metadata
        let updated_metadata = ChannelMetadata::new(
            "Bitcoin Discussion (Updated)",
            "The best channel for Bitcoin discussion",
            "https://bitcoin.org/img/icons/logotop.svg",
        );

        let metadata_event =
            ChannelMetadataEvent::new("channel_create_id", updated_metadata, 1617932200)
                .with_categories(vec!["bitcoin".to_string(), "crypto".to_string()]);

        let tags = metadata_event.to_tags();
        assert!(tags.iter().any(|t| t[0] == "e"));
        assert!(tags.iter().any(|t| t == &vec!["t", "bitcoin"]));

        // 3. Post a message
        let msg = ChannelMessageEvent::new(
            "channel_create_id",
            "wss://relay.damus.io",
            "Hello everyone!",
            1617932300,
        );
        assert!(!msg.is_reply());

        // 4. Reply to the message
        let reply = ChannelMessageEvent::reply(
            "channel_create_id",
            "msg_event_id",
            "wss://relay.damus.io",
            "Hi there!",
            1617932400,
        )
        .mention_pubkey("original_author_pubkey", None);

        assert!(reply.is_reply());
        let reply_tags = reply.to_tags();
        assert!(reply_tags.iter().any(|t| t.len() >= 4 && t[3] == "root"));
        assert!(reply_tags.iter().any(|t| t.len() >= 4 && t[3] == "reply"));

        // 5. Hide a message
        let hide = ChannelHideMessageEvent::new("spam_msg_id", 1617932500).with_reason("Spam");
        assert_eq!(hide.to_tags()[0], vec!["e", "spam_msg_id"]);

        // 6. Mute a user
        let mute =
            ChannelMuteUserEvent::new("spammer_pubkey", 1617932600).with_reason("Repeated spam");
        assert_eq!(mute.to_tags()[0], vec!["p", "spammer_pubkey"]);
    }
}
