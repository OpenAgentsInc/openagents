//! Channel management for NIP-28 public chat.

use nostr::ChannelMetadata;

/// A public chat channel (NIP-28).
#[derive(Debug, Clone)]
pub struct Channel {
    /// Channel creation event ID
    pub id: String,
    /// Channel metadata (name, about, picture)
    pub metadata: ChannelMetadata,
    /// Creator's public key
    pub creator_pubkey: String,
    /// Creation timestamp
    pub created_at: u64,
    /// Relay URL hint
    pub relay_url: Option<String>,
}

impl Channel {
    /// Get the channel name.
    pub fn name(&self) -> &str {
        &self.metadata.name
    }

    /// Get the channel description.
    pub fn about(&self) -> &str {
        &self.metadata.about
    }

    /// Get the channel picture URL.
    pub fn picture(&self) -> &str {
        &self.metadata.picture
    }

    /// Check if this channel was created by the given pubkey.
    pub fn is_creator(&self, pubkey: &str) -> bool {
        self.creator_pubkey == pubkey
    }
}

/// A channel in the list view (with unread count).
#[derive(Debug, Clone)]
pub struct ChannelListItem {
    /// Channel ID
    pub id: String,
    /// Channel name
    pub name: String,
    /// Unread message count
    pub unread_count: u32,
    /// Last message preview
    pub last_message: Option<String>,
    /// Last message timestamp
    pub last_message_at: Option<u64>,
    /// Whether this channel is selected
    pub selected: bool,
}

impl From<&Channel> for ChannelListItem {
    fn from(channel: &Channel) -> Self {
        Self {
            id: channel.id.clone(),
            name: channel.metadata.name.clone(),
            unread_count: 0,
            last_message: None,
            last_message_at: None,
            selected: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel() {
        let channel = Channel {
            id: "abc123".to_string(),
            metadata: ChannelMetadata::new("Test Channel", "A test channel", "https://pic.com"),
            creator_pubkey: "pubkey123".to_string(),
            created_at: 1234567890,
            relay_url: Some("wss://relay.example.com".to_string()),
        };

        assert_eq!(channel.name(), "Test Channel");
        assert_eq!(channel.about(), "A test channel");
        assert_eq!(channel.picture(), "https://pic.com");
        assert!(channel.is_creator("pubkey123"));
        assert!(!channel.is_creator("other"));
    }

    #[test]
    fn test_channel_list_item_from() {
        let channel = Channel {
            id: "abc123".to_string(),
            metadata: ChannelMetadata::new("Test", "About", "https://pic.com"),
            creator_pubkey: "pubkey123".to_string(),
            created_at: 1234567890,
            relay_url: None,
        };

        let item = ChannelListItem::from(&channel);

        assert_eq!(item.id, "abc123");
        assert_eq!(item.name, "Test");
        assert_eq!(item.unread_count, 0);
        assert!(item.last_message.is_none());
        assert!(!item.selected);
    }
}
