//! NIP-01: Basic protocol flow description.
//!
//! This module implements the core Nostr event structure and operations:
//! - Event structure (id, pubkey, created_at, kind, tags, content, sig)
//! - Event serialization for hashing
//! - Event signing with Schnorr signatures (requires `full` feature)
//! - Event verification (requires `full` feature)
//! - Kind classification (regular, replaceable, ephemeral, addressable)

#[cfg(feature = "full")]
use bitcoin::hashes::{Hash, sha256};
#[cfg(feature = "full")]
use bitcoin::key::Secp256k1;
#[cfg(feature = "full")]
use bitcoin::secp256k1::{Message, SecretKey, XOnlyPublicKey, schnorr};
#[cfg(feature = "full")]
use rand::RngCore;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during NIP-01 operations.
#[derive(Debug, Error)]
pub enum Nip01Error {
    #[error("invalid event: {0}")]
    InvalidEvent(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("signing error: {0}")]
    Signing(String),

    #[error("verification error: {0}")]
    Verification(String),

    #[error("invalid hex: {0}")]
    InvalidHex(String),

    #[error("invalid public key: {0}")]
    InvalidPublicKey(String),

    #[error("invalid signature: {0}")]
    InvalidSignature(String),
}

/// A signed Nostr event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// 32-bytes lowercase hex-encoded sha256 of the serialized event data
    pub id: String,
    /// 32-bytes lowercase hex-encoded public key of the event creator
    pub pubkey: String,
    /// Unix timestamp in seconds
    pub created_at: u64,
    /// Event kind (integer between 0 and 65535)
    pub kind: u16,
    /// Array of arrays of strings (tags)
    pub tags: Vec<Vec<String>>,
    /// Arbitrary string content
    pub content: String,
    /// 64-bytes lowercase hex signature
    pub sig: String,
}

/// An unsigned event (before signing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsignedEvent {
    /// 32-bytes lowercase hex-encoded public key of the event creator
    pub pubkey: String,
    /// Unix timestamp in seconds
    pub created_at: u64,
    /// Event kind
    pub kind: u16,
    /// Array of arrays of strings (tags)
    pub tags: Vec<Vec<String>>,
    /// Arbitrary string content
    pub content: String,
}

/// A template for creating events (without pubkey, which comes from the signing key).
///
/// Event templates are used to create events before signing. The pubkey is derived
/// from the secret key during signing, so templates don't include it.
///
/// # Examples
///
/// Creating a text note:
///
/// ```
/// use nostr::nip01::EventTemplate;
/// use std::time::{SystemTime, UNIX_EPOCH};
///
/// let template = EventTemplate {
///     created_at: SystemTime::now()
///         .duration_since(UNIX_EPOCH)
///         .unwrap()
///         .as_secs(),
///     kind: 1,  // Short text note
///     tags: vec![],
///     content: "Hello Nostr!".to_string(),
///     };
///
/// // Sign with secret key (requires `full` feature)
/// // let event = sign_event(template, &secret_key)?;
/// ```
///
/// Creating an event with tags:
///
/// ```
/// use nostr::nip01::EventTemplate;
/// use std::time::{SystemTime, UNIX_EPOCH};
///
/// let template = EventTemplate {
///     created_at: SystemTime::now()
///         .duration_since(UNIX_EPOCH)
///         .unwrap()
///         .as_secs(),
///     kind: 1,
///     tags: vec![
///         vec!["e".to_string(), "event_id_to_reply_to".to_string()],
///         vec!["p".to_string(), "pubkey_to_mention".to_string()],
///     ],
///     content: "This is a reply with a mention".to_string(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventTemplate {
    /// Unix timestamp in seconds
    pub created_at: u64,
    /// Event kind
    pub kind: u16,
    /// Array of arrays of strings (tags)
    pub tags: Vec<Vec<String>>,
    /// Arbitrary string content
    pub content: String,
}

/// Event kind classification according to NIP-01.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindClassification {
    /// Events expected to be stored by relays
    Regular,
    /// Only latest event per pubkey+kind is stored
    Replaceable,
    /// Not expected to be stored by relays
    Ephemeral,
    /// Only latest event per pubkey+kind+d-tag is stored
    Addressable,
    /// Unknown classification
    Unknown,
}

// Standard event kinds
pub const KIND_METADATA: u16 = 0;
pub const KIND_SHORT_TEXT_NOTE: u16 = 1;
pub const KIND_RECOMMEND_RELAY: u16 = 2;
pub const KIND_CONTACTS: u16 = 3;

/// Generate a random 32-byte secret key.
#[cfg(feature = "full")]
pub fn generate_secret_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    rand::rng().fill_bytes(&mut key);
    key
}

/// Get the public key (x-only, 32 bytes) from a secret key.
#[cfg(feature = "full")]
pub fn get_public_key(secret_key: &[u8; 32]) -> Result<[u8; 32], Nip01Error> {
    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(secret_key)
        .map_err(|e| Nip01Error::InvalidPublicKey(e.to_string()))?;
    let (xonly, _parity) = sk.x_only_public_key(&secp);
    Ok(xonly.serialize())
}

/// Get the public key as a hex string from a secret key.
#[cfg(feature = "full")]
pub fn get_public_key_hex(secret_key: &[u8; 32]) -> Result<String, Nip01Error> {
    Ok(hex::encode(get_public_key(secret_key)?))
}

/// Serialize an unsigned event for hashing.
///
/// Format: `[0, pubkey, created_at, kind, tags, content]`
pub fn serialize_event(event: &UnsignedEvent) -> Result<String, Nip01Error> {
    if !validate_unsigned_event(event) {
        return Err(Nip01Error::InvalidEvent(
            "can't serialize event with wrong or missing properties".to_string(),
        ));
    }

    // Build the serialization array: [0, pubkey, created_at, kind, tags, content]
    let serialized = serde_json::to_string(&(
        0,
        &event.pubkey,
        event.created_at,
        event.kind,
        &event.tags,
        &event.content,
    ))
    .map_err(|e| Nip01Error::Serialization(e.to_string()))?;

    Ok(serialized)
}

/// Get the event hash (id) from an unsigned event.
#[cfg(feature = "full")]
pub fn get_event_hash(event: &UnsignedEvent) -> Result<String, Nip01Error> {
    let serialized = serialize_event(event)?;
    let hash = sha256::Hash::hash(serialized.as_bytes());
    Ok(hex::encode(hash.as_byte_array()))
}

/// Validate an unsigned event structure.
pub fn validate_unsigned_event(event: &UnsignedEvent) -> bool {
    // Check pubkey is 64 hex characters
    if event.pubkey.len() != 64 {
        return false;
    }
    if !event.pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }
    // Pubkey should be lowercase
    if event.pubkey != event.pubkey.to_lowercase() {
        return false;
    }

    // Check tags are valid (arrays of strings)
    for tag in &event.tags {
        if tag.is_empty() {
            // Tags should have at least one element (the tag name)
            // However, nostr-tools allows empty tags, so we'll be permissive
        }
    }

    true
}

/// Validate a signed event structure (not including signature verification).
#[cfg(feature = "full")]
pub fn validate_event(event: &Event) -> bool {
    // Check id is 64 hex characters
    if event.id.len() != 64 || !event.id.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }

    // Check pubkey is 64 lowercase hex characters
    if event.pubkey.len() != 64 || !event.pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }
    if event.pubkey != event.pubkey.to_lowercase() {
        return false;
    }

    // Check sig is 128 hex characters
    if event.sig.len() != 128 || !event.sig.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }

    true
}

/// Sign an event template with a secret key, producing a complete signed event.
#[cfg(feature = "full")]
pub fn finalize_event(
    template: &EventTemplate,
    secret_key: &[u8; 32],
) -> Result<Event, Nip01Error> {
    let secp = Secp256k1::new();

    // Get public key
    let sk = SecretKey::from_slice(secret_key).map_err(|e| Nip01Error::Signing(e.to_string()))?;
    let (xonly_pk, _parity) = sk.x_only_public_key(&secp);
    let pubkey = hex::encode(xonly_pk.serialize());

    // Create unsigned event
    let unsigned = UnsignedEvent {
        pubkey: pubkey.clone(),
        created_at: template.created_at,
        kind: template.kind,
        tags: template.tags.clone(),
        content: template.content.clone(),
    };

    // Get event id (hash)
    let id = get_event_hash(&unsigned)?;

    // Sign the id
    let id_bytes =
        hex::decode(&id).map_err(|e| Nip01Error::Signing(format!("invalid id hex: {}", e)))?;
    let message = Message::from_digest_slice(&id_bytes)
        .map_err(|e| Nip01Error::Signing(format!("invalid message: {}", e)))?;

    let keypair = bitcoin::secp256k1::Keypair::from_secret_key(&secp, &sk);
    let sig = secp.sign_schnorr_no_aux_rand(&message, &keypair);
    let sig_hex = hex::encode(sig.serialize());

    Ok(Event {
        id,
        pubkey,
        created_at: template.created_at,
        kind: template.kind,
        tags: template.tags.clone(),
        content: template.content.clone(),
        sig: sig_hex,
    })
}

/// Verify an event's signature and id.
#[cfg(feature = "full")]
pub fn verify_event(event: &Event) -> Result<bool, Nip01Error> {
    // First validate structure
    if !validate_event(event) {
        return Ok(false);
    }

    // Reconstruct unsigned event and verify hash matches id
    let unsigned = UnsignedEvent {
        pubkey: event.pubkey.clone(),
        created_at: event.created_at,
        kind: event.kind,
        tags: event.tags.clone(),
        content: event.content.clone(),
    };

    let computed_id = get_event_hash(&unsigned)?;
    if computed_id != event.id {
        return Ok(false);
    }

    // Verify signature
    let secp = Secp256k1::verification_only();

    let id_bytes = hex::decode(&event.id)
        .map_err(|e| Nip01Error::Verification(format!("invalid id hex: {}", e)))?;
    let message = Message::from_digest_slice(&id_bytes)
        .map_err(|e| Nip01Error::Verification(format!("invalid message: {}", e)))?;

    let sig_bytes = hex::decode(&event.sig)
        .map_err(|e| Nip01Error::Verification(format!("invalid sig hex: {}", e)))?;
    let sig = schnorr::Signature::from_slice(&sig_bytes)
        .map_err(|e| Nip01Error::Verification(format!("invalid signature: {}", e)))?;

    let pubkey_bytes = hex::decode(&event.pubkey)
        .map_err(|e| Nip01Error::Verification(format!("invalid pubkey hex: {}", e)))?;
    let pubkey = XOnlyPublicKey::from_slice(&pubkey_bytes)
        .map_err(|e| Nip01Error::Verification(format!("invalid pubkey: {}", e)))?;

    match secp.verify_schnorr(&sig, &message, &pubkey) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Classify an event kind according to NIP-01 rules.
pub fn classify_kind(kind: u16) -> KindClassification {
    let k = kind as u32;

    // Regular: 1000 <= n < 10000 || 4 <= n < 45 || n == 1 || n == 2
    if (1000..10000).contains(&k) || (4..45).contains(&k) || k == 1 || k == 2 {
        return KindClassification::Regular;
    }

    // Replaceable: 10000 <= n < 20000 || n == 0 || n == 3
    if (10000..20000).contains(&k) || k == 0 || k == 3 {
        return KindClassification::Replaceable;
    }

    // Ephemeral: 20000 <= n < 30000
    if (20000..30000).contains(&k) {
        return KindClassification::Ephemeral;
    }

    // Addressable: 30000 <= n < 40000
    if (30000..40000).contains(&k) {
        return KindClassification::Addressable;
    }

    KindClassification::Unknown
}

/// Check if a kind is regular.
pub fn is_regular_kind(kind: u16) -> bool {
    matches!(classify_kind(kind), KindClassification::Regular)
}

/// Check if a kind is replaceable.
pub fn is_replaceable_kind(kind: u16) -> bool {
    matches!(classify_kind(kind), KindClassification::Replaceable)
}

/// Check if a kind is ephemeral.
pub fn is_ephemeral_kind(kind: u16) -> bool {
    matches!(classify_kind(kind), KindClassification::Ephemeral)
}

/// Check if a kind is addressable.
pub fn is_addressable_kind(kind: u16) -> bool {
    matches!(classify_kind(kind), KindClassification::Addressable)
}

/// Sort events in reverse-chronological order by created_at,
/// then by id (lexicographically) in case of ties.
pub fn sort_events(events: &mut [Event]) {
    events.sort_by(|a, b| match b.created_at.cmp(&a.created_at) {
        std::cmp::Ordering::Equal => a.id.cmp(&b.id),
        other => other,
    });
}

#[cfg(all(test, feature = "full"))]
mod tests {
    use super::*;

    // Test private key used in nostr-tools tests
    const TEST_PRIVATE_KEY: &str =
        "d217c1ff2f8a65c3e3a1740db3b9f58b8c848bb45e26d00ed4714e4a0f4ceecf";

    fn test_private_key() -> [u8; 32] {
        let bytes = hex::decode(TEST_PRIVATE_KEY).unwrap();
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        key
    }

    // =========================================================================
    // Key generation tests (mirrors nostr-tools pure.test.ts)
    // =========================================================================

    #[test]
    fn test_private_key_generation() {
        let sk = generate_secret_key();
        let hex = hex::encode(sk);
        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_public_key_generation() {
        let sk = generate_secret_key();
        let pk = get_public_key_hex(&sk).unwrap();
        assert_eq!(pk.len(), 64);
        assert!(pk.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_public_key_from_private_key_deterministic() {
        let sk = generate_secret_key();
        let pk = get_public_key_hex(&sk).unwrap();

        // Same private key should always produce the same public key
        for _ in 0..5 {
            assert_eq!(get_public_key_hex(&sk).unwrap(), pk);
        }
    }

    // =========================================================================
    // finalize_event tests (mirrors nostr-tools)
    // =========================================================================

    #[test]
    fn test_finalize_event_creates_signed_event() {
        let private_key = test_private_key();
        let public_key = get_public_key_hex(&private_key).unwrap();

        let template = EventTemplate {
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "Hello, world!".to_string(),
            created_at: 1617932115,
        };

        let event = finalize_event(&template, &private_key).unwrap();

        assert_eq!(event.kind, template.kind);
        assert_eq!(event.tags, template.tags);
        assert_eq!(event.content, template.content);
        assert_eq!(event.created_at, template.created_at);
        assert_eq!(event.pubkey, public_key);
        assert_eq!(event.id.len(), 64);
        assert_eq!(event.sig.len(), 128);
    }

    // =========================================================================
    // serialize_event tests (mirrors nostr-tools)
    // =========================================================================

    #[test]
    fn test_serialize_event_valid() {
        let private_key = test_private_key();
        let public_key = get_public_key_hex(&private_key).unwrap();

        let unsigned = UnsignedEvent {
            pubkey: public_key.clone(),
            created_at: 1617932115,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "Hello, world!".to_string(),
        };

        let serialized = serialize_event(&unsigned).unwrap();

        // Should match the format [0, pubkey, created_at, kind, tags, content]
        let expected = format!("[0,\"{}\",1617932115,1,[],\"Hello, world!\"]", public_key);
        assert_eq!(serialized, expected);
    }

    #[test]
    fn test_serialize_event_invalid_pubkey() {
        let unsigned = UnsignedEvent {
            pubkey: "invalid".to_string(), // Not 64 hex chars
            created_at: 1617932115,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "Hello, world!".to_string(),
        };

        let result = serialize_event(&unsigned);
        assert!(result.is_err());
    }

    // =========================================================================
    // get_event_hash tests (mirrors nostr-tools)
    // =========================================================================

    #[test]
    fn test_get_event_hash() {
        let private_key = test_private_key();
        let public_key = get_public_key_hex(&private_key).unwrap();

        let unsigned = UnsignedEvent {
            pubkey: public_key,
            created_at: 1617932115,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "Hello, world!".to_string(),
        };

        let hash = get_event_hash(&unsigned).unwrap();

        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    // =========================================================================
    // validate_event tests (mirrors nostr-tools)
    // =========================================================================

    #[test]
    fn test_validate_unsigned_event_valid() {
        let private_key = test_private_key();
        let public_key = get_public_key_hex(&private_key).unwrap();

        let unsigned = UnsignedEvent {
            pubkey: public_key,
            created_at: 1617932115,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "Hello, world!".to_string(),
        };

        assert!(validate_unsigned_event(&unsigned));
    }

    #[test]
    fn test_validate_unsigned_event_invalid_pubkey() {
        let unsigned = UnsignedEvent {
            pubkey: "invalid_pubkey".to_string(),
            created_at: 1617932115,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "Hello, world!".to_string(),
        };

        assert!(!validate_unsigned_event(&unsigned));
    }

    #[test]
    fn test_validate_unsigned_event_uppercase_pubkey() {
        let private_key = test_private_key();
        let public_key = get_public_key_hex(&private_key).unwrap().to_uppercase();

        let unsigned = UnsignedEvent {
            pubkey: public_key,
            created_at: 1617932115,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "Hello, world!".to_string(),
        };

        assert!(!validate_unsigned_event(&unsigned));
    }

    // =========================================================================
    // verify_event tests (mirrors nostr-tools)
    // =========================================================================

    #[test]
    fn test_verify_event_valid_signature() {
        let private_key = test_private_key();

        let template = EventTemplate {
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "Hello, world!".to_string(),
            created_at: 1617932115,
        };

        let event = finalize_event(&template, &private_key).unwrap();
        assert!(verify_event(&event).unwrap());
    }

    #[test]
    fn test_verify_event_invalid_signature() {
        let private_key = test_private_key();

        let template = EventTemplate {
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "Hello, world!".to_string(),
            created_at: 1617932115,
        };

        let mut event = finalize_event(&template, &private_key).unwrap();

        // Tamper with the signature
        let mut sig_chars: Vec<char> = event.sig.chars().collect();
        sig_chars[0] = '6';
        sig_chars[1] = '6';
        sig_chars[2] = '6';
        event.sig = sig_chars.into_iter().collect();

        assert!(!verify_event(&event).unwrap());
    }

    #[test]
    fn test_verify_event_wrong_pubkey() {
        let private_key1 = test_private_key();
        let private_key2_hex = "5b4a34f4e4b23c63ad55a35e3f84a3b53d96dbf266edf521a8358f71d19cbf67";
        let private_key2_bytes = hex::decode(private_key2_hex).unwrap();
        let mut private_key2 = [0u8; 32];
        private_key2.copy_from_slice(&private_key2_bytes);

        let public_key2 = get_public_key_hex(&private_key2).unwrap();

        let template = EventTemplate {
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "Hello, world!".to_string(),
            created_at: 1617932115,
        };

        let mut event = finalize_event(&template, &private_key1).unwrap();
        // Replace pubkey with a different one
        event.pubkey = public_key2;

        assert!(!verify_event(&event).unwrap());
    }

    #[test]
    fn test_verify_event_invalid_id() {
        let private_key = test_private_key();

        let template = EventTemplate {
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "Hello, world!".to_string(),
            created_at: 1617932115,
        };

        let mut event = finalize_event(&template, &private_key).unwrap();

        // Tamper with the id
        let mut id_chars: Vec<char> = event.id.chars().collect();
        id_chars[0] = '6';
        id_chars[1] = '6';
        id_chars[2] = '6';
        event.id = id_chars.into_iter().collect();

        assert!(!verify_event(&event).unwrap());
    }

    // =========================================================================
    // sort_events tests (mirrors nostr-tools core.test.ts)
    // =========================================================================

    #[test]
    fn test_sort_events() {
        let mut events = vec![
            Event {
                id: "abc123".to_string(),
                pubkey: "a".repeat(64),
                created_at: 1610000000,
                kind: 1,
                tags: vec![],
                content: "Hello".to_string(),
                sig: "a".repeat(128),
            },
            Event {
                id: "abc124".to_string(),
                pubkey: "a".repeat(64),
                created_at: 1620000000,
                kind: 1,
                tags: vec![],
                content: "World".to_string(),
                sig: "a".repeat(128),
            },
            Event {
                id: "abc125".to_string(),
                pubkey: "a".repeat(64),
                created_at: 1620000000,
                kind: 1,
                tags: vec![],
                content: "!".to_string(),
                sig: "a".repeat(128),
            },
        ];

        sort_events(&mut events);

        // Should be sorted by created_at descending, then by id ascending
        assert_eq!(events[0].id, "abc124");
        assert_eq!(events[1].id, "abc125");
        assert_eq!(events[2].id, "abc123");
    }

    // =========================================================================
    // Kind classification tests (mirrors nostr-tools kinds.ts)
    // =========================================================================

    #[test]
    fn test_is_regular_kind() {
        assert!(is_regular_kind(1)); // ShortTextNote
        assert!(is_regular_kind(2)); // RecommendRelay
        assert!(is_regular_kind(4)); // EncryptedDirectMessage
        assert!(is_regular_kind(7)); // Reaction
        assert!(is_regular_kind(1000));
        assert!(is_regular_kind(9999));

        assert!(!is_regular_kind(0)); // Metadata is replaceable
        assert!(!is_regular_kind(3)); // Contacts is replaceable
    }

    #[test]
    fn test_is_replaceable_kind() {
        assert!(is_replaceable_kind(0)); // Metadata
        assert!(is_replaceable_kind(3)); // Contacts
        assert!(is_replaceable_kind(10000));
        assert!(is_replaceable_kind(19999));

        assert!(!is_replaceable_kind(1));
        assert!(!is_replaceable_kind(20000));
    }

    #[test]
    fn test_is_ephemeral_kind() {
        assert!(is_ephemeral_kind(20000));
        assert!(is_ephemeral_kind(25000));
        assert!(is_ephemeral_kind(29999));

        assert!(!is_ephemeral_kind(19999));
        assert!(!is_ephemeral_kind(30000));
    }

    #[test]
    fn test_is_addressable_kind() {
        assert!(is_addressable_kind(30000));
        assert!(is_addressable_kind(35000));
        assert!(is_addressable_kind(39999));

        assert!(!is_addressable_kind(29999));
        assert!(!is_addressable_kind(40000));
    }

    #[test]
    fn test_classify_kind() {
        assert_eq!(classify_kind(1), KindClassification::Regular);
        assert_eq!(classify_kind(0), KindClassification::Replaceable);
        assert_eq!(classify_kind(20000), KindClassification::Ephemeral);
        assert_eq!(classify_kind(30000), KindClassification::Addressable);
        assert_eq!(classify_kind(50000), KindClassification::Unknown);
    }

    // =========================================================================
    // Additional tests
    // =========================================================================

    #[test]
    fn test_event_with_tags() {
        let private_key = test_private_key();

        let template = EventTemplate {
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![
                vec!["e".to_string(), "abc123".to_string()],
                vec!["p".to_string(), "def456".to_string()],
            ],
            content: "Hello with tags!".to_string(),
            created_at: 1617932115,
        };

        let event = finalize_event(&template, &private_key).unwrap();
        assert!(verify_event(&event).unwrap());
        assert_eq!(event.tags.len(), 2);
        assert_eq!(event.tags[0][0], "e");
        assert_eq!(event.tags[1][0], "p");
    }

    #[test]
    fn test_event_with_special_characters_in_content() {
        let private_key = test_private_key();

        let template = EventTemplate {
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "Hello\nWorld\t\"quotes\" and \\backslash".to_string(),
            created_at: 1617932115,
        };

        let event = finalize_event(&template, &private_key).unwrap();
        assert!(verify_event(&event).unwrap());
    }

    #[test]
    fn test_event_with_unicode_content() {
        let private_key = test_private_key();

        let template = EventTemplate {
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "Hello 世界 🌍 مرحبا".to_string(),
            created_at: 1617932115,
        };

        let event = finalize_event(&template, &private_key).unwrap();
        assert!(verify_event(&event).unwrap());
    }

    #[test]
    fn test_event_roundtrip_json() {
        let private_key = test_private_key();

        let template = EventTemplate {
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![vec!["t".to_string(), "nostr".to_string()]],
            content: "Testing JSON roundtrip".to_string(),
            created_at: 1617932115,
        };

        let event = finalize_event(&template, &private_key).unwrap();

        // Serialize to JSON
        let json = serde_json::to_string(&event).unwrap();

        // Deserialize back
        let event2: Event = serde_json::from_str(&json).unwrap();

        assert_eq!(event, event2);
        assert!(verify_event(&event2).unwrap());
    }

    #[test]
    fn test_deterministic_event_id() {
        let private_key = test_private_key();
        let public_key = get_public_key_hex(&private_key).unwrap();

        let unsigned = UnsignedEvent {
            pubkey: public_key,
            created_at: 1617932115,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "Hello, world!".to_string(),
        };

        // Same event should always produce the same hash
        let hash1 = get_event_hash(&unsigned).unwrap();
        let hash2 = get_event_hash(&unsigned).unwrap();
        assert_eq!(hash1, hash2);
    }
}
