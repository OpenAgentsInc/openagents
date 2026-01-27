//! NIP-10: Text Notes and Threads
//!
//! Defines conventions for replies, mentions, and threading in kind 1 text notes
//! using e-tags with markers (root, reply, mention) and p-tags for participants.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/10.md>

use crate::Event;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

/// Event kind for text notes
pub const TEXT_NOTE_KIND: u16 = 1;

/// Errors that can occur during NIP-10 operations
#[derive(Debug, Error)]
pub enum Nip10Error {
    #[error("invalid event kind: expected 1, got {0}")]
    InvalidKind(u16),

    #[error("invalid e-tag format: {0}")]
    InvalidETag(String),

    #[error("parse error: {0}")]
    Parse(String),
}

/// E-tag marker types for thread structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ETagMarker {
    /// The root event of a thread
    Root,

    /// The immediate parent event being replied to
    Reply,

    /// A mentioned event (not part of reply chain)
    Mention,
}

impl ETagMarker {
    pub fn as_str(&self) -> &'static str {
        match self {
            ETagMarker::Root => "root",
            ETagMarker::Reply => "reply",
            ETagMarker::Mention => "mention",
        }
    }
}

impl std::str::FromStr for ETagMarker {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "root" => Ok(ETagMarker::Root),
            "reply" => Ok(ETagMarker::Reply),
            "mention" => Ok(ETagMarker::Mention),
            _ => Err(()),
        }
    }
}

/// An e-tag reference to another event
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventReference {
    /// Event ID being referenced
    pub event_id: String,

    /// Recommended relay URL (optional)
    pub relay_url: Option<String>,

    /// Marker indicating role in thread (optional)
    pub marker: Option<ETagMarker>,

    /// Author's public key (optional but recommended)
    pub author_pubkey: Option<String>,
}

impl EventReference {
    /// Parse an event reference from an e-tag
    pub fn from_tag(tag: &[String]) -> Result<Self, Nip10Error> {
        if tag.is_empty() || tag[0] != "e" {
            return Err(Nip10Error::InvalidETag(
                "tag must start with 'e'".to_string(),
            ));
        }

        if tag.len() < 2 {
            return Err(Nip10Error::InvalidETag(
                "e-tag must have at least event ID".to_string(),
            ));
        }

        let event_id = tag[1].clone();

        let relay_url = if tag.len() > 2 && !tag[2].is_empty() {
            Some(tag[2].clone())
        } else {
            None
        };

        let marker = if tag.len() > 3 && !tag[3].is_empty() {
            ETagMarker::from_str(&tag[3]).ok()
        } else {
            None
        };

        let author_pubkey = if tag.len() > 4 && !tag[4].is_empty() {
            Some(tag[4].clone())
        } else {
            None
        };

        Ok(Self {
            event_id,
            relay_url,
            marker,
            author_pubkey,
        })
    }

    /// Convert to an e-tag array
    pub fn to_tag(&self) -> Vec<String> {
        let mut tag = vec!["e".to_string(), self.event_id.clone()];

        // Add relay URL (empty string if None)
        tag.push(self.relay_url.clone().unwrap_or_default());

        // Add marker if present
        if let Some(ref marker) = self.marker {
            tag.push(marker.as_str().to_string());

            // Add author pubkey if present
            if let Some(ref pubkey) = self.author_pubkey {
                tag.push(pubkey.clone());
            }
        }

        tag
    }
}

/// A text note with thread/reply metadata
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextNote {
    pub event: Event,
    pub event_references: Vec<EventReference>,
    pub mentioned_pubkeys: Vec<String>,
}

impl TextNote {
    /// Create a text note from an event
    pub fn from_event(event: Event) -> Result<Self, Nip10Error> {
        if event.kind != TEXT_NOTE_KIND {
            return Err(Nip10Error::InvalidKind(event.kind));
        }

        let mut event_references = Vec::new();
        let mut mentioned_pubkeys = Vec::new();

        // Parse e-tags
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "e" {
                event_references.push(EventReference::from_tag(tag)?);
            }
        }

        // Parse p-tags
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "p" && tag.len() > 1 {
                mentioned_pubkeys.push(tag[1].clone());
            }
        }

        Ok(Self {
            event,
            event_references,
            mentioned_pubkeys,
        })
    }

    /// Check if this is a reply to another event
    pub fn is_reply(&self) -> bool {
        !self.event_references.is_empty()
    }

    /// Check if this is a root post (not a reply)
    pub fn is_root_post(&self) -> bool {
        self.event_references.is_empty()
    }

    /// Get the root event of the thread (if this is a reply)
    pub fn get_thread_root(&self) -> Option<&EventReference> {
        // First, look for an event reference with "root" marker
        for eref in &self.event_references {
            if matches!(eref.marker, Some(ETagMarker::Root)) {
                return Some(eref);
            }
        }

        // Fallback to deprecated positional interpretation:
        // If there are 2+ e-tags, first is root
        if self.event_references.len() >= 2 {
            return Some(&self.event_references[0]);
        }

        // If there's only one e-tag, it's both root and reply target
        if self.event_references.len() == 1 {
            return Some(&self.event_references[0]);
        }

        None
    }

    /// Get the immediate parent being replied to (if this is a reply)
    pub fn get_reply_target(&self) -> Option<&EventReference> {
        // First, look for an event reference with "reply" marker
        for eref in &self.event_references {
            if matches!(eref.marker, Some(ETagMarker::Reply)) {
                return Some(eref);
            }
        }

        // Fallback to deprecated positional interpretation:
        // If there are 2+ e-tags, last is the reply target
        if self.event_references.len() >= 2 {
            return self.event_references.last();
        }

        // If there's only one e-tag, it's both root and reply target
        if self.event_references.len() == 1 {
            return Some(&self.event_references[0]);
        }

        None
    }

    /// Get all mentioned events (events with "mention" marker or middle events in deprecated format)
    pub fn get_mentions(&self) -> Vec<&EventReference> {
        let mut mentions = Vec::new();

        for eref in &self.event_references {
            if matches!(eref.marker, Some(ETagMarker::Mention)) {
                mentions.push(eref);
            }
        }

        // If using modern markers, return only explicitly marked mentions
        if mentions.is_empty() && self.event_references.len() > 2 {
            // Deprecated: middle e-tags are mentions
            mentions.extend(&self.event_references[1..self.event_references.len() - 1]);
        }

        mentions
    }

    /// Get all mentioned public keys
    pub fn get_mentioned_pubkeys(&self) -> &[String] {
        &self.mentioned_pubkeys
    }

    /// Get the content of the text note
    pub fn content(&self) -> &str {
        &self.event.content
    }

    /// Get the author's public key
    pub fn author_pubkey(&self) -> &str {
        &self.event.pubkey
    }

    /// Get the timestamp
    pub fn created_at(&self) -> u64 {
        self.event.created_at
    }

    /// Validate the text note structure
    pub fn validate(&self) -> Result<(), Nip10Error> {
        if self.event.kind != TEXT_NOTE_KIND {
            return Err(Nip10Error::InvalidKind(self.event.kind));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(e_tags: Vec<Vec<String>>, p_tags: Vec<Vec<String>>) -> Event {
        let mut tags = e_tags;
        tags.extend(p_tags);

        Event {
            id: "note_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1234567890,
            kind: TEXT_NOTE_KIND,
            tags,
            content: "Hello Nostr!".to_string(),
            sig: "test_sig".to_string(),
        }
    }

    #[test]
    fn test_etag_marker_from_str() {
        assert!(matches!(
            ETagMarker::from_str("root"),
            Ok(ETagMarker::Root)
        ));
        assert!(matches!(
            ETagMarker::from_str("reply"),
            Ok(ETagMarker::Reply)
        ));
        assert!(matches!(
            ETagMarker::from_str("mention"),
            Ok(ETagMarker::Mention)
        ));
        assert!(ETagMarker::from_str("unknown").is_err());
    }

    #[test]
    fn test_etag_marker_as_str() {
        assert_eq!(ETagMarker::Root.as_str(), "root");
        assert_eq!(ETagMarker::Reply.as_str(), "reply");
        assert_eq!(ETagMarker::Mention.as_str(), "mention");
    }

    #[test]
    fn test_event_reference_from_tag_minimal() {
        let tag = vec!["e".to_string(), "event123".to_string()];
        let eref = EventReference::from_tag(&tag).unwrap();

        assert_eq!(eref.event_id, "event123");
        assert!(eref.relay_url.is_none());
        assert!(eref.marker.is_none());
        assert!(eref.author_pubkey.is_none());
    }

    #[test]
    fn test_event_reference_from_tag_with_relay() {
        let tag = vec![
            "e".to_string(),
            "event123".to_string(),
            "wss://relay.com".to_string(),
        ];
        let eref = EventReference::from_tag(&tag).unwrap();

        assert_eq!(eref.event_id, "event123");
        assert_eq!(eref.relay_url, Some("wss://relay.com".to_string()));
    }

    #[test]
    fn test_event_reference_from_tag_with_marker() {
        let tag = vec![
            "e".to_string(),
            "event123".to_string(),
            "wss://relay.com".to_string(),
            "root".to_string(),
        ];
        let eref = EventReference::from_tag(&tag).unwrap();

        assert_eq!(eref.event_id, "event123");
        assert_eq!(eref.marker, Some(ETagMarker::Root));
    }

    #[test]
    fn test_event_reference_from_tag_with_author() {
        let tag = vec![
            "e".to_string(),
            "event123".to_string(),
            "wss://relay.com".to_string(),
            "reply".to_string(),
            "author456".to_string(),
        ];
        let eref = EventReference::from_tag(&tag).unwrap();

        assert_eq!(eref.event_id, "event123");
        assert_eq!(eref.marker, Some(ETagMarker::Reply));
        assert_eq!(eref.author_pubkey, Some("author456".to_string()));
    }

    #[test]
    fn test_event_reference_to_tag() {
        let eref = EventReference {
            event_id: "event123".to_string(),
            relay_url: Some("wss://relay.com".to_string()),
            marker: Some(ETagMarker::Root),
            author_pubkey: Some("author456".to_string()),
        };

        let tag = eref.to_tag();
        assert_eq!(tag[0], "e");
        assert_eq!(tag[1], "event123");
        assert_eq!(tag[2], "wss://relay.com");
        assert_eq!(tag[3], "root");
        assert_eq!(tag[4], "author456");
    }

    #[test]
    fn test_text_note_root_post() {
        let event = create_test_event(vec![], vec![]);
        let note = TextNote::from_event(event).unwrap();

        assert!(note.is_root_post());
        assert!(!note.is_reply());
        assert!(note.get_thread_root().is_none());
        assert!(note.get_reply_target().is_none());
    }

    #[test]
    fn test_text_note_simple_reply() {
        let e_tags = vec![vec!["e".to_string(), "parent123".to_string()]];
        let event = create_test_event(e_tags, vec![]);
        let note = TextNote::from_event(event).unwrap();

        assert!(!note.is_root_post());
        assert!(note.is_reply());

        let root = note.get_thread_root().unwrap();
        assert_eq!(root.event_id, "parent123");

        let reply = note.get_reply_target().unwrap();
        assert_eq!(reply.event_id, "parent123");
    }

    #[test]
    fn test_text_note_reply_with_markers() {
        let e_tags = vec![
            vec![
                "e".to_string(),
                "root123".to_string(),
                "".to_string(),
                "root".to_string(),
            ],
            vec![
                "e".to_string(),
                "parent456".to_string(),
                "".to_string(),
                "reply".to_string(),
            ],
        ];
        let event = create_test_event(e_tags, vec![]);
        let note = TextNote::from_event(event).unwrap();

        assert!(note.is_reply());

        let root = note.get_thread_root().unwrap();
        assert_eq!(root.event_id, "root123");

        let reply = note.get_reply_target().unwrap();
        assert_eq!(reply.event_id, "parent456");
    }

    #[test]
    fn test_text_note_deprecated_positional() {
        // Deprecated format: [root, mention, reply]
        let e_tags = vec![
            vec!["e".to_string(), "root123".to_string()],
            vec!["e".to_string(), "mention456".to_string()],
            vec!["e".to_string(), "parent789".to_string()],
        ];
        let event = create_test_event(e_tags, vec![]);
        let note = TextNote::from_event(event).unwrap();

        let root = note.get_thread_root().unwrap();
        assert_eq!(root.event_id, "root123");

        let reply = note.get_reply_target().unwrap();
        assert_eq!(reply.event_id, "parent789");

        let mentions = note.get_mentions();
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].event_id, "mention456");
    }

    #[test]
    fn test_text_note_with_mentions() {
        let e_tags = vec![
            vec![
                "e".to_string(),
                "root123".to_string(),
                "".to_string(),
                "root".to_string(),
            ],
            vec![
                "e".to_string(),
                "mention1".to_string(),
                "".to_string(),
                "mention".to_string(),
            ],
            vec![
                "e".to_string(),
                "mention2".to_string(),
                "".to_string(),
                "mention".to_string(),
            ],
            vec![
                "e".to_string(),
                "parent456".to_string(),
                "".to_string(),
                "reply".to_string(),
            ],
        ];
        let event = create_test_event(e_tags, vec![]);
        let note = TextNote::from_event(event).unwrap();

        let mentions = note.get_mentions();
        assert_eq!(mentions.len(), 2);
        assert_eq!(mentions[0].event_id, "mention1");
        assert_eq!(mentions[1].event_id, "mention2");
    }

    #[test]
    fn test_text_note_with_p_tags() {
        let p_tags = vec![
            vec!["p".to_string(), "user1".to_string()],
            vec!["p".to_string(), "user2".to_string()],
        ];
        let event = create_test_event(vec![], p_tags);
        let note = TextNote::from_event(event).unwrap();

        let pubkeys = note.get_mentioned_pubkeys();
        assert_eq!(pubkeys.len(), 2);
        assert_eq!(pubkeys[0], "user1");
        assert_eq!(pubkeys[1], "user2");
    }

    #[test]
    fn test_text_note_content_and_metadata() {
        let event = create_test_event(vec![], vec![]);
        let note = TextNote::from_event(event).unwrap();

        assert_eq!(note.content(), "Hello Nostr!");
        assert_eq!(note.author_pubkey(), "author_pubkey");
        assert_eq!(note.created_at(), 1234567890);
    }

    #[test]
    fn test_text_note_invalid_kind() {
        let mut event = create_test_event(vec![], vec![]);
        event.kind = 3;

        let result = TextNote::from_event(event);
        assert!(result.is_err());
    }

    #[test]
    fn test_text_note_validate() {
        let event = create_test_event(vec![], vec![]);
        let note = TextNote::from_event(event).unwrap();
        assert!(note.validate().is_ok());
    }
}
