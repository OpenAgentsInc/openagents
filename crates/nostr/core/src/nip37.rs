//! NIP-37: Draft Wraps
//!
//! Defines kind 31234 as encrypted storage for unsigned draft events and kind 10013
//! for relay lists for private content.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/37.md>

use crate::Event;
use serde_json;
use thiserror::Error;

/// Event kind for draft wraps (encrypted storage for draft events)
pub const DRAFT_WRAP_KIND: u16 = 31234;

/// Event kind for relay list for private content
pub const PRIVATE_CONTENT_RELAY_LIST_KIND: u16 = 10013;

/// Errors that can occur during NIP-37 operations
#[derive(Debug, Error)]
pub enum Nip37Error {
    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("missing required d-tag")]
    MissingDTag,

    #[error("missing required k-tag")]
    MissingKTag,

    #[error("invalid k-tag value: {0}")]
    InvalidKTag(String),

    #[error("invalid tag format: {0}")]
    InvalidTag(String),

    #[error("draft content is encrypted, decryption required")]
    EncryptedContent,

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("parse error: {0}")]
    Parse(String),
}

/// A draft wrap event (kind 31234) that stores an encrypted draft event
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftWrap {
    pub event: Event,
    pub identifier: String,
    pub draft_kind: u16,
    pub expiration: Option<u64>,
}

impl DraftWrap {
    /// Create a draft wrap from an event
    pub fn from_event(event: Event) -> Result<Self, Nip37Error> {
        if event.kind != DRAFT_WRAP_KIND {
            return Err(Nip37Error::InvalidKind {
                expected: DRAFT_WRAP_KIND,
                actual: event.kind,
            });
        }

        // Find the d-tag (required for addressable events)
        let mut identifier = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "d" && tag.len() > 1 {
                identifier = Some(tag[1].clone());
                break;
            }
        }

        let identifier = identifier.ok_or(Nip37Error::MissingDTag)?;

        // Find the k-tag (required)
        let mut draft_kind = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "k" && tag.len() > 1 {
                draft_kind = Some(
                    tag[1]
                        .parse::<u16>()
                        .map_err(|_| Nip37Error::InvalidKTag(tag[1].clone()))?,
                );
                break;
            }
        }

        let draft_kind = draft_kind.ok_or(Nip37Error::MissingKTag)?;

        // Find expiration tag (optional but recommended)
        let mut expiration = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "expiration" && tag.len() > 1 {
                if let Ok(timestamp) = tag[1].parse::<u64>() {
                    expiration = Some(timestamp);
                }
                break;
            }
        }

        Ok(Self {
            event,
            identifier,
            draft_kind,
            expiration,
        })
    }

    /// Get the draft's unique identifier (d-tag value)
    pub fn get_identifier(&self) -> &str {
        &self.identifier
    }

    /// Get the kind of the draft event
    pub fn get_draft_kind(&self) -> u16 {
        self.draft_kind
    }

    /// Get the expiration timestamp (if set)
    pub fn get_expiration(&self) -> Option<u64> {
        self.expiration
    }

    /// Get the encrypted content (requires NIP-44 decryption to access the draft)
    pub fn get_encrypted_content(&self) -> &str {
        &self.event.content
    }

    /// Check if the draft has been deleted (blank content)
    pub fn is_deleted(&self) -> bool {
        self.event.content.is_empty()
    }

    /// Get the author's public key
    pub fn get_author(&self) -> &str {
        &self.event.pubkey
    }

    /// Get the creation/update timestamp
    pub fn get_created_at(&self) -> u64 {
        self.event.created_at
    }

    /// Construct the addressable event coordinate (kind:pubkey:d-tag)
    pub fn get_coordinate(&self) -> String {
        format!(
            "{}:{}:{}",
            self.event.kind, self.event.pubkey, self.identifier
        )
    }

    /// Validate the draft wrap structure
    pub fn validate(&self) -> Result<(), Nip37Error> {
        if self.event.kind != DRAFT_WRAP_KIND {
            return Err(Nip37Error::InvalidKind {
                expected: DRAFT_WRAP_KIND,
                actual: self.event.kind,
            });
        }

        // Ensure d-tag exists
        let has_d_tag = self
            .event
            .tags
            .iter()
            .any(|tag| !tag.is_empty() && tag[0] == "d");

        if !has_d_tag {
            return Err(Nip37Error::MissingDTag);
        }

        // Ensure k-tag exists
        let has_k_tag = self
            .event
            .tags
            .iter()
            .any(|tag| !tag.is_empty() && tag[0] == "k");

        if !has_k_tag {
            return Err(Nip37Error::MissingKTag);
        }

        Ok(())
    }
}

/// A relay list for private content (kind 10013)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateContentRelayList {
    pub event: Event,
}

impl PrivateContentRelayList {
    /// Create a private content relay list from an event
    pub fn from_event(event: Event) -> Result<Self, Nip37Error> {
        if event.kind != PRIVATE_CONTENT_RELAY_LIST_KIND {
            return Err(Nip37Error::InvalidKind {
                expected: PRIVATE_CONTENT_RELAY_LIST_KIND,
                actual: event.kind,
            });
        }

        Ok(Self { event })
    }

    /// Get the encrypted content (requires NIP-44 decryption to access relay list)
    pub fn get_encrypted_content(&self) -> &str {
        &self.event.content
    }

    /// Get the author's public key
    pub fn get_author(&self) -> &str {
        &self.event.pubkey
    }

    /// Get the creation/update timestamp
    pub fn get_created_at(&self) -> u64 {
        self.event.created_at
    }

    /// Validate the relay list structure
    pub fn validate(&self) -> Result<(), Nip37Error> {
        if self.event.kind != PRIVATE_CONTENT_RELAY_LIST_KIND {
            return Err(Nip37Error::InvalidKind {
                expected: PRIVATE_CONTENT_RELAY_LIST_KIND,
                actual: self.event.kind,
            });
        }

        Ok(())
    }
}

/// Helper function to check if an event kind is a draft wrap
pub fn is_draft_wrap_kind(kind: u16) -> bool {
    kind == DRAFT_WRAP_KIND
}

/// Helper function to check if an event kind is a private content relay list
pub fn is_private_content_relay_list_kind(kind: u16) -> bool {
    kind == PRIVATE_CONTENT_RELAY_LIST_KIND
}

/// Helper function to check if an event kind is NIP-37 related
pub fn is_nip37_kind(kind: u16) -> bool {
    is_draft_wrap_kind(kind) || is_private_content_relay_list_kind(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_draft_wrap_event(
        identifier: &str,
        draft_kind: u16,
        content: &str,
        expiration: Option<u64>,
    ) -> Event {
        let mut tags = vec![
            vec!["d".to_string(), identifier.to_string()],
            vec!["k".to_string(), draft_kind.to_string()],
        ];

        if let Some(exp) = expiration {
            tags.push(vec!["expiration".to_string(), exp.to_string()]);
        }

        Event {
            id: "draft_wrap_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1675642635,
            kind: DRAFT_WRAP_KIND,
            tags,
            content: content.to_string(),
            sig: "test_sig".to_string(),
        }
    }

    fn create_test_relay_list_event(content: &str) -> Event {
        Event {
            id: "relay_list_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1675642635,
            kind: PRIVATE_CONTENT_RELAY_LIST_KIND,
            tags: vec![],
            content: content.to_string(),
            sig: "test_sig".to_string(),
        }
    }

    #[test]
    fn test_draft_wrap_from_event_minimal() {
        let event = create_test_draft_wrap_event("my-draft-1", 1, "encrypted_content_here", None);
        let draft = DraftWrap::from_event(event).unwrap();

        assert_eq!(draft.get_identifier(), "my-draft-1");
        assert_eq!(draft.get_draft_kind(), 1);
        assert!(draft.get_expiration().is_none());
        assert_eq!(draft.get_encrypted_content(), "encrypted_content_here");
        assert!(!draft.is_deleted());
    }

    #[test]
    fn test_draft_wrap_with_expiration() {
        let event = create_test_draft_wrap_event(
            "my-draft-2",
            30023,
            "encrypted_content",
            Some(1683000000),
        );
        let draft = DraftWrap::from_event(event).unwrap();

        assert_eq!(draft.get_identifier(), "my-draft-2");
        assert_eq!(draft.get_draft_kind(), 30023);
        assert_eq!(draft.get_expiration(), Some(1683000000));
    }

    #[test]
    fn test_draft_wrap_deleted() {
        let event = create_test_draft_wrap_event("my-draft-3", 1, "", None);
        let draft = DraftWrap::from_event(event).unwrap();

        assert!(draft.is_deleted());
    }

    #[test]
    fn test_draft_wrap_coordinate() {
        let event = create_test_draft_wrap_event("my-draft", 1, "content", None);
        let draft = DraftWrap::from_event(event).unwrap();
        assert_eq!(draft.get_coordinate(), "31234:author_pubkey:my-draft");
    }

    #[test]
    fn test_draft_wrap_missing_d_tag() {
        let event = Event {
            id: "draft_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1675642635,
            kind: DRAFT_WRAP_KIND,
            tags: vec![vec!["k".to_string(), "1".to_string()]],
            content: "content".to_string(),
            sig: "test_sig".to_string(),
        };

        let result = DraftWrap::from_event(event);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip37Error::MissingDTag));
    }

    #[test]
    fn test_draft_wrap_missing_k_tag() {
        let event = Event {
            id: "draft_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1675642635,
            kind: DRAFT_WRAP_KIND,
            tags: vec![vec!["d".to_string(), "my-draft".to_string()]],
            content: "content".to_string(),
            sig: "test_sig".to_string(),
        };

        let result = DraftWrap::from_event(event);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip37Error::MissingKTag));
    }

    #[test]
    fn test_draft_wrap_invalid_k_tag() {
        let event = Event {
            id: "draft_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1675642635,
            kind: DRAFT_WRAP_KIND,
            tags: vec![
                vec!["d".to_string(), "my-draft".to_string()],
                vec!["k".to_string(), "not_a_number".to_string()],
            ],
            content: "content".to_string(),
            sig: "test_sig".to_string(),
        };

        let result = DraftWrap::from_event(event);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip37Error::InvalidKTag(_)));
    }

    #[test]
    fn test_draft_wrap_invalid_kind() {
        let mut event = create_test_draft_wrap_event("draft", 1, "content", None);
        event.kind = 1;

        let result = DraftWrap::from_event(event);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip37Error::InvalidKind {
                expected: DRAFT_WRAP_KIND,
                actual: 1
            }
        ));
    }

    #[test]
    fn test_draft_wrap_validate() {
        let event = create_test_draft_wrap_event("draft", 1, "content", None);
        let draft = DraftWrap::from_event(event).unwrap();
        assert!(draft.validate().is_ok());
    }

    #[test]
    fn test_draft_wrap_get_author() {
        let event = create_test_draft_wrap_event("draft", 1, "content", None);
        let draft = DraftWrap::from_event(event).unwrap();
        assert_eq!(draft.get_author(), "author_pubkey");
    }

    #[test]
    fn test_draft_wrap_get_created_at() {
        let event = create_test_draft_wrap_event("draft", 1, "content", None);
        let draft = DraftWrap::from_event(event).unwrap();
        assert_eq!(draft.get_created_at(), 1675642635);
    }

    #[test]
    fn test_relay_list_from_event() {
        let event = create_test_relay_list_event("encrypted_relay_list");
        let relay_list = PrivateContentRelayList::from_event(event).unwrap();

        assert_eq!(relay_list.get_encrypted_content(), "encrypted_relay_list");
        assert_eq!(relay_list.get_author(), "author_pubkey");
        assert_eq!(relay_list.get_created_at(), 1675642635);
    }

    #[test]
    fn test_relay_list_invalid_kind() {
        let mut event = create_test_relay_list_event("content");
        event.kind = 1;

        let result = PrivateContentRelayList::from_event(event);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip37Error::InvalidKind {
                expected: PRIVATE_CONTENT_RELAY_LIST_KIND,
                actual: 1
            }
        ));
    }

    #[test]
    fn test_relay_list_validate() {
        let event = create_test_relay_list_event("content");
        let relay_list = PrivateContentRelayList::from_event(event).unwrap();
        assert!(relay_list.validate().is_ok());
    }

    #[test]
    fn test_is_draft_wrap_kind() {
        assert!(is_draft_wrap_kind(DRAFT_WRAP_KIND));
        assert!(!is_draft_wrap_kind(1));
        assert!(!is_draft_wrap_kind(10013));
    }

    #[test]
    fn test_is_private_content_relay_list_kind() {
        assert!(is_private_content_relay_list_kind(
            PRIVATE_CONTENT_RELAY_LIST_KIND
        ));
        assert!(!is_private_content_relay_list_kind(1));
        assert!(!is_private_content_relay_list_kind(31234));
    }

    #[test]
    fn test_is_nip37_kind() {
        assert!(is_nip37_kind(DRAFT_WRAP_KIND));
        assert!(is_nip37_kind(PRIVATE_CONTENT_RELAY_LIST_KIND));
        assert!(!is_nip37_kind(1));
        assert!(!is_nip37_kind(7));
    }
}
