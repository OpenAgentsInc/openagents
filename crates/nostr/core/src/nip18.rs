//! NIP-18: Reposts
//!
//! Defines how users repost/share other events using kind 6 (repost of kind 1 text notes)
//! and kind 16 (generic repost of any event kind).
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/18.md>

use crate::Event;
use thiserror::Error;

/// Event kind for reposts (reposting kind 1 text notes)
pub const REPOST_KIND: u16 = 6;

/// Event kind for generic reposts (reposting any event kind except kind 1)
pub const GENERIC_REPOST_KIND: u16 = 16;

/// Errors that can occur during NIP-18 operations
#[derive(Debug, Error)]
pub enum Nip18Error {
    #[error("invalid event kind: expected 6 or 16, got {0}")]
    InvalidKind(u16),

    #[error("missing required e-tag")]
    MissingEventTag,

    #[error("invalid tag format: {0}")]
    InvalidTag(String),

    #[error("json parse error: {0}")]
    JsonParse(String),

    #[error("parse error: {0}")]
    Parse(String),
}

/// A repost of a kind 1 text note (kind 6)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repost {
    pub event: Event,
    pub reposted_event_id: String,
    pub reposted_author: Option<String>,
    pub reposted_event: Option<Event>,
    pub relay_url: Option<String>,
}

impl Repost {
    /// Create a repost from an event
    pub fn from_event(event: Event) -> Result<Self, Nip18Error> {
        if event.kind != REPOST_KIND {
            return Err(Nip18Error::InvalidKind(event.kind));
        }

        // Find the e-tag (required)
        let mut reposted_event_id = None;
        let mut relay_url = None;

        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "e" {
                if tag.len() > 1 {
                    reposted_event_id = Some(tag[1].clone());
                }
                if tag.len() > 2 && !tag[2].is_empty() {
                    relay_url = Some(tag[2].clone());
                }
                break;
            }
        }

        let reposted_event_id = reposted_event_id.ok_or(Nip18Error::MissingEventTag)?;

        // Find the p-tag (recommended)
        let mut reposted_author = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "p" && tag.len() > 1 {
                reposted_author = Some(tag[1].clone());
                break;
            }
        }

        // Try to parse the reposted event from content field (JSON stringified)
        let reposted_event = if !event.content.is_empty() {
            serde_json::from_str::<Event>(&event.content).ok()
            // Content may be empty or invalid, which is allowed
        } else {
            None
        };

        Ok(Self {
            event,
            reposted_event_id,
            reposted_author,
            reposted_event,
            relay_url,
        })
    }

    /// Get the reposted event ID
    pub fn get_reposted_event_id(&self) -> &str {
        &self.reposted_event_id
    }

    /// Get the reposted author pubkey (if available)
    pub fn get_reposted_author(&self) -> Option<&str> {
        self.reposted_author.as_deref()
    }

    /// Get the full reposted event (if included in content)
    pub fn get_reposted_event(&self) -> Option<&Event> {
        self.reposted_event.as_ref()
    }

    /// Get the relay URL hint
    pub fn get_relay_url(&self) -> Option<&str> {
        self.relay_url.as_deref()
    }

    /// Get the reposter's public key
    pub fn reposter_pubkey(&self) -> &str {
        &self.event.pubkey
    }

    /// Get the timestamp of the repost
    pub fn created_at(&self) -> u64 {
        self.event.created_at
    }

    /// Validate the repost structure
    pub fn validate(&self) -> Result<(), Nip18Error> {
        if self.event.kind != REPOST_KIND {
            return Err(Nip18Error::InvalidKind(self.event.kind));
        }

        // Ensure e-tag exists
        let has_e_tag = self
            .event
            .tags
            .iter()
            .any(|tag| !tag.is_empty() && tag[0] == "e");

        if !has_e_tag {
            return Err(Nip18Error::MissingEventTag);
        }

        Ok(())
    }
}

/// A generic repost of any event kind except kind 1 (kind 16)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericRepost {
    pub event: Event,
    pub reposted_event_id: Option<String>,
    pub reposted_author: Option<String>,
    pub reposted_event_kind: Option<u16>,
    pub reposted_event: Option<Event>,
    pub addressable_coords: Option<String>,
    pub relay_url: Option<String>,
}

impl GenericRepost {
    /// Create a generic repost from an event
    pub fn from_event(event: Event) -> Result<Self, Nip18Error> {
        if event.kind != GENERIC_REPOST_KIND {
            return Err(Nip18Error::InvalidKind(event.kind));
        }

        // Find the e-tag (optional for replaceable events)
        let mut reposted_event_id = None;
        let mut relay_url = None;

        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "e" {
                if tag.len() > 1 {
                    reposted_event_id = Some(tag[1].clone());
                }
                if tag.len() > 2 && !tag[2].is_empty() {
                    relay_url = Some(tag[2].clone());
                }
                break;
            }
        }

        // Find the p-tag (recommended)
        let mut reposted_author = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "p" && tag.len() > 1 {
                reposted_author = Some(tag[1].clone());
                break;
            }
        }

        // Find the k-tag (recommended - stringified kind number)
        let mut reposted_event_kind = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "k" && tag.len() > 1 {
                if let Ok(kind) = tag[1].parse::<u16>() {
                    reposted_event_kind = Some(kind);
                }
                break;
            }
        }

        // Find the a-tag (for replaceable events)
        let mut addressable_coords = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "a" && tag.len() > 1 {
                addressable_coords = Some(tag[1].clone());
                break;
            }
        }

        // Try to parse the reposted event from content field
        let reposted_event = if !event.content.is_empty() {
            serde_json::from_str::<Event>(&event.content).ok()
        } else {
            None
        };

        Ok(Self {
            event,
            reposted_event_id,
            reposted_author,
            reposted_event_kind,
            reposted_event,
            addressable_coords,
            relay_url,
        })
    }

    /// Get the reposted event ID (if available)
    pub fn get_reposted_event_id(&self) -> Option<&str> {
        self.reposted_event_id.as_deref()
    }

    /// Get the reposted author pubkey (if available)
    pub fn get_reposted_author(&self) -> Option<&str> {
        self.reposted_author.as_deref()
    }

    /// Get the reposted event kind (if available)
    pub fn get_reposted_event_kind(&self) -> Option<u16> {
        self.reposted_event_kind
    }

    /// Get the full reposted event (if included in content)
    pub fn get_reposted_event(&self) -> Option<&Event> {
        self.reposted_event.as_ref()
    }

    /// Get the addressable event coordinates (for replaceable events)
    pub fn get_addressable_coords(&self) -> Option<&str> {
        self.addressable_coords.as_deref()
    }

    /// Get the relay URL hint
    pub fn get_relay_url(&self) -> Option<&str> {
        self.relay_url.as_deref()
    }

    /// Get the reposter's public key
    pub fn reposter_pubkey(&self) -> &str {
        &self.event.pubkey
    }

    /// Get the timestamp of the repost
    pub fn created_at(&self) -> u64 {
        self.event.created_at
    }

    /// Validate the generic repost structure
    pub fn validate(&self) -> Result<(), Nip18Error> {
        if self.event.kind != GENERIC_REPOST_KIND {
            return Err(Nip18Error::InvalidKind(self.event.kind));
        }

        // For non-addressable events, either e-tag or content with full event is required
        if self.addressable_coords.is_none()
            && self.reposted_event_id.is_none()
            && self.reposted_event.is_none()
        {
            return Err(Nip18Error::Parse(
                "generic repost must have either a-tag, e-tag, or full event in content"
                    .to_string(),
            ));
        }

        Ok(())
    }
}

/// Helper function to check if an event kind is a repost
pub fn is_repost_kind(kind: u16) -> bool {
    kind == REPOST_KIND || kind == GENERIC_REPOST_KIND
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_repost_event(reposted_id: &str, author: Option<&str>, content: &str) -> Event {
        let mut tags = vec![vec!["e".to_string(), reposted_id.to_string()]];

        if let Some(author_pubkey) = author {
            tags.push(vec!["p".to_string(), author_pubkey.to_string()]);
        }

        Event {
            id: "repost_id".to_string(),
            pubkey: "reposter_pubkey".to_string(),
            created_at: 1234567890,
            kind: REPOST_KIND,
            tags,
            content: content.to_string(),
            sig: "test_sig".to_string(),
        }
    }

    fn create_test_generic_repost_event(
        reposted_id: Option<&str>,
        author: Option<&str>,
        kind: Option<u16>,
        content: &str,
    ) -> Event {
        let mut tags = Vec::new();

        if let Some(id) = reposted_id {
            tags.push(vec!["e".to_string(), id.to_string()]);
        }

        if let Some(author_pubkey) = author {
            tags.push(vec!["p".to_string(), author_pubkey.to_string()]);
        }

        if let Some(k) = kind {
            tags.push(vec!["k".to_string(), k.to_string()]);
        }

        Event {
            id: "generic_repost_id".to_string(),
            pubkey: "reposter_pubkey".to_string(),
            created_at: 1234567890,
            kind: GENERIC_REPOST_KIND,
            tags,
            content: content.to_string(),
            sig: "test_sig".to_string(),
        }
    }

    #[test]
    fn test_repost_from_event() {
        let event = create_test_repost_event("event123", Some("author456"), "");
        let repost = Repost::from_event(event).unwrap();

        assert_eq!(repost.reposted_event_id, "event123");
        assert_eq!(repost.reposted_author, Some("author456".to_string()));
        assert_eq!(repost.get_reposted_event_id(), "event123");
        assert_eq!(repost.get_reposted_author(), Some("author456"));
    }

    #[test]
    fn test_repost_from_event_with_content() {
        let original_event = Event {
            id: "original123".to_string(),
            pubkey: "original_author".to_string(),
            created_at: 1234567800,
            kind: 1,
            tags: vec![],
            content: "Original note".to_string(),
            sig: "original_sig".to_string(),
        };

        let content = serde_json::to_string(&original_event).unwrap();
        let event = create_test_repost_event("original123", Some("original_author"), &content);
        let repost = Repost::from_event(event).unwrap();

        assert!(repost.get_reposted_event().is_some());
        let reposted = repost.get_reposted_event().unwrap();
        assert_eq!(reposted.id, "original123");
        assert_eq!(reposted.content, "Original note");
    }

    #[test]
    fn test_repost_missing_e_tag() {
        let event = Event {
            id: "repost_id".to_string(),
            pubkey: "reposter_pubkey".to_string(),
            created_at: 1234567890,
            kind: REPOST_KIND,
            tags: vec![],
            content: "".to_string(),
            sig: "test_sig".to_string(),
        };

        let result = Repost::from_event(event);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip18Error::MissingEventTag));
    }

    #[test]
    fn test_repost_invalid_kind() {
        let mut event = create_test_repost_event("event123", Some("author456"), "");
        event.kind = 1;

        let result = Repost::from_event(event);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip18Error::InvalidKind(1)));
    }

    #[test]
    fn test_repost_validate() {
        let event = create_test_repost_event("event123", Some("author456"), "");
        let repost = Repost::from_event(event).unwrap();
        assert!(repost.validate().is_ok());
    }

    #[test]
    fn test_repost_reposter_pubkey() {
        let event = create_test_repost_event("event123", Some("author456"), "");
        let repost = Repost::from_event(event).unwrap();
        assert_eq!(repost.reposter_pubkey(), "reposter_pubkey");
    }

    #[test]
    fn test_repost_created_at() {
        let event = create_test_repost_event("event123", Some("author456"), "");
        let repost = Repost::from_event(event).unwrap();
        assert_eq!(repost.created_at(), 1234567890);
    }

    #[test]
    fn test_generic_repost_from_event() {
        let event =
            create_test_generic_repost_event(Some("event123"), Some("author456"), Some(3), "");
        let repost = GenericRepost::from_event(event).unwrap();

        assert_eq!(repost.reposted_event_id, Some("event123".to_string()));
        assert_eq!(repost.reposted_author, Some("author456".to_string()));
        assert_eq!(repost.reposted_event_kind, Some(3));
    }

    #[test]
    fn test_generic_repost_with_addressable() {
        let mut event = create_test_generic_repost_event(None, Some("author456"), Some(30023), "");
        event.tags.push(vec![
            "a".to_string(),
            "30023:author456:article-slug".to_string(),
        ]);

        let repost = GenericRepost::from_event(event).unwrap();
        assert_eq!(
            repost.get_addressable_coords(),
            Some("30023:author456:article-slug")
        );
    }

    #[test]
    fn test_generic_repost_with_content() {
        let original_event = Event {
            id: "original123".to_string(),
            pubkey: "original_author".to_string(),
            created_at: 1234567800,
            kind: 3,
            tags: vec![],
            content: "Contact list".to_string(),
            sig: "original_sig".to_string(),
        };

        let content = serde_json::to_string(&original_event).unwrap();
        let event = create_test_generic_repost_event(
            Some("original123"),
            Some("original_author"),
            Some(3),
            &content,
        );
        let repost = GenericRepost::from_event(event).unwrap();

        assert!(repost.get_reposted_event().is_some());
        let reposted = repost.get_reposted_event().unwrap();
        assert_eq!(reposted.kind, 3);
        assert_eq!(reposted.content, "Contact list");
    }

    #[test]
    fn test_generic_repost_validate() {
        let event =
            create_test_generic_repost_event(Some("event123"), Some("author456"), Some(3), "");
        let repost = GenericRepost::from_event(event).unwrap();
        assert!(repost.validate().is_ok());
    }

    #[test]
    fn test_generic_repost_validate_missing_required() {
        let event = create_test_generic_repost_event(None, Some("author456"), Some(3), "");
        let repost = GenericRepost::from_event(event).unwrap();
        assert!(repost.validate().is_err());
    }

    #[test]
    fn test_is_repost_kind() {
        assert!(is_repost_kind(REPOST_KIND));
        assert!(is_repost_kind(GENERIC_REPOST_KIND));
        assert!(!is_repost_kind(1));
        assert!(!is_repost_kind(7));
    }
}
