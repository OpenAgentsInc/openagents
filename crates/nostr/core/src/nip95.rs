//! NIP-95: File Storage on Relays
//!
//! Defines a mechanism for storing binary files directly on Nostr relays using
//! base64-encoded content in events. Files are split into content events (kind 1064)
//! and metadata/header events (kind 1065) that reference them.
//!
//! Note: This NIP is deprecated in favor of NIP-96 (HTTP File Storage), but remains
//! useful for understanding relay-based file storage.
//!
//! See: <https://github.com/nostr-protocol/nips/pull/345>

use crate::Event;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Event kind for file content (base64-encoded binary data)
pub const FILE_CONTENT_KIND: u16 = 1064;

/// Event kind for file header/metadata
pub const FILE_HEADER_KIND: u16 = 1065;

/// Errors that can occur during NIP-95 operations
#[derive(Debug, Error)]
pub enum Nip95Error {
    #[error("event is not a file content event (kind {0})")]
    InvalidContentKind(u16),

    #[error("event is not a file header event (kind {0})")]
    InvalidHeaderKind(u16),

    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid tag format: {0}")]
    InvalidTag(String),

    #[error("base64 decode error: {0}")]
    Base64Error(String),

    #[error("invalid hash: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
}

/// File content event (kind 1064)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileContent {
    /// Base64-encoded binary content
    pub content_base64: String,
    /// Event ID of this content event
    pub event_id: String,
}

/// File header/metadata event (kind 1065)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileHeader {
    /// MIME type
    pub mime_type: String,
    /// SHA-256 hash of the file content
    pub hash: String,
    /// File size in bytes
    pub size: u64,
    /// Short identifier or filename
    pub summary: Option<String>,
    /// Accessibility description
    pub alt: Option<String>,
    /// References to content events (kind 1064)
    pub content_events: Vec<String>,
    /// Block size for multi-part files
    pub block_size: Option<u64>,
}

impl FileContent {
    /// Create a new file content event
    pub fn new(content_base64: String, event_id: String) -> Self {
        Self {
            content_base64,
            event_id,
        }
    }

    /// Parse from an event
    pub fn from_event(event: &Event) -> Result<Self, Nip95Error> {
        if event.kind != FILE_CONTENT_KIND {
            return Err(Nip95Error::InvalidContentKind(event.kind));
        }

        Ok(Self {
            content_base64: event.content.clone(),
            event_id: event.id.clone(),
        })
    }

    /// Decode base64 content to bytes
    #[cfg(feature = "full")]
    pub fn decode(&self) -> Result<Vec<u8>, Nip95Error> {
        use base64::{Engine, engine::general_purpose::STANDARD};
        STANDARD
            .decode(&self.content_base64)
            .map_err(|e| Nip95Error::Base64Error(e.to_string()))
    }
}

impl FileHeader {
    /// Create a new file header
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mime_type: String,
        hash: String,
        size: u64,
        summary: Option<String>,
        alt: Option<String>,
        content_events: Vec<String>,
        block_size: Option<u64>,
    ) -> Self {
        Self {
            mime_type,
            hash,
            size,
            summary,
            alt,
            content_events,
            block_size,
        }
    }

    /// Parse from an event
    pub fn from_event(event: &Event) -> Result<Self, Nip95Error> {
        if event.kind != FILE_HEADER_KIND {
            return Err(Nip95Error::InvalidHeaderKind(event.kind));
        }

        let mime_type = get_mime_type(event)?;
        let hash = get_hash(event)?;
        let size = get_size(event)?;
        let summary = get_summary(event);
        let alt = get_alt(event);
        let content_events = get_content_events(event);
        let block_size = get_block_size(event);

        Ok(Self {
            mime_type,
            hash,
            size,
            summary,
            alt,
            content_events,
            block_size,
        })
    }

    /// Check if this is a multi-part file
    pub fn is_multipart(&self) -> bool {
        self.content_events.len() > 1
    }
}

/// Check if an event is a file content event
pub fn is_file_content_kind(kind: u16) -> bool {
    kind == FILE_CONTENT_KIND
}

/// Check if an event is a file header event
pub fn is_file_header_kind(kind: u16) -> bool {
    kind == FILE_HEADER_KIND
}

/// Check if an event is a NIP-95 event
pub fn is_nip95_kind(kind: u16) -> bool {
    kind == FILE_CONTENT_KIND || kind == FILE_HEADER_KIND
}

/// Get MIME type from file header event
pub fn get_mime_type(event: &Event) -> Result<String, Nip95Error> {
    event
        .tags
        .iter()
        .find(|tag| tag.len() >= 2 && tag[0] == "m")
        .map(|tag| tag[1].clone())
        .ok_or_else(|| Nip95Error::MissingField("mime type (m tag)".to_string()))
}

/// Get SHA-256 hash from file header event
pub fn get_hash(event: &Event) -> Result<String, Nip95Error> {
    event
        .tags
        .iter()
        .find(|tag| tag.len() >= 2 && tag[0] == "x")
        .map(|tag| tag[1].clone())
        .ok_or_else(|| Nip95Error::MissingField("hash (x tag)".to_string()))
}

/// Get file size from file header event
pub fn get_size(event: &Event) -> Result<u64, Nip95Error> {
    event
        .tags
        .iter()
        .find(|tag| tag.len() >= 2 && tag[0] == "size")
        .and_then(|tag| tag[1].parse::<u64>().ok())
        .ok_or_else(|| Nip95Error::MissingField("size tag".to_string()))
}

/// Get summary from file header event
pub fn get_summary(event: &Event) -> Option<String> {
    event
        .tags
        .iter()
        .find(|tag| tag.len() >= 2 && tag[0] == "summary")
        .map(|tag| tag[1].clone())
}

/// Get alt text from file header event
pub fn get_alt(event: &Event) -> Option<String> {
    event
        .tags
        .iter()
        .find(|tag| tag.len() >= 2 && tag[0] == "alt")
        .map(|tag| tag[1].clone())
}

/// Get content event references from file header event
pub fn get_content_events(event: &Event) -> Vec<String> {
    event
        .tags
        .iter()
        .filter(|tag| tag.len() >= 2 && tag[0] == "e")
        .map(|tag| tag[1].clone())
        .collect()
}

/// Get block size from file header event
pub fn get_block_size(event: &Event) -> Option<u64> {
    event
        .tags
        .iter()
        .find(|tag| tag.len() >= 2 && tag[0] == "block_size")
        .and_then(|tag| tag[1].parse::<u64>().ok())
}

/// Create MIME type tag
#[allow(dead_code)]
pub fn create_mime_type_tag(mime_type: String) -> Vec<String> {
    vec!["m".to_string(), mime_type]
}

/// Create hash tag
#[allow(dead_code)]
pub fn create_hash_tag(hash: String) -> Vec<String> {
    vec!["x".to_string(), hash]
}

/// Create size tag
#[allow(dead_code)]
pub fn create_size_tag(size: u64) -> Vec<String> {
    vec!["size".to_string(), size.to_string()]
}

/// Create summary tag
#[allow(dead_code)]
pub fn create_summary_tag(summary: String) -> Vec<String> {
    vec!["summary".to_string(), summary]
}

/// Create alt tag
#[allow(dead_code)]
pub fn create_alt_tag(alt: String) -> Vec<String> {
    vec!["alt".to_string(), alt]
}

/// Create content event reference tag
#[allow(dead_code)]
pub fn create_content_event_tag(event_id: String) -> Vec<String> {
    vec!["e".to_string(), event_id]
}

/// Create block size tag
#[allow(dead_code)]
pub fn create_block_size_tag(block_size: u64) -> Vec<String> {
    vec!["block_size".to_string(), block_size.to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(kind: u16, content: &str, tags: Vec<Vec<String>>) -> Event {
        Event {
            id: "test_id".to_string(),
            pubkey: "test_pubkey".to_string(),
            created_at: 1707409439,
            kind,
            tags,
            content: content.to_string(),
            sig: "test_sig".to_string(),
        }
    }

    #[test]
    fn test_is_file_content_kind() {
        assert!(is_file_content_kind(1064));
        assert!(!is_file_content_kind(1065));
        assert!(!is_file_content_kind(1));
    }

    #[test]
    fn test_is_file_header_kind() {
        assert!(is_file_header_kind(1065));
        assert!(!is_file_header_kind(1064));
        assert!(!is_file_header_kind(1));
    }

    #[test]
    fn test_is_nip95_kind() {
        assert!(is_nip95_kind(1064));
        assert!(is_nip95_kind(1065));
        assert!(!is_nip95_kind(1));
    }

    #[test]
    fn test_file_content_new() {
        let content = FileContent::new("aGVsbG8=".to_string(), "event123".to_string());
        assert_eq!(content.content_base64, "aGVsbG8=");
        assert_eq!(content.event_id, "event123");
    }

    #[test]
    fn test_file_content_from_event() {
        let event = create_test_event(1064, "aGVsbG8=", vec![]);
        let content = FileContent::from_event(&event).unwrap();

        assert_eq!(content.content_base64, "aGVsbG8=");
        assert_eq!(content.event_id, "test_id");
    }

    #[test]
    fn test_file_content_from_event_invalid_kind() {
        let event = create_test_event(1, "test", vec![]);
        let result = FileContent::from_event(&event);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip95Error::InvalidContentKind(1)
        ));
    }

    #[test]
    #[cfg(feature = "full")]
    fn test_file_content_decode() {
        let content = FileContent::new("aGVsbG8=".to_string(), "event123".to_string());
        let decoded = content.decode().unwrap();

        assert_eq!(decoded, b"hello");
    }

    #[test]
    fn test_file_header_from_event() {
        let event = create_test_event(
            1065,
            "",
            vec![
                vec!["m".to_string(), "image/jpeg".to_string()],
                vec!["x".to_string(), "abc123hash".to_string()],
                vec!["size".to_string(), "1024".to_string()],
                vec!["summary".to_string(), "photo.jpg".to_string()],
                vec!["alt".to_string(), "A beautiful photo".to_string()],
                vec!["e".to_string(), "content_event_1".to_string()],
                vec!["block_size".to_string(), "512".to_string()],
            ],
        );

        let header = FileHeader::from_event(&event).unwrap();

        assert_eq!(header.mime_type, "image/jpeg");
        assert_eq!(header.hash, "abc123hash");
        assert_eq!(header.size, 1024);
        assert_eq!(header.summary, Some("photo.jpg".to_string()));
        assert_eq!(header.alt, Some("A beautiful photo".to_string()));
        assert_eq!(header.content_events, vec!["content_event_1".to_string()]);
        assert_eq!(header.block_size, Some(512));
    }

    #[test]
    fn test_file_header_from_event_minimal() {
        let event = create_test_event(
            1065,
            "",
            vec![
                vec!["m".to_string(), "text/plain".to_string()],
                vec!["x".to_string(), "hash456".to_string()],
                vec!["size".to_string(), "100".to_string()],
            ],
        );

        let header = FileHeader::from_event(&event).unwrap();

        assert_eq!(header.mime_type, "text/plain");
        assert_eq!(header.hash, "hash456");
        assert_eq!(header.size, 100);
        assert!(header.summary.is_none());
        assert!(header.alt.is_none());
        assert!(header.content_events.is_empty());
        assert!(header.block_size.is_none());
    }

    #[test]
    fn test_file_header_from_event_invalid_kind() {
        let event = create_test_event(1, "", vec![]);
        let result = FileHeader::from_event(&event);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip95Error::InvalidHeaderKind(1)
        ));
    }

    #[test]
    fn test_file_header_from_event_missing_mime_type() {
        let event = create_test_event(
            1065,
            "",
            vec![
                vec!["x".to_string(), "hash".to_string()],
                vec!["size".to_string(), "100".to_string()],
            ],
        );

        let result = FileHeader::from_event(&event);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip95Error::MissingField(_)));
    }

    #[test]
    fn test_file_header_from_event_missing_hash() {
        let event = create_test_event(
            1065,
            "",
            vec![
                vec!["m".to_string(), "text/plain".to_string()],
                vec!["size".to_string(), "100".to_string()],
            ],
        );

        let result = FileHeader::from_event(&event);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip95Error::MissingField(_)));
    }

    #[test]
    fn test_file_header_from_event_missing_size() {
        let event = create_test_event(
            1065,
            "",
            vec![
                vec!["m".to_string(), "text/plain".to_string()],
                vec!["x".to_string(), "hash".to_string()],
            ],
        );

        let result = FileHeader::from_event(&event);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip95Error::MissingField(_)));
    }

    #[test]
    fn test_file_header_is_multipart() {
        let header_single = FileHeader::new(
            "image/jpeg".to_string(),
            "hash".to_string(),
            1024,
            None,
            None,
            vec!["event1".to_string()],
            None,
        );
        assert!(!header_single.is_multipart());

        let header_multi = FileHeader::new(
            "image/jpeg".to_string(),
            "hash".to_string(),
            2048,
            None,
            None,
            vec!["event1".to_string(), "event2".to_string()],
            Some(1024),
        );
        assert!(header_multi.is_multipart());
    }

    #[test]
    fn test_get_mime_type() {
        let event = create_test_event(
            1065,
            "",
            vec![vec!["m".to_string(), "video/mp4".to_string()]],
        );
        assert_eq!(get_mime_type(&event).unwrap(), "video/mp4");
    }

    #[test]
    fn test_get_hash() {
        let event = create_test_event(
            1065,
            "",
            vec![vec!["x".to_string(), "sha256hash".to_string()]],
        );
        assert_eq!(get_hash(&event).unwrap(), "sha256hash");
    }

    #[test]
    fn test_get_size() {
        let event = create_test_event(1065, "", vec![vec!["size".to_string(), "4096".to_string()]]);
        assert_eq!(get_size(&event).unwrap(), 4096);
    }

    #[test]
    fn test_get_summary() {
        let event = create_test_event(
            1065,
            "",
            vec![vec!["summary".to_string(), "myfile.pdf".to_string()]],
        );
        assert_eq!(get_summary(&event), Some("myfile.pdf".to_string()));
    }

    #[test]
    fn test_get_alt() {
        let event = create_test_event(
            1065,
            "",
            vec![vec!["alt".to_string(), "Description".to_string()]],
        );
        assert_eq!(get_alt(&event), Some("Description".to_string()));
    }

    #[test]
    fn test_get_content_events() {
        let event = create_test_event(
            1065,
            "",
            vec![
                vec!["e".to_string(), "event1".to_string()],
                vec!["e".to_string(), "event2".to_string()],
                vec!["e".to_string(), "event3".to_string()],
            ],
        );
        let events = get_content_events(&event);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0], "event1");
        assert_eq!(events[1], "event2");
        assert_eq!(events[2], "event3");
    }

    #[test]
    fn test_get_block_size() {
        let event = create_test_event(
            1065,
            "",
            vec![vec!["block_size".to_string(), "1024".to_string()]],
        );
        assert_eq!(get_block_size(&event), Some(1024));
    }

    #[test]
    fn test_create_tags() {
        assert_eq!(
            create_mime_type_tag("image/png".to_string()),
            vec!["m".to_string(), "image/png".to_string()]
        );

        assert_eq!(
            create_hash_tag("abc123".to_string()),
            vec!["x".to_string(), "abc123".to_string()]
        );

        assert_eq!(
            create_size_tag(2048),
            vec!["size".to_string(), "2048".to_string()]
        );

        assert_eq!(
            create_summary_tag("file.txt".to_string()),
            vec!["summary".to_string(), "file.txt".to_string()]
        );

        assert_eq!(
            create_alt_tag("Alt text".to_string()),
            vec!["alt".to_string(), "Alt text".to_string()]
        );

        assert_eq!(
            create_content_event_tag("event123".to_string()),
            vec!["e".to_string(), "event123".to_string()]
        );

        assert_eq!(
            create_block_size_tag(512),
            vec!["block_size".to_string(), "512".to_string()]
        );
    }
}
