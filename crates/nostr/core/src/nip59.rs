//! NIP-59: Gift Wrap
//!
//! This NIP defines a protocol for encapsulating any nostr event to obscure metadata.
//! It uses three main concepts: rumors (unsigned events), seals (kind 13), and gift wraps (kind 1059).
//!
//! ## Protocol Flow
//! 1. Create a rumor (unsigned event with id but no signature)
//! 2. Seal the rumor (encrypt with NIP-44, sign as kind 13)
//! 3. Gift wrap the seal (encrypt with random key as kind 1059)
//!
//! This provides deniability, sender privacy, and recipient privacy.

use crate::nip01::{Event, EventTemplate, UnsignedEvent};
use crate::nip44::{decrypt as nip44_decrypt, encrypt as nip44_encrypt};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind for seal events
pub const KIND_SEAL: u16 = 13;

/// Kind for gift wrap events
pub const KIND_GIFT_WRAP: u16 = 1059;

/// Two days in seconds (for randomizing timestamps)
const TWO_DAYS_SECS: i64 = 2 * 24 * 60 * 60;

/// Errors that can occur during NIP-59 operations.
#[derive(Debug, Error)]
pub enum Nip59Error {
    #[error("invalid kind: expected {expected}, got {got}")]
    InvalidKind { expected: u16, got: u16 },

    #[error("rumor must not have a signature")]
    RumorHasSignature,

    #[error("seal must have empty tags")]
    SealHasNonEmptyTags,

    #[error("gift wrap missing p tag")]
    GiftWrapMissingPTag,

    #[error("encryption error: {0}")]
    Encryption(String),

    #[error("decryption error: {0}")]
    Decryption(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("NIP-01 error: {0}")]
    Nip01(#[from] crate::nip01::Nip01Error),

    #[error("NIP-44 error: {0}")]
    Nip44(#[from] crate::nip44::Nip44Error),
}

/// A rumor is an unsigned event (no signature).
/// It has an id but is not signed, providing deniability if leaked.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rumor {
    /// Event ID (hash of serialized event)
    pub id: String,
    /// Author's public key
    pub pubkey: String,
    /// Unix timestamp
    pub created_at: u64,
    /// Event kind
    pub kind: u16,
    /// Event tags
    pub tags: Vec<Vec<String>>,
    /// Event content
    pub content: String,
}

impl Rumor {
    /// Create a new rumor from an unsigned event.
    pub fn new(unsigned: UnsignedEvent) -> Result<Self, Nip59Error> {
        let id = crate::nip01::get_event_hash(&unsigned)?;

        Ok(Self {
            id,
            pubkey: unsigned.pubkey,
            created_at: unsigned.created_at,
            kind: unsigned.kind,
            tags: unsigned.tags,
            content: unsigned.content,
        })
    }

    /// Convert rumor to UnsignedEvent for further processing.
    pub fn to_unsigned_event(&self) -> UnsignedEvent {
        UnsignedEvent {
            pubkey: self.pubkey.clone(),
            created_at: self.created_at,
            kind: self.kind,
            tags: self.tags.clone(),
            content: self.content.clone(),
        }
    }

    /// Serialize to JSON for encryption.
    pub fn to_json(&self) -> Result<String, Nip59Error> {
        serde_json::to_string(self).map_err(|e| Nip59Error::Serialization(e.to_string()))
    }

    /// Deserialize from JSON after decryption.
    pub fn from_json(json: &str) -> Result<Self, Nip59Error> {
        serde_json::from_str(json).map_err(|e| Nip59Error::Serialization(e.to_string()))
    }
}

/// Generate a random timestamp up to 2 days in the past.
pub fn random_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Random offset between 0 and 2 days
    let offset = rand::random::<u64>() % TWO_DAYS_SECS as u64;
    now - offset
}

/// Create a seal event (kind 13) from a rumor.
///
/// The seal encrypts the rumor using NIP-44 with the sender's private key
/// and recipient's public key. The seal is signed by the sender.
///
/// # Arguments
/// * `rumor` - The unsigned event to seal
/// * `sender_private_key` - Sender's private key (32 bytes)
/// * `recipient_public_key` - Recipient's public key (hex string)
pub fn create_seal(
    rumor: &Rumor,
    sender_private_key: &[u8; 32],
    recipient_public_key: &str,
) -> Result<Event, Nip59Error> {
    // Serialize rumor to JSON
    let rumor_json = rumor.to_json()?;

    // Encrypt rumor with NIP-44
    // Convert x-only public key (32 bytes hex) to compressed format (33 bytes)
    let recipient_pubkey_bytes = hex::decode(recipient_public_key)
        .map_err(|e| Nip59Error::Encryption(format!("Invalid pubkey hex: {}", e)))?;

    // Add 0x02 prefix for compressed public key (assuming even parity)
    let mut compressed_pubkey = vec![0x02];
    compressed_pubkey.extend_from_slice(&recipient_pubkey_bytes);

    let encrypted_content = nip44_encrypt(sender_private_key, &compressed_pubkey, &rumor_json)?;

    // Create seal event template
    let seal_template = EventTemplate {
        created_at: random_timestamp(),
        kind: KIND_SEAL,
        tags: vec![], // MUST be empty per NIP-59
        content: encrypted_content,
    };

    // Sign the seal
    let seal = crate::nip01::finalize_event(&seal_template, sender_private_key)?;

    Ok(seal)
}

/// Unwrap a seal event to recover the rumor.
///
/// # Arguments
/// * `seal` - The seal event (kind 13)
/// * `recipient_private_key` - Recipient's private key (32 bytes)
pub fn unwrap_seal(seal: &Event, recipient_private_key: &[u8; 32]) -> Result<Rumor, Nip59Error> {
    // Validate seal kind
    if seal.kind != KIND_SEAL {
        return Err(Nip59Error::InvalidKind {
            expected: KIND_SEAL,
            got: seal.kind,
        });
    }

    // Validate seal has no tags
    if !seal.tags.is_empty() {
        return Err(Nip59Error::SealHasNonEmptyTags);
    }

    // Decrypt seal content to get rumor JSON
    // Convert x-only public key (32 bytes hex) to compressed format (33 bytes)
    let sender_pubkey_bytes = hex::decode(&seal.pubkey)
        .map_err(|e| Nip59Error::Decryption(format!("Invalid pubkey hex: {}", e)))?;

    // Add 0x02 prefix for compressed public key (assuming even parity)
    let mut compressed_pubkey = vec![0x02];
    compressed_pubkey.extend_from_slice(&sender_pubkey_bytes);

    let rumor_json = nip44_decrypt(recipient_private_key, &compressed_pubkey, &seal.content)?;

    // Deserialize rumor
    let rumor = Rumor::from_json(&rumor_json)?;

    // Verify rumor pubkey matches seal pubkey
    if rumor.pubkey != seal.pubkey {
        return Err(Nip59Error::Decryption(
            "Rumor pubkey does not match seal pubkey".to_string(),
        ));
    }

    Ok(rumor)
}

/// Create a gift wrap event (kind 1059) from a seal.
///
/// The gift wrap encrypts the seal using NIP-44 with a random ephemeral key
/// and the recipient's public key. The gift wrap is signed by the random key.
///
/// # Arguments
/// * `seal` - The seal event to wrap
/// * `recipient_public_key` - Recipient's public key (hex string)
pub fn create_gift_wrap(seal: &Event, recipient_public_key: &str) -> Result<Event, Nip59Error> {
    // Generate random ephemeral key
    let random_private_key = crate::nip01::generate_secret_key();

    // Serialize seal to JSON
    let seal_json =
        serde_json::to_string(seal).map_err(|e| Nip59Error::Serialization(e.to_string()))?;

    // Encrypt seal with NIP-44 using random key
    // Convert x-only public key (32 bytes hex) to compressed format (33 bytes)
    let recipient_pubkey_bytes = hex::decode(recipient_public_key)
        .map_err(|e| Nip59Error::Encryption(format!("Invalid pubkey hex: {}", e)))?;

    // Add 0x02 prefix for compressed public key (assuming even parity)
    let mut compressed_pubkey = vec![0x02];
    compressed_pubkey.extend_from_slice(&recipient_pubkey_bytes);

    let encrypted_content = nip44_encrypt(&random_private_key, &compressed_pubkey, &seal_json)?;

    // Create gift wrap event template
    let wrap_template = EventTemplate {
        created_at: random_timestamp(),
        kind: KIND_GIFT_WRAP,
        tags: vec![vec!["p".to_string(), recipient_public_key.to_string()]],
        content: encrypted_content,
    };

    // Sign the gift wrap with the random key
    let wrap = crate::nip01::finalize_event(&wrap_template, &random_private_key)?;

    Ok(wrap)
}

/// Unwrap a gift wrap event to recover the seal.
///
/// # Arguments
/// * `wrap` - The gift wrap event (kind 1059)
/// * `recipient_private_key` - Recipient's private key (32 bytes)
pub fn unwrap_gift_wrap(
    wrap: &Event,
    recipient_private_key: &[u8; 32],
) -> Result<Event, Nip59Error> {
    // Validate gift wrap kind
    if wrap.kind != KIND_GIFT_WRAP {
        return Err(Nip59Error::InvalidKind {
            expected: KIND_GIFT_WRAP,
            got: wrap.kind,
        });
    }

    // Verify p tag exists
    let has_p_tag = wrap.tags.iter().any(|tag| tag.len() >= 2 && tag[0] == "p");
    if !has_p_tag {
        return Err(Nip59Error::GiftWrapMissingPTag);
    }

    // Decrypt gift wrap content to get seal JSON
    // Note: wrap.pubkey is the random public key
    // Convert x-only public key (32 bytes hex) to compressed format (33 bytes)
    let random_pubkey_bytes = hex::decode(&wrap.pubkey)
        .map_err(|e| Nip59Error::Decryption(format!("Invalid pubkey hex: {}", e)))?;

    // Add 0x02 prefix for compressed public key (assuming even parity)
    let mut compressed_pubkey = vec![0x02];
    compressed_pubkey.extend_from_slice(&random_pubkey_bytes);

    let seal_json = nip44_decrypt(recipient_private_key, &compressed_pubkey, &wrap.content)?;

    // Deserialize seal
    let seal: Event =
        serde_json::from_str(&seal_json).map_err(|e| Nip59Error::Serialization(e.to_string()))?;

    Ok(seal)
}

/// Complete workflow: Create a rumor, seal it, and gift wrap it.
///
/// This is the high-level function for sending a gift-wrapped message.
///
/// # Arguments
/// * `unsigned_event` - The event to send (will become a rumor)
/// * `sender_private_key` - Sender's private key (32 bytes)
/// * `recipient_public_key` - Recipient's public key (hex string)
///
/// # Returns
/// The gift wrap event ready to publish
pub fn gift_wrap(
    unsigned_event: UnsignedEvent,
    sender_private_key: &[u8; 32],
    recipient_public_key: &str,
) -> Result<Event, Nip59Error> {
    // Create rumor
    let rumor = Rumor::new(unsigned_event)?;

    // Create seal
    let seal = create_seal(&rumor, sender_private_key, recipient_public_key)?;

    // Create gift wrap
    let wrap = create_gift_wrap(&seal, recipient_public_key)?;

    Ok(wrap)
}

/// Complete workflow: Unwrap a gift wrap to recover the original rumor.
///
/// This is the high-level function for receiving a gift-wrapped message.
///
/// # Arguments
/// * `wrap` - The gift wrap event received
/// * `recipient_private_key` - Recipient's private key (32 bytes)
///
/// # Returns
/// The original rumor (unsigned event with id)
pub fn unwrap_gift_wrap_full(
    wrap: &Event,
    recipient_private_key: &[u8; 32],
) -> Result<Rumor, Nip59Error> {
    // Unwrap gift wrap to get seal
    let seal = unwrap_gift_wrap(wrap, recipient_private_key)?;

    // Unwrap seal to get rumor
    let rumor = unwrap_seal(&seal, recipient_private_key)?;

    Ok(rumor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_rumor() {
        let sk = crate::nip01::generate_secret_key();
        let pk = crate::nip01::get_public_key_hex(&sk).unwrap();

        let unsigned = UnsignedEvent {
            pubkey: pk.clone(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "Hello, world!".to_string(),
        };

        let rumor = Rumor::new(unsigned).unwrap();
        assert_eq!(rumor.pubkey, pk);
        assert_eq!(rumor.kind, 1);
        assert_eq!(rumor.content, "Hello, world!");
        assert!(!rumor.id.is_empty());
    }

    #[test]
    fn test_rumor_json_roundtrip() {
        let sk = crate::nip01::generate_secret_key();
        let pk = crate::nip01::get_public_key_hex(&sk).unwrap();

        let rumor = Rumor {
            id: "0".repeat(64), // Valid event ID (64 hex chars)
            pubkey: pk,
            created_at: 1234567890,
            kind: 1,
            tags: vec![vec!["test".to_string()]],
            content: "Hello".to_string(),
        };

        let json = rumor.to_json().unwrap();
        let recovered = Rumor::from_json(&json).unwrap();

        assert_eq!(rumor, recovered);
    }

    #[test]
    fn test_random_timestamp_in_past() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let random_ts = random_timestamp();

        // Should be in the past
        assert!(random_ts <= now);

        // Should be within 2 days of now
        assert!(now - random_ts <= TWO_DAYS_SECS as u64);
    }

    #[test]
    fn test_seal_creation_and_unwrap() {
        let sender_sk = crate::nip01::generate_secret_key();
        let recipient_sk = crate::nip01::generate_secret_key();
        let recipient_pk = crate::nip01::get_public_key_hex(&recipient_sk).unwrap();

        let unsigned = UnsignedEvent {
            pubkey: crate::nip01::get_public_key_hex(&sender_sk).unwrap(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "Secret message".to_string(),
        };

        let rumor = Rumor::new(unsigned).unwrap();
        let seal = create_seal(&rumor, &sender_sk, &recipient_pk).unwrap();

        // Verify seal properties
        assert_eq!(seal.kind, KIND_SEAL);
        assert!(seal.tags.is_empty());
        assert!(!seal.content.is_empty());

        // Unwrap seal
        let recovered_rumor = unwrap_seal(&seal, &recipient_sk).unwrap();
        assert_eq!(rumor, recovered_rumor);
    }

    #[test]
    fn test_gift_wrap_creation_and_unwrap() {
        let sender_sk = crate::nip01::generate_secret_key();
        let recipient_sk = crate::nip01::generate_secret_key();
        let recipient_pk = crate::nip01::get_public_key_hex(&recipient_sk).unwrap();

        let unsigned = UnsignedEvent {
            pubkey: crate::nip01::get_public_key_hex(&sender_sk).unwrap(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "Secret message".to_string(),
        };

        let rumor = Rumor::new(unsigned).unwrap();
        let seal = create_seal(&rumor, &sender_sk, &recipient_pk).unwrap();
        let wrap = create_gift_wrap(&seal, &recipient_pk).unwrap();

        // Verify wrap properties
        assert_eq!(wrap.kind, KIND_GIFT_WRAP);
        assert_eq!(wrap.tags.len(), 1);
        assert_eq!(wrap.tags[0][0], "p");
        assert_eq!(wrap.tags[0][1], recipient_pk);

        // Unwrap
        let recovered_seal = unwrap_gift_wrap(&wrap, &recipient_sk).unwrap();
        assert_eq!(seal.id, recovered_seal.id);
    }

    #[test]
    fn test_full_workflow() {
        let sender_sk = crate::nip01::generate_secret_key();
        let recipient_sk = crate::nip01::generate_secret_key();
        let recipient_pk = crate::nip01::get_public_key_hex(&recipient_sk).unwrap();

        let unsigned = UnsignedEvent {
            pubkey: crate::nip01::get_public_key_hex(&sender_sk).unwrap(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![vec!["test".to_string()]],
            content: "Are you going to the party tonight?".to_string(),
        };

        // Wrap
        let wrap = gift_wrap(unsigned.clone(), &sender_sk, &recipient_pk).unwrap();

        // Verify wrap
        assert_eq!(wrap.kind, KIND_GIFT_WRAP);

        // Unwrap
        let rumor = unwrap_gift_wrap_full(&wrap, &recipient_sk).unwrap();

        // Verify rumor matches original
        assert_eq!(rumor.pubkey, unsigned.pubkey);
        assert_eq!(rumor.kind, unsigned.kind);
        assert_eq!(rumor.tags, unsigned.tags);
        assert_eq!(rumor.content, unsigned.content);
    }

    #[test]
    fn test_invalid_seal_kind() {
        let recipient_sk = crate::nip01::generate_secret_key();

        let fake_seal = Event {
            id: "test".to_string(),
            pubkey: "test".to_string(),
            created_at: 123,
            kind: 1, // Wrong kind
            tags: vec![],
            content: "test".to_string(),
            sig: "test".to_string(),
        };

        assert!(unwrap_seal(&fake_seal, &recipient_sk).is_err());
    }

    #[test]
    fn test_seal_with_non_empty_tags() {
        let sender_sk = crate::nip01::generate_secret_key();
        let recipient_sk = crate::nip01::generate_secret_key();

        let fake_seal = Event {
            id: "test".to_string(),
            pubkey: crate::nip01::get_public_key_hex(&sender_sk).unwrap(),
            created_at: 123,
            kind: KIND_SEAL,
            tags: vec![vec!["p".to_string(), "test".to_string()]], // Should be empty
            content: "test".to_string(),
            sig: "test".to_string(),
        };

        assert!(unwrap_seal(&fake_seal, &recipient_sk).is_err());
    }

    #[test]
    fn test_gift_wrap_missing_p_tag() {
        let recipient_sk = crate::nip01::generate_secret_key();

        let fake_wrap = Event {
            id: "test".to_string(),
            pubkey: "test".to_string(),
            created_at: 123,
            kind: KIND_GIFT_WRAP,
            tags: vec![], // Missing p tag
            content: "test".to_string(),
            sig: "test".to_string(),
        };

        assert!(unwrap_gift_wrap(&fake_wrap, &recipient_sk).is_err());
    }
}
