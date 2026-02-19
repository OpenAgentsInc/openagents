//! NIP-03: OpenTimestamps Attestations for Events
//!
//! This module implements NIP-03 which defines kind 1040 events that contain
//! OpenTimestamps proofs for other Nostr events.
//!
//! # Overview
//!
//! OpenTimestamps provides cryptographic proof that data existed at a specific point in time
//! by anchoring it to the Bitcoin blockchain. NIP-03 allows Nostr events to have timestamped
//! attestations that can be independently verified.
//!
//! # Event Structure
//!
//! A kind 1040 event contains:
//! - `content`: Base64-encoded .ots file data
//! - `e` tag: References the target event ID
//! - `k` tag: Specifies the target event kind
//!
//! # Example
//!
//! ```
//! use nostr::nip03::{OpenTimestampsAttestation, create_attestation_tags, KIND_OTS_ATTESTATION};
//! use nostr::{EventTemplate, finalize_event, generate_secret_key};
//!
//! // Create an attestation for an event
//! let target_event_id = "abcd1234...";
//! let target_kind = 1;
//! let ots_data = b"OTS file data here";
//! let ots_base64 = base64::encode(ots_data);
//!
//! let tags = create_attestation_tags(
//!     target_event_id,
//!     target_kind,
//!     Some("wss://relay.example.com")
//! );
//!
//! let template = EventTemplate {
//!     kind: KIND_OTS_ATTESTATION,
//!     tags,
//!     content: ots_base64,
//!     created_at: 1234567890,
//! };
//!
//! # let sk = generate_secret_key();
//! # let event = finalize_event(&template, &sk).unwrap();
//! ```

#[cfg(feature = "full")]
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use thiserror::Error;

/// Event kind for OpenTimestamps attestations
pub const KIND_OTS_ATTESTATION: u16 = 1040;

/// Tag name for target event kind
pub const TARGET_KIND_TAG: &str = "k";

/// Tag name for target event ID
pub const TARGET_EVENT_TAG: &str = "e";

/// NIP-03 error types
#[derive(Debug, Error, Clone, PartialEq)]
pub enum Nip03Error {
    /// Missing required 'e' tag
    #[error("missing required 'e' tag (target event ID)")]
    MissingEventTag,

    /// Missing required 'k' tag
    #[error("missing required 'k' tag (target event kind)")]
    MissingKindTag,

    /// Invalid 'e' tag format
    #[error("invalid 'e' tag: {0}")]
    InvalidEventTag(String),

    /// Invalid 'k' tag format
    #[error("invalid 'k' tag: {0}")]
    InvalidKindTag(String),

    /// Invalid base64 content
    #[error("invalid base64 content: {0}")]
    InvalidBase64(String),

    /// Empty content
    #[error("content is empty")]
    EmptyContent,

    /// Wrong event kind
    #[error("expected kind 1040, got {0}")]
    WrongKind(u16),
}

/// Represents an OpenTimestamps attestation event
#[derive(Debug, Clone, PartialEq)]
pub struct OpenTimestampsAttestation {
    /// The target event ID being attested
    pub target_event_id: String,
    /// The kind of the target event
    pub target_event_kind: u16,
    /// The relay URL where the target event can be found (optional)
    pub relay_url: Option<String>,
    /// The raw .ots file data (decoded from base64)
    pub ots_data: Vec<u8>,
}

/// Create tags for an OpenTimestamps attestation event
///
/// # Example
///
/// ```
/// use nostr::nip03::create_attestation_tags;
///
/// let tags = create_attestation_tags(
///     "event123",
///     1,
///     Some("wss://relay.example.com")
/// );
///
/// assert_eq!(tags.len(), 2);
/// assert_eq!(tags[0][0], "e");
/// assert_eq!(tags[1][0], "k");
/// ```
pub fn create_attestation_tags(
    target_event_id: &str,
    target_event_kind: u16,
    relay_url: Option<&str>,
) -> Vec<Vec<String>> {
    let mut tags = Vec::new();

    // Add 'e' tag with event ID and optional relay URL
    let mut e_tag = vec![TARGET_EVENT_TAG.to_string(), target_event_id.to_string()];
    if let Some(url) = relay_url {
        e_tag.push(url.to_string());
    }
    tags.push(e_tag);

    // Add 'k' tag with event kind
    tags.push(vec![
        TARGET_KIND_TAG.to_string(),
        target_event_kind.to_string(),
    ]);

    tags
}

/// Extract the target event ID from tags
///
/// # Example
///
/// ```
/// use nostr::nip03::get_target_event_id;
///
/// let tags = vec![
///     vec!["e".to_string(), "event123".to_string()],
///     vec!["k".to_string(), "1".to_string()],
/// ];
///
/// assert_eq!(get_target_event_id(&tags).unwrap(), "event123");
/// ```
pub fn get_target_event_id(tags: &[Vec<String>]) -> Result<String, Nip03Error> {
    tags.iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(TARGET_EVENT_TAG))
        .and_then(|tag| tag.get(1).cloned())
        .ok_or(Nip03Error::MissingEventTag)
}

/// Extract the target event kind from tags
///
/// # Example
///
/// ```
/// use nostr::nip03::get_target_event_kind;
///
/// let tags = vec![
///     vec!["e".to_string(), "event123".to_string()],
///     vec!["k".to_string(), "1".to_string()],
/// ];
///
/// assert_eq!(get_target_event_kind(&tags).unwrap(), 1);
/// ```
pub fn get_target_event_kind(tags: &[Vec<String>]) -> Result<u16, Nip03Error> {
    let kind_str = tags
        .iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(TARGET_KIND_TAG))
        .and_then(|tag| tag.get(1))
        .ok_or(Nip03Error::MissingKindTag)?;

    kind_str
        .parse::<u16>()
        .map_err(|e| Nip03Error::InvalidKindTag(e.to_string()))
}

/// Extract the relay URL from 'e' tag (if present)
///
/// # Example
///
/// ```
/// use nostr::nip03::get_target_relay_url;
///
/// let tags = vec![
///     vec!["e".to_string(), "event123".to_string(), "wss://relay.example.com".to_string()],
///     vec!["k".to_string(), "1".to_string()],
/// ];
///
/// assert_eq!(get_target_relay_url(&tags).unwrap(), Some("wss://relay.example.com".to_string()));
/// ```
pub fn get_target_relay_url(tags: &[Vec<String>]) -> Result<Option<String>, Nip03Error> {
    Ok(tags
        .iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(TARGET_EVENT_TAG))
        .and_then(|tag| tag.get(2).cloned()))
}

/// Decode base64-encoded OTS data from content
///
/// # Example
///
/// ```
/// use nostr::nip03::decode_ots_content;
///
/// let ots_data = b"OTS file data";
/// let base64 = base64::encode(ots_data);
///
/// let decoded = decode_ots_content(&base64).unwrap();
/// assert_eq!(decoded, ots_data);
/// ```
#[cfg(feature = "full")]
pub fn decode_ots_content(content: &str) -> Result<Vec<u8>, Nip03Error> {
    if content.is_empty() {
        return Err(Nip03Error::EmptyContent);
    }

    BASE64
        .decode(content)
        .map_err(|e| Nip03Error::InvalidBase64(e.to_string()))
}

/// Encode OTS data as base64 for event content
///
/// # Example
///
/// ```
/// use nostr::nip03::encode_ots_content;
///
/// let ots_data = b"OTS file data";
/// let base64 = encode_ots_content(ots_data);
///
/// assert!(base64.len() > 0);
/// ```
#[cfg(feature = "full")]
pub fn encode_ots_content(ots_data: &[u8]) -> String {
    BASE64.encode(ots_data)
}

/// Parse an OpenTimestamps attestation event
///
/// # Example
///
/// ```
/// use nostr::nip03::{parse_attestation, KIND_OTS_ATTESTATION};
/// use nostr::{EventTemplate, finalize_event, generate_secret_key};
///
/// let tags = vec![
///     vec!["e".to_string(), "event123".to_string()],
///     vec!["k".to_string(), "1".to_string()],
/// ];
///
/// let ots_data = b"OTS file data";
/// let content = base64::encode(ots_data);
///
/// let template = EventTemplate {
///     kind: KIND_OTS_ATTESTATION,
///     tags,
///     content,
///     created_at: 1234567890,
/// };
///
/// # let sk = generate_secret_key();
/// # let event = finalize_event(&template, &sk).unwrap();
/// let attestation = parse_attestation(event.kind, &event.tags, &event.content).unwrap();
/// assert_eq!(attestation.target_event_id, "event123");
/// assert_eq!(attestation.target_event_kind, 1);
/// ```
#[cfg(feature = "full")]
pub fn parse_attestation(
    kind: u16,
    tags: &[Vec<String>],
    content: &str,
) -> Result<OpenTimestampsAttestation, Nip03Error> {
    // Validate kind
    if kind != KIND_OTS_ATTESTATION {
        return Err(Nip03Error::WrongKind(kind));
    }

    // Extract required fields
    let target_event_id = get_target_event_id(tags)?;
    let target_event_kind = get_target_event_kind(tags)?;
    let relay_url = get_target_relay_url(tags)?;

    // Decode OTS data
    let ots_data = decode_ots_content(content)?;

    Ok(OpenTimestampsAttestation {
        target_event_id,
        target_event_kind,
        relay_url,
        ots_data,
    })
}

/// Check if an event is an OpenTimestamps attestation
///
/// # Example
///
/// ```
/// use nostr::nip03::{is_ots_attestation, KIND_OTS_ATTESTATION};
///
/// assert!(is_ots_attestation(KIND_OTS_ATTESTATION));
/// assert!(!is_ots_attestation(1));
/// ```
pub fn is_ots_attestation(kind: u16) -> bool {
    kind == KIND_OTS_ATTESTATION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_attestation_tags() {
        let tags = create_attestation_tags("event123", 1, Some("wss://relay.example.com"));

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0][0], "e");
        assert_eq!(tags[0][1], "event123");
        assert_eq!(tags[0][2], "wss://relay.example.com");
        assert_eq!(tags[1][0], "k");
        assert_eq!(tags[1][1], "1");
    }

    #[test]
    fn test_create_attestation_tags_without_relay() {
        let tags = create_attestation_tags("event123", 1, None);

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].len(), 2); // No relay URL
        assert_eq!(tags[0][0], "e");
        assert_eq!(tags[0][1], "event123");
        assert_eq!(tags[1][0], "k");
        assert_eq!(tags[1][1], "1");
    }

    #[test]
    fn test_get_target_event_id() {
        let tags = vec![
            vec!["e".to_string(), "event123".to_string()],
            vec!["k".to_string(), "1".to_string()],
        ];

        assert_eq!(get_target_event_id(&tags).unwrap(), "event123");
    }

    #[test]
    fn test_get_target_event_id_missing() {
        let tags = vec![vec!["k".to_string(), "1".to_string()]];

        assert!(matches!(
            get_target_event_id(&tags),
            Err(Nip03Error::MissingEventTag)
        ));
    }

    #[test]
    fn test_get_target_event_kind() {
        let tags = vec![
            vec!["e".to_string(), "event123".to_string()],
            vec!["k".to_string(), "1".to_string()],
        ];

        assert_eq!(get_target_event_kind(&tags).unwrap(), 1);
    }

    #[test]
    fn test_get_target_event_kind_missing() {
        let tags = vec![vec!["e".to_string(), "event123".to_string()]];

        assert!(matches!(
            get_target_event_kind(&tags),
            Err(Nip03Error::MissingKindTag)
        ));
    }

    #[test]
    fn test_get_target_event_kind_invalid() {
        let tags = vec![vec!["k".to_string(), "invalid".to_string()]];

        assert!(matches!(
            get_target_event_kind(&tags),
            Err(Nip03Error::InvalidKindTag(_))
        ));
    }

    #[test]
    fn test_get_target_relay_url() {
        let tags = vec![vec![
            "e".to_string(),
            "event123".to_string(),
            "wss://relay.example.com".to_string(),
        ]];

        assert_eq!(
            get_target_relay_url(&tags).unwrap(),
            Some("wss://relay.example.com".to_string())
        );
    }

    #[test]
    fn test_get_target_relay_url_missing() {
        let tags = vec![vec!["e".to_string(), "event123".to_string()]];

        assert_eq!(get_target_relay_url(&tags).unwrap(), None);
    }

    #[test]
    fn test_decode_ots_content() {
        let ots_data = b"OTS file data";
        let base64 = BASE64.encode(ots_data);

        let decoded = decode_ots_content(&base64).unwrap();
        assert_eq!(decoded, ots_data);
    }

    #[test]
    fn test_decode_ots_content_invalid() {
        assert!(matches!(
            decode_ots_content("not valid base64!!!"),
            Err(Nip03Error::InvalidBase64(_))
        ));
    }

    #[test]
    fn test_decode_ots_content_empty() {
        assert!(matches!(
            decode_ots_content(""),
            Err(Nip03Error::EmptyContent)
        ));
    }

    #[test]
    fn test_encode_ots_content() {
        let ots_data = b"OTS file data";
        let base64 = encode_ots_content(ots_data);

        assert!(!base64.is_empty());
        assert_eq!(BASE64.decode(&base64).unwrap(), ots_data);
    }

    #[test]
    fn test_parse_attestation() {
        let tags = vec![
            vec!["e".to_string(), "event123".to_string()],
            vec!["k".to_string(), "1".to_string()],
        ];

        let ots_data = b"OTS file data";
        let content = BASE64.encode(ots_data);

        let attestation = parse_attestation(KIND_OTS_ATTESTATION, &tags, &content).unwrap();

        assert_eq!(attestation.target_event_id, "event123");
        assert_eq!(attestation.target_event_kind, 1);
        assert_eq!(attestation.relay_url, None);
        assert_eq!(attestation.ots_data, ots_data);
    }

    #[test]
    fn test_parse_attestation_with_relay() {
        let tags = vec![
            vec![
                "e".to_string(),
                "event123".to_string(),
                "wss://relay.example.com".to_string(),
            ],
            vec!["k".to_string(), "1".to_string()],
        ];

        let ots_data = b"OTS file data";
        let content = BASE64.encode(ots_data);

        let attestation = parse_attestation(KIND_OTS_ATTESTATION, &tags, &content).unwrap();

        assert_eq!(attestation.target_event_id, "event123");
        assert_eq!(attestation.target_event_kind, 1);
        assert_eq!(
            attestation.relay_url,
            Some("wss://relay.example.com".to_string())
        );
        assert_eq!(attestation.ots_data, ots_data);
    }

    #[test]
    fn test_parse_attestation_wrong_kind() {
        let tags = vec![
            vec!["e".to_string(), "event123".to_string()],
            vec!["k".to_string(), "1".to_string()],
        ];

        let content = BASE64.encode(b"OTS file data");

        assert!(matches!(
            parse_attestation(1, &tags, &content),
            Err(Nip03Error::WrongKind(1))
        ));
    }

    #[test]
    fn test_is_ots_attestation() {
        assert!(is_ots_attestation(KIND_OTS_ATTESTATION));
        assert!(is_ots_attestation(1040));
        assert!(!is_ots_attestation(1));
        assert!(!is_ots_attestation(0));
    }
}
