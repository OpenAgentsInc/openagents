//! NIP-25: Reactions
//!
//! Defines how users react to other notes using kind 7 events.
//! Reactions can be likes (+), dislikes (-), or custom emoji reactions.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/25.md>

use crate::Event;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Event kind for reactions to Nostr events
pub const REACTION_KIND: u16 = 7;

/// Event kind for reactions to external content
pub const EXTERNAL_REACTION_KIND: u16 = 17;

/// Errors that can occur during NIP-25 operations
#[derive(Debug, Error)]
pub enum Nip25Error {
    #[error("invalid event kind: expected 7 or 17, got {0}")]
    InvalidKind(u16),

    #[error("missing required e-tag")]
    MissingEventTag,

    #[error("invalid tag format: {0}")]
    InvalidTag(String),

    #[error("parse error: {0}")]
    Parse(String),
}

/// Type of reaction
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReactionType {
    /// Like/upvote (content: "+" or empty)
    Like,

    /// Dislike/downvote (content: "-")
    Dislike,

    /// Custom emoji or text reaction
    Custom(String),
}

impl ReactionType {
    /// Parse a reaction type from content string
    pub fn from_content(content: &str) -> Self {
        match content {
            "" | "+" => ReactionType::Like,
            "-" => ReactionType::Dislike,
            custom => ReactionType::Custom(custom.to_string()),
        }
    }

    /// Convert reaction type to content string
    pub fn to_content(&self) -> String {
        match self {
            ReactionType::Like => "+".to_string(),
            ReactionType::Dislike => "-".to_string(),
            ReactionType::Custom(s) => s.clone(),
        }
    }

    /// Check if this is a like reaction
    pub fn is_like(&self) -> bool {
        matches!(self, ReactionType::Like)
    }

    /// Check if this is a dislike reaction
    pub fn is_dislike(&self) -> bool {
        matches!(self, ReactionType::Dislike)
    }

    /// Check if this is a custom reaction
    pub fn is_custom(&self) -> bool {
        matches!(self, ReactionType::Custom(_))
    }
}

/// A reaction to a Nostr event or external content
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reaction {
    pub event: Event,
    pub reaction_type: ReactionType,
    pub reacted_event_id: String,
    pub reacted_event_author: Option<String>,
    pub reacted_event_kind: Option<u16>,
    pub addressable_event_coords: Option<String>,
}

impl Reaction {
    /// Create a new reaction from an event
    pub fn from_event(event: Event) -> Result<Self, Nip25Error> {
        if event.kind != REACTION_KIND && event.kind != EXTERNAL_REACTION_KIND {
            return Err(Nip25Error::InvalidKind(event.kind));
        }

        let reaction_type = ReactionType::from_content(&event.content);

        // Find the e-tag (required)
        let mut reacted_event_id = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "e"
                && tag.len() > 1
            {
                reacted_event_id = Some(tag[1].clone());
            }
        }

        let reacted_event_id = reacted_event_id.ok_or(Nip25Error::MissingEventTag)?;

        // Find the p-tag (recommended)
        let mut reacted_event_author = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "p"
                && tag.len() > 1
            {
                reacted_event_author = Some(tag[1].clone());
            }
        }

        // Find the k-tag (optional)
        let mut reacted_event_kind = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "k"
                && tag.len() > 1
                && let Ok(kind) = tag[1].parse::<u16>()
            {
                reacted_event_kind = Some(kind);
            }
        }

        // Find the a-tag (optional, for addressable events)
        let mut addressable_event_coords = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "a"
                && tag.len() > 1
            {
                addressable_event_coords = Some(tag[1].clone());
            }
        }

        Ok(Self {
            event,
            reaction_type,
            reacted_event_id,
            reacted_event_author,
            reacted_event_kind,
            addressable_event_coords,
        })
    }

    /// Get the reaction content
    pub fn content(&self) -> &str {
        &self.event.content
    }

    /// Get the reactor's public key
    pub fn reactor_pubkey(&self) -> &str {
        &self.event.pubkey
    }

    /// Get the timestamp of the reaction
    pub fn created_at(&self) -> u64 {
        self.event.created_at
    }

    /// Check if this is a like reaction
    pub fn is_like(&self) -> bool {
        self.reaction_type.is_like()
    }

    /// Check if this is a dislike reaction
    pub fn is_dislike(&self) -> bool {
        self.reaction_type.is_dislike()
    }

    /// Check if this is a custom reaction
    pub fn is_custom(&self) -> bool {
        self.reaction_type.is_custom()
    }

    /// Validate the reaction structure
    pub fn validate(&self) -> Result<(), Nip25Error> {
        // Ensure event kind is correct
        if self.event.kind != REACTION_KIND && self.event.kind != EXTERNAL_REACTION_KIND {
            return Err(Nip25Error::InvalidKind(self.event.kind));
        }

        // Ensure e-tag exists
        let has_e_tag = self
            .event
            .tags
            .iter()
            .any(|tag| !tag.is_empty() && tag[0] == "e");

        if !has_e_tag {
            return Err(Nip25Error::MissingEventTag);
        }

        Ok(())
    }
}

/// Helper function to check if an event kind is a reaction
pub fn is_reaction_kind(kind: u16) -> bool {
    kind == REACTION_KIND || kind == EXTERNAL_REACTION_KIND
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_reaction_event(content: &str, event_id: &str, author: Option<&str>) -> Event {
        let mut tags = vec![vec!["e".to_string(), event_id.to_string()]];

        if let Some(author_pubkey) = author {
            tags.push(vec!["p".to_string(), author_pubkey.to_string()]);
        }

        Event {
            id: "reaction_id".to_string(),
            pubkey: "reactor_pubkey".to_string(),
            created_at: 1234567890,
            kind: REACTION_KIND,
            tags,
            content: content.to_string(),
            sig: "test_sig".to_string(),
        }
    }

    #[test]
    fn test_reaction_type_from_content() {
        assert_eq!(ReactionType::from_content("+"), ReactionType::Like);
        assert_eq!(ReactionType::from_content(""), ReactionType::Like);
        assert_eq!(ReactionType::from_content("-"), ReactionType::Dislike);
        assert_eq!(
            ReactionType::from_content("❤️"),
            ReactionType::Custom("❤️".to_string())
        );
        assert_eq!(
            ReactionType::from_content("🔥"),
            ReactionType::Custom("🔥".to_string())
        );
    }

    #[test]
    fn test_reaction_type_to_content() {
        assert_eq!(ReactionType::Like.to_content(), "+");
        assert_eq!(ReactionType::Dislike.to_content(), "-");
        assert_eq!(ReactionType::Custom("❤️".to_string()).to_content(), "❤️");
    }

    #[test]
    fn test_reaction_type_checks() {
        assert!(ReactionType::Like.is_like());
        assert!(!ReactionType::Like.is_dislike());
        assert!(!ReactionType::Like.is_custom());

        assert!(!ReactionType::Dislike.is_like());
        assert!(ReactionType::Dislike.is_dislike());
        assert!(!ReactionType::Dislike.is_custom());

        let custom = ReactionType::Custom("🔥".to_string());
        assert!(!custom.is_like());
        assert!(!custom.is_dislike());
        assert!(custom.is_custom());
    }

    #[test]
    fn test_reaction_from_event_like() {
        let event = create_test_reaction_event("+", "event123", Some("author456"));
        let reaction = Reaction::from_event(event).unwrap();

        assert_eq!(reaction.reaction_type, ReactionType::Like);
        assert_eq!(reaction.reacted_event_id, "event123");
        assert_eq!(reaction.reacted_event_author, Some("author456".to_string()));
        assert!(reaction.is_like());
    }

    #[test]
    fn test_reaction_from_event_dislike() {
        let event = create_test_reaction_event("-", "event123", Some("author456"));
        let reaction = Reaction::from_event(event).unwrap();

        assert_eq!(reaction.reaction_type, ReactionType::Dislike);
        assert!(reaction.is_dislike());
    }

    #[test]
    fn test_reaction_from_event_emoji() {
        let event = create_test_reaction_event("❤️", "event123", Some("author456"));
        let reaction = Reaction::from_event(event).unwrap();

        assert_eq!(
            reaction.reaction_type,
            ReactionType::Custom("❤️".to_string())
        );
        assert!(reaction.is_custom());
        assert_eq!(reaction.content(), "❤️");
    }

    #[test]
    fn test_reaction_from_event_no_author() {
        let event = create_test_reaction_event("+", "event123", None);
        let reaction = Reaction::from_event(event).unwrap();

        assert_eq!(reaction.reacted_event_id, "event123");
        assert_eq!(reaction.reacted_event_author, None);
    }

    #[test]
    fn test_reaction_from_event_with_kind_tag() {
        let mut event = create_test_reaction_event("+", "event123", Some("author456"));
        event.tags.push(vec!["k".to_string(), "1".to_string()]);

        let reaction = Reaction::from_event(event).unwrap();
        assert_eq!(reaction.reacted_event_kind, Some(1));
    }

    #[test]
    fn test_reaction_from_event_with_addressable_tag() {
        let mut event = create_test_reaction_event("+", "event123", Some("author456"));
        event.tags.push(vec![
            "a".to_string(),
            "30023:author456:my-article".to_string(),
        ]);

        let reaction = Reaction::from_event(event).unwrap();
        assert_eq!(
            reaction.addressable_event_coords,
            Some("30023:author456:my-article".to_string())
        );
    }

    #[test]
    fn test_reaction_missing_e_tag() {
        let event = Event {
            id: "reaction_id".to_string(),
            pubkey: "reactor_pubkey".to_string(),
            created_at: 1234567890,
            kind: REACTION_KIND,
            tags: vec![],
            content: "+".to_string(),
            sig: "test_sig".to_string(),
        };

        let result = Reaction::from_event(event);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip25Error::MissingEventTag));
    }

    #[test]
    fn test_reaction_invalid_kind() {
        let mut event = create_test_reaction_event("+", "event123", Some("author456"));
        event.kind = 1;

        let result = Reaction::from_event(event);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip25Error::InvalidKind(1)));
    }

    #[test]
    fn test_reaction_validate() {
        let event = create_test_reaction_event("+", "event123", Some("author456"));
        let reaction = Reaction::from_event(event).unwrap();
        assert!(reaction.validate().is_ok());
    }

    #[test]
    fn test_reaction_reactor_pubkey() {
        let event = create_test_reaction_event("+", "event123", Some("author456"));
        let reaction = Reaction::from_event(event).unwrap();
        assert_eq!(reaction.reactor_pubkey(), "reactor_pubkey");
    }

    #[test]
    fn test_reaction_created_at() {
        let event = create_test_reaction_event("+", "event123", Some("author456"));
        let reaction = Reaction::from_event(event).unwrap();
        assert_eq!(reaction.created_at(), 1234567890);
    }

    #[test]
    fn test_is_reaction_kind() {
        assert!(is_reaction_kind(REACTION_KIND));
        assert!(is_reaction_kind(EXTERNAL_REACTION_KIND));
        assert!(!is_reaction_kind(1));
        assert!(!is_reaction_kind(3));
    }

    #[test]
    fn test_reaction_empty_content_is_like() {
        let event = create_test_reaction_event("", "event123", Some("author456"));
        let reaction = Reaction::from_event(event).unwrap();
        assert!(reaction.is_like());
        assert_eq!(reaction.reaction_type, ReactionType::Like);
    }
}
