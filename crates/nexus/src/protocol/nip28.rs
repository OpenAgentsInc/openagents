//! NIP-28: Public Chat
//!
//! Channel creation and messaging for public chat channels.
//! The relay stores and routes these events like any other,
//! but clients can filter by these kinds for chat functionality.

/// Channel creation event kind
pub const KIND_CHANNEL_CREATE: u16 = 40;

/// Channel metadata update kind
pub const KIND_CHANNEL_METADATA: u16 = 41;

/// Channel message kind
pub const KIND_CHANNEL_MESSAGE: u16 = 42;

/// Hide channel message kind (moderation)
pub const KIND_CHANNEL_HIDE: u16 = 43;

/// Mute user in channel kind (moderation)
pub const KIND_CHANNEL_MUTE: u16 = 44;

/// Check if a kind is a channel-related kind
pub fn is_channel_kind(kind: u16) -> bool {
    matches!(kind, KIND_CHANNEL_CREATE | KIND_CHANNEL_METADATA | KIND_CHANNEL_MESSAGE | KIND_CHANNEL_HIDE | KIND_CHANNEL_MUTE)
}

/// Check if a kind is a channel message kind
pub fn is_channel_message_kind(kind: u16) -> bool {
    kind == KIND_CHANNEL_MESSAGE
}

/// Extract the root channel ID from a channel message event
pub fn get_channel_id(event: &nostr::Event) -> Option<String> {
    if event.kind != KIND_CHANNEL_MESSAGE {
        return None;
    }

    // Look for root "e" tag
    for tag in &event.tags {
        if tag.len() >= 4 && tag[0] == "e" && tag[3] == "root" {
            return Some(tag[1].clone());
        }
    }

    // Fall back to first "e" tag
    for tag in &event.tags {
        if tag.len() >= 2 && tag[0] == "e" {
            return Some(tag[1].clone());
        }
    }

    None
}
