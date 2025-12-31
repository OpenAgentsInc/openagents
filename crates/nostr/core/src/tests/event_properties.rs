//! Property-based tests for Nostr event signature verification
//!
//! These tests use quickcheck to verify cryptographic properties hold for all valid inputs:
//! 1. Any event signed with a keypair can be verified with its public key
//! 2. Signature verification fails for tampered event content
//! 3. Signature verification fails for wrong public key
//! 4. Event ID computation is deterministic
//! 5. Schnorr signature properties hold for all valid events

use crate::{EventTemplate, finalize_event, generate_secret_key, verify_event};
use quickcheck::{Arbitrary, Gen, quickcheck};
use std::time::{SystemTime, UNIX_EPOCH};

/// Arbitrary event content for property testing
#[derive(Clone, Debug)]
struct ArbitraryEventContent {
    content: String,
    kind: u16,
}

impl Arbitrary for ArbitraryEventContent {
    fn arbitrary(g: &mut Gen) -> Self {
        let content_variants = vec![
            "Hello Nostr!",
            "Test message",
            "",
            "Long content with multiple words and punctuation!",
            "Unicode: 🚀 ⚡ 🎉",
            "{ \"key\": \"value\" }",
        ];

        let content = g
            .choose(&content_variants)
            .unwrap_or(&"default")
            .to_string();

        // Use common event kinds for testing
        let kind_variants = vec![1, 3, 4, 7, 1984, 30023];
        let kind = *g.choose(&kind_variants).unwrap_or(&1);

        ArbitraryEventContent { content, kind }
    }
}

#[test]
fn test_property_sign_verify_roundtrip() {
    // Property: Any event signed with a keypair can be verified with its public key
    fn prop(event_content: ArbitraryEventContent) -> bool {
        // Generate keypair
        let secret_key = generate_secret_key();

        // Create event template
        let template = EventTemplate {
            kind: event_content.kind,
            content: event_content.content,
            tags: vec![],
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        // Sign event
        let event = match finalize_event(&template, &secret_key) {
            Ok(e) => e,
            Err(_) => return false,
        };

        // Verify event - returns Result<bool, _>, unwrap the boolean
        verify_event(&event).unwrap_or(false)
    }

    quickcheck(prop as fn(ArbitraryEventContent) -> bool);
}

#[test]
fn test_property_tampered_content_fails() {
    // Property: Signature verification fails for tampered event content
    fn prop(event_content: ArbitraryEventContent) -> bool {
        // Generate keypair
        let secret_key = generate_secret_key();

        // Create and sign event
        let template = EventTemplate {
            kind: event_content.kind,
            content: event_content.content.clone(),
            tags: vec![],
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        let mut event = match finalize_event(&template, &secret_key) {
            Ok(e) => e,
            Err(_) => return true, // If signing fails, property holds vacuously
        };

        // Tamper with content (if not empty)
        if !event.content.is_empty() {
            event.content.push('X'); // Modify content

            // Verification should fail - check that verify returns false
            return !verify_event(&event).unwrap_or(false);
        }

        true
    }

    quickcheck(prop as fn(ArbitraryEventContent) -> bool);
}

#[test]
fn test_property_wrong_pubkey_fails() {
    // Property: Signature verification fails for wrong public key
    fn prop(event_content: ArbitraryEventContent) -> bool {
        // Generate two different keypairs
        let secret_key1 = generate_secret_key();
        let secret_key2 = generate_secret_key();

        // Create and sign event with first keypair
        let template = EventTemplate {
            kind: event_content.kind,
            content: event_content.content,
            tags: vec![],
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        let mut event = match finalize_event(&template, &secret_key1) {
            Ok(e) => e,
            Err(_) => return true,
        };

        // Replace pubkey with second keypair's pubkey
        // Compute public key from second secret key
        use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
        let secp = Secp256k1::new();
        let sk2 = SecretKey::from_slice(&secret_key2).unwrap();
        let pk2 = PublicKey::from_secret_key(&secp, &sk2);
        let pk2_bytes = pk2.serialize();

        // Replace pubkey (x-only, so take last 32 bytes)
        let x_only_pk = &pk2_bytes[1..33];
        event.pubkey = hex::encode(x_only_pk);

        // Verification should fail (wrong pubkey for this signature)
        !verify_event(&event).unwrap_or(false)
    }

    quickcheck(prop as fn(ArbitraryEventContent) -> bool);
}

#[test]
fn test_property_event_id_deterministic() {
    // Property: Event ID computation is deterministic (same event = same ID)
    fn prop(event_content: ArbitraryEventContent) -> bool {
        let secret_key = generate_secret_key();

        // Create event template with fixed timestamp for determinism
        let fixed_timestamp = 1704067200; // 2024-01-01 00:00:00 UTC
        let template = EventTemplate {
            kind: event_content.kind,
            content: event_content.content,
            tags: vec![],
            created_at: fixed_timestamp,
        };

        // Sign event twice
        let event1 = match finalize_event(&template, &secret_key) {
            Ok(e) => e,
            Err(_) => return true,
        };

        let event2 = match finalize_event(&template, &secret_key) {
            Ok(e) => e,
            Err(_) => return true,
        };

        // Event IDs should be identical
        event1.id == event2.id
    }

    quickcheck(prop as fn(ArbitraryEventContent) -> bool);
}

#[test]
fn test_property_signature_uniqueness() {
    // Property: Different events produce different signatures
    fn prop(content1: String, content2: String) -> bool {
        if content1 == content2 {
            return true; // Skip if contents are the same
        }

        let secret_key = generate_secret_key();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let template1 = EventTemplate {
            kind: 1,
            content: content1,
            tags: vec![],
            created_at: timestamp,
        };

        let template2 = EventTemplate {
            kind: 1,
            content: content2,
            tags: vec![],
            created_at: timestamp,
        };

        let event1 = match finalize_event(&template1, &secret_key) {
            Ok(e) => e,
            Err(_) => return true,
        };

        let event2 = match finalize_event(&template2, &secret_key) {
            Ok(e) => e,
            Err(_) => return true,
        };

        // Different content should produce different signatures
        event1.sig != event2.sig
    }

    quickcheck(prop as fn(String, String) -> bool);
}

#[test]
fn test_property_valid_event_structure() {
    // Property: All signed events have valid structure
    fn prop(event_content: ArbitraryEventContent) -> bool {
        let secret_key = generate_secret_key();

        let template = EventTemplate {
            kind: event_content.kind,
            content: event_content.content,
            tags: vec![],
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        let event = match finalize_event(&template, &secret_key) {
            Ok(e) => e,
            Err(_) => return true,
        };

        // Verify structural properties
        event.id.len() == 64 && // SHA256 hash is 64 hex chars
        event.pubkey.len() == 64 && // 32-byte pubkey is 64 hex chars
        event.sig.len() == 128 && // 64-byte signature is 128 hex chars
        event.kind == template.kind &&
        event.content == template.content &&
        event.created_at == template.created_at
    }

    quickcheck(prop as fn(ArbitraryEventContent) -> bool);
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_basic_sign_verify() {
        // Sanity check that the basic flow works
        let sk = generate_secret_key();
        let template = EventTemplate {
            kind: 1,
            content: "test".to_string(),
            tags: vec![],
            created_at: 1234567890,
        };

        let event = finalize_event(&template, &sk).unwrap();
        assert!(verify_event(&event).unwrap_or(false));
    }

    #[test]
    fn test_tampered_content() {
        let sk = generate_secret_key();
        let template = EventTemplate {
            kind: 1,
            content: "original".to_string(),
            tags: vec![],
            created_at: 1234567890,
        };

        let mut event = finalize_event(&template, &sk).unwrap();
        event.content = "tampered".to_string();

        assert!(!verify_event(&event).unwrap_or(false));
    }
}
