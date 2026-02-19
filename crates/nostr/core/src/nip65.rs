//! NIP-65: Relay List Metadata
//!
//! Defines a replaceable event (kind 10002) to advertise relays where the user
//! writes to and relays where the user reads mentions from. This enables better
//! relay discovery and more efficient event distribution.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/65.md>

use crate::Event;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

/// Event kind for relay list metadata
pub const RELAY_LIST_METADATA_KIND: u16 = 10002;

/// Tag name for relay entries
pub const RELAY_TAG: &str = "r";

/// Marker for read-only relays
pub const READ_MARKER: &str = "read";

/// Marker for write-only relays
pub const WRITE_MARKER: &str = "write";

/// Errors that can occur during NIP-65 operations
#[derive(Debug, Error)]
pub enum Nip65Error {
    #[error("event is not a relay list metadata event (kind {0})")]
    InvalidKind(u16),

    #[error("invalid relay tag format: {0}")]
    InvalidTag(String),

    #[error("invalid relay URL: {0}")]
    InvalidUrl(String),
}

/// Relay marker indicating usage type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RelayMarker {
    /// Relay is used for reading events
    Read,
    /// Relay is used for writing events
    Write,
    /// Relay is used for both reading and writing (default)
    ReadWrite,
}

impl RelayMarker {
    /// Convert to string (returns None for ReadWrite as it's implicit)
    pub fn to_str(&self) -> Option<&str> {
        match self {
            RelayMarker::Read => Some("read"),
            RelayMarker::Write => Some("write"),
            RelayMarker::ReadWrite => None,
        }
    }

    /// Check if this relay supports reading
    pub fn can_read(&self) -> bool {
        matches!(self, RelayMarker::Read | RelayMarker::ReadWrite)
    }

    /// Check if this relay supports writing
    pub fn can_write(&self) -> bool {
        matches!(self, RelayMarker::Write | RelayMarker::ReadWrite)
    }
}

impl std::str::FromStr for RelayMarker {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "read" => RelayMarker::Read,
            "write" => RelayMarker::Write,
            _ => RelayMarker::ReadWrite,
        })
    }
}

/// A relay entry in the relay list
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayEntry {
    /// Relay URL (wss://)
    pub url: String,
    /// Marker indicating read/write capability
    pub marker: RelayMarker,
}

impl RelayEntry {
    /// Create a new relay entry
    pub fn new(url: String, marker: RelayMarker) -> Self {
        Self { url, marker }
    }

    /// Create a read/write relay entry
    pub fn read_write(url: String) -> Self {
        Self {
            url,
            marker: RelayMarker::ReadWrite,
        }
    }

    /// Create a read-only relay entry
    pub fn read(url: String) -> Self {
        Self {
            url,
            marker: RelayMarker::Read,
        }
    }

    /// Create a write-only relay entry
    pub fn write(url: String) -> Self {
        Self {
            url,
            marker: RelayMarker::Write,
        }
    }
}

/// Relay list metadata event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayListMetadata {
    /// List of relay entries
    pub relays: Vec<RelayEntry>,
}

impl RelayListMetadata {
    /// Create a new relay list metadata
    pub fn new(relays: Vec<RelayEntry>) -> Self {
        Self { relays }
    }

    /// Parse from an event
    pub fn from_event(event: &Event) -> Result<Self, Nip65Error> {
        if event.kind != RELAY_LIST_METADATA_KIND {
            return Err(Nip65Error::InvalidKind(event.kind));
        }

        let relays = get_relay_entries(event)?;

        Ok(Self { relays })
    }

    /// Get all read relays
    pub fn read_relays(&self) -> Vec<String> {
        self.relays
            .iter()
            .filter(|r| r.marker.can_read())
            .map(|r| r.url.clone())
            .collect()
    }

    /// Get all write relays
    pub fn write_relays(&self) -> Vec<String> {
        self.relays
            .iter()
            .filter(|r| r.marker.can_write())
            .map(|r| r.url.clone())
            .collect()
    }

    /// Get all relays (regardless of marker)
    pub fn all_relays(&self) -> Vec<String> {
        self.relays.iter().map(|r| r.url.clone()).collect()
    }
}

/// Check if an event is a relay list metadata event
pub fn is_relay_list_metadata_kind(kind: u16) -> bool {
    kind == RELAY_LIST_METADATA_KIND
}

/// Get relay entries from an event
pub fn get_relay_entries(event: &Event) -> Result<Vec<RelayEntry>, Nip65Error> {
    let mut entries = Vec::new();

    for tag in &event.tags {
        if tag.is_empty() || tag[0] != RELAY_TAG {
            continue;
        }

        if tag.len() < 2 {
            return Err(Nip65Error::InvalidTag(
                "relay tag must have at least URL".to_string(),
            ));
        }

        let url = tag[1].clone();
        let marker = if tag.len() >= 3 {
            RelayMarker::from_str(&tag[2]).unwrap_or(RelayMarker::ReadWrite)
        } else {
            RelayMarker::ReadWrite
        };

        entries.push(RelayEntry { url, marker });
    }

    Ok(entries)
}

/// Create a relay tag
#[allow(dead_code)]
pub fn create_relay_tag(url: String, marker: Option<RelayMarker>) -> Vec<String> {
    let mut tag = vec![RELAY_TAG.to_string(), url];

    if let Some(m) = marker
        && let Some(marker_str) = m.to_str()
    {
        tag.push(marker_str.to_string());
    }

    tag
}

/// Add a relay tag to an event's tags
#[allow(dead_code)]
pub fn add_relay_tag(tags: &mut Vec<Vec<String>>, url: String, marker: Option<RelayMarker>) {
    tags.push(create_relay_tag(url, marker));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(kind: u16, tags: Vec<Vec<String>>) -> Event {
        Event {
            id: "test_id".to_string(),
            pubkey: "test_pubkey".to_string(),
            created_at: 1707409439,
            kind,
            tags,
            content: "".to_string(),
            sig: "test_sig".to_string(),
        }
    }

    #[test]
    fn test_is_relay_list_metadata_kind() {
        assert!(is_relay_list_metadata_kind(10002));
        assert!(!is_relay_list_metadata_kind(10001));
        assert!(!is_relay_list_metadata_kind(1));
    }

    #[test]
    fn test_relay_marker_from_str() {
        assert!(matches!(
            RelayMarker::from_str("read"),
            Ok(RelayMarker::Read)
        ));
        assert!(matches!(
            RelayMarker::from_str("write"),
            Ok(RelayMarker::Write)
        ));
        assert!(matches!(
            RelayMarker::from_str(""),
            Ok(RelayMarker::ReadWrite)
        ));
        assert!(matches!(
            RelayMarker::from_str("invalid"),
            Ok(RelayMarker::ReadWrite)
        ));
    }

    #[test]
    fn test_relay_marker_to_str() {
        assert_eq!(RelayMarker::Read.to_str(), Some("read"));
        assert_eq!(RelayMarker::Write.to_str(), Some("write"));
        assert_eq!(RelayMarker::ReadWrite.to_str(), None);
    }

    #[test]
    fn test_relay_marker_can_read() {
        assert!(RelayMarker::Read.can_read());
        assert!(!RelayMarker::Write.can_read());
        assert!(RelayMarker::ReadWrite.can_read());
    }

    #[test]
    fn test_relay_marker_can_write() {
        assert!(!RelayMarker::Read.can_write());
        assert!(RelayMarker::Write.can_write());
        assert!(RelayMarker::ReadWrite.can_write());
    }

    #[test]
    fn test_relay_entry_new() {
        let entry = RelayEntry::new("wss://relay.example.com".to_string(), RelayMarker::Read);
        assert_eq!(entry.url, "wss://relay.example.com");
        assert_eq!(entry.marker, RelayMarker::Read);
    }

    #[test]
    fn test_relay_entry_constructors() {
        let rw = RelayEntry::read_write("wss://rw.com".to_string());
        assert_eq!(rw.marker, RelayMarker::ReadWrite);

        let r = RelayEntry::read("wss://r.com".to_string());
        assert_eq!(r.marker, RelayMarker::Read);

        let w = RelayEntry::write("wss://w.com".to_string());
        assert_eq!(w.marker, RelayMarker::Write);
    }

    #[test]
    fn test_relay_list_metadata_from_event() {
        let event = create_test_event(
            10002,
            vec![
                vec!["r".to_string(), "wss://alicerelay.example.com".to_string()],
                vec!["r".to_string(), "wss://brando-relay.com".to_string()],
                vec![
                    "r".to_string(),
                    "wss://expensive-relay.example2.com".to_string(),
                    "write".to_string(),
                ],
                vec![
                    "r".to_string(),
                    "wss://nostr-relay.example.com".to_string(),
                    "read".to_string(),
                ],
            ],
        );

        let metadata = RelayListMetadata::from_event(&event).unwrap();

        assert_eq!(metadata.relays.len(), 4);
        assert_eq!(metadata.relays[0].url, "wss://alicerelay.example.com");
        assert_eq!(metadata.relays[0].marker, RelayMarker::ReadWrite);
        assert_eq!(metadata.relays[2].url, "wss://expensive-relay.example2.com");
        assert_eq!(metadata.relays[2].marker, RelayMarker::Write);
        assert_eq!(metadata.relays[3].url, "wss://nostr-relay.example.com");
        assert_eq!(metadata.relays[3].marker, RelayMarker::Read);
    }

    #[test]
    fn test_relay_list_metadata_from_event_invalid_kind() {
        let event = create_test_event(1, vec![]);
        let result = RelayListMetadata::from_event(&event);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip65Error::InvalidKind(1)));
    }

    #[test]
    fn test_relay_list_metadata_read_relays() {
        let metadata = RelayListMetadata::new(vec![
            RelayEntry::read_write("wss://rw.com".to_string()),
            RelayEntry::read("wss://r.com".to_string()),
            RelayEntry::write("wss://w.com".to_string()),
        ]);

        let read_relays = metadata.read_relays();
        assert_eq!(read_relays.len(), 2);
        assert!(read_relays.contains(&"wss://rw.com".to_string()));
        assert!(read_relays.contains(&"wss://r.com".to_string()));
    }

    #[test]
    fn test_relay_list_metadata_write_relays() {
        let metadata = RelayListMetadata::new(vec![
            RelayEntry::read_write("wss://rw.com".to_string()),
            RelayEntry::read("wss://r.com".to_string()),
            RelayEntry::write("wss://w.com".to_string()),
        ]);

        let write_relays = metadata.write_relays();
        assert_eq!(write_relays.len(), 2);
        assert!(write_relays.contains(&"wss://rw.com".to_string()));
        assert!(write_relays.contains(&"wss://w.com".to_string()));
    }

    #[test]
    fn test_relay_list_metadata_all_relays() {
        let metadata = RelayListMetadata::new(vec![
            RelayEntry::read_write("wss://rw.com".to_string()),
            RelayEntry::read("wss://r.com".to_string()),
            RelayEntry::write("wss://w.com".to_string()),
        ]);

        let all_relays = metadata.all_relays();
        assert_eq!(all_relays.len(), 3);
    }

    #[test]
    fn test_get_relay_entries() {
        let event = create_test_event(
            10002,
            vec![
                vec!["r".to_string(), "wss://relay1.com".to_string()],
                vec![
                    "r".to_string(),
                    "wss://relay2.com".to_string(),
                    "write".to_string(),
                ],
                vec![
                    "r".to_string(),
                    "wss://relay3.com".to_string(),
                    "read".to_string(),
                ],
            ],
        );

        let entries = get_relay_entries(&event).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].marker, RelayMarker::ReadWrite);
        assert_eq!(entries[1].marker, RelayMarker::Write);
        assert_eq!(entries[2].marker, RelayMarker::Read);
    }

    #[test]
    fn test_get_relay_entries_invalid_tag() {
        let event = create_test_event(10002, vec![vec!["r".to_string()]]);
        let result = get_relay_entries(&event);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip65Error::InvalidTag(_)));
    }

    #[test]
    fn test_create_relay_tag_read_write() {
        let tag = create_relay_tag("wss://relay.com".to_string(), None);
        assert_eq!(tag, vec!["r".to_string(), "wss://relay.com".to_string()]);
    }

    #[test]
    fn test_create_relay_tag_with_marker() {
        let tag = create_relay_tag("wss://relay.com".to_string(), Some(RelayMarker::Write));
        assert_eq!(
            tag,
            vec![
                "r".to_string(),
                "wss://relay.com".to_string(),
                "write".to_string()
            ]
        );
    }

    #[test]
    fn test_create_relay_tag_read_write_explicit() {
        let tag = create_relay_tag("wss://relay.com".to_string(), Some(RelayMarker::ReadWrite));
        // ReadWrite marker is omitted
        assert_eq!(tag, vec!["r".to_string(), "wss://relay.com".to_string()]);
    }

    #[test]
    fn test_add_relay_tag() {
        let mut tags = vec![vec!["p".to_string(), "somepubkey".to_string()]];
        add_relay_tag(
            &mut tags,
            "wss://relay.com".to_string(),
            Some(RelayMarker::Read),
        );

        assert_eq!(tags.len(), 2);
        assert_eq!(
            tags[1],
            vec![
                "r".to_string(),
                "wss://relay.com".to_string(),
                "read".to_string()
            ]
        );
    }

    #[test]
    fn test_example_from_nip() {
        let event = create_test_event(
            10002,
            vec![
                vec!["r".to_string(), "wss://alicerelay.example.com".to_string()],
                vec!["r".to_string(), "wss://brando-relay.com".to_string()],
                vec![
                    "r".to_string(),
                    "wss://expensive-relay.example2.com".to_string(),
                    "write".to_string(),
                ],
                vec![
                    "r".to_string(),
                    "wss://nostr-relay.example.com".to_string(),
                    "read".to_string(),
                ],
            ],
        );

        let metadata = RelayListMetadata::from_event(&event).unwrap();

        // Check write relays
        let write_relays = metadata.write_relays();
        assert_eq!(write_relays.len(), 3); // 2 read-write + 1 write
        assert!(write_relays.contains(&"wss://alicerelay.example.com".to_string()));
        assert!(write_relays.contains(&"wss://brando-relay.com".to_string()));
        assert!(write_relays.contains(&"wss://expensive-relay.example2.com".to_string()));

        // Check read relays
        let read_relays = metadata.read_relays();
        assert_eq!(read_relays.len(), 3); // 2 read-write + 1 read
        assert!(read_relays.contains(&"wss://alicerelay.example.com".to_string()));
        assert!(read_relays.contains(&"wss://brando-relay.com".to_string()));
        assert!(read_relays.contains(&"wss://nostr-relay.example.com".to_string()));
    }
}
