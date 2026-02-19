//! Property-based tests for NIP-01 Event validation
//!
//! These tests use quickcheck to verify that event validation, hashing,
//! and signing operations satisfy fundamental properties for all possible inputs.

use crate::nip01::validate_unsigned_event;
#[cfg(feature = "full")]
use crate::nip01::{
    EventTemplate, UnsignedEvent, finalize_event, generate_secret_key, get_event_hash,
    get_public_key_hex, validate_event, verify_event,
};
use quickcheck::{Arbitrary, Gen};

/// Wrapper for valid hex strings (64 chars lowercase)
#[derive(Debug, Clone)]
struct HexString64(String);

impl Arbitrary for HexString64 {
    fn arbitrary(g: &mut Gen) -> Self {
        let bytes: Vec<u8> = (0..32).map(|_| u8::arbitrary(g)).collect();
        HexString64(hex::encode(bytes))
    }
}

/// Wrapper for EventTemplate to implement Arbitrary
#[derive(Debug, Clone)]
struct ArbitraryEventTemplate(EventTemplate);

impl Arbitrary for ArbitraryEventTemplate {
    fn arbitrary(g: &mut Gen) -> Self {
        // Generate reasonable timestamp (not too far in past/future)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let created_at = now.saturating_sub(86400 * 7) + (u64::arbitrary(g) % (86400 * 14));

        // Generate reasonable kind (0-65535)
        let kind = u16::arbitrary(g);

        // Generate tags (limit to reasonable size)
        let tag_count = usize::arbitrary(g) % 10;
        let tags = (0..tag_count)
            .map(|_| {
                let tag_len = 1 + (usize::arbitrary(g) % 5);
                (0..tag_len)
                    .map(|_| {
                        // Generate simple ASCII strings for tags
                        let len = 1 + (usize::arbitrary(g) % 20);
                        (0..len)
                            .map(|_| {
                                let c = char::arbitrary(g);
                                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                                    c
                                } else {
                                    'a'
                                }
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect();

        // Generate content (limit to reasonable size)
        let content_len = usize::arbitrary(g) % 1000;
        let content: String = (0..content_len)
            .map(|_| {
                let c = char::arbitrary(g);
                if c.is_ascii() && !c.is_control() {
                    c
                } else {
                    'a'
                }
            })
            .collect();

        ArbitraryEventTemplate(EventTemplate {
            created_at,
            kind,
            tags,
            content,
        })
    }
}

/// Property: validate_unsigned_event accepts well-formed events
fn prop_validate_unsigned_event_wellformed(
    hex: HexString64,
    template: ArbitraryEventTemplate,
) -> bool {
    let event = UnsignedEvent {
        pubkey: hex.0,
        created_at: template.0.created_at,
        kind: template.0.kind,
        tags: template.0.tags,
        content: template.0.content,
    };

    validate_unsigned_event(&event)
}

/// Property: validate_unsigned_event rejects uppercase pubkeys
fn prop_validate_unsigned_event_rejects_uppercase() -> bool {
    let event = UnsignedEvent {
        pubkey: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
        created_at: 1234567890,
        kind: 1,
        tags: vec![],
        content: "test".to_string(),
    };

    !validate_unsigned_event(&event)
}

/// Property: validate_unsigned_event rejects wrong-length pubkeys
fn prop_validate_unsigned_event_rejects_wrong_length() -> bool {
    let event = UnsignedEvent {
        pubkey: "abc".to_string(), // Too short
        created_at: 1234567890,
        kind: 1,
        tags: vec![],
        content: "test".to_string(),
    };

    !validate_unsigned_event(&event)
}

#[cfg(feature = "full")]
/// Property: get_event_hash is deterministic
fn prop_event_hash_deterministic(hex: HexString64, template: ArbitraryEventTemplate) -> bool {
    let event = UnsignedEvent {
        pubkey: hex.0,
        created_at: template.0.created_at,
        kind: template.0.kind,
        tags: template.0.tags,
        content: template.0.content,
    };

    if !validate_unsigned_event(&event) {
        return true; // Skip invalid events
    }

    let hash1 = get_event_hash(&event).ok();
    let hash2 = get_event_hash(&event).ok();

    hash1 == hash2 && hash1.is_some()
}

#[cfg(feature = "full")]
/// Property: event hash is always 64 hex characters
fn prop_event_hash_format(hex: HexString64, template: ArbitraryEventTemplate) -> bool {
    let event = UnsignedEvent {
        pubkey: hex.0,
        created_at: template.0.created_at,
        kind: template.0.kind,
        tags: template.0.tags,
        content: template.0.content,
    };

    if !validate_unsigned_event(&event) {
        return true; // Skip invalid events
    }

    match get_event_hash(&event) {
        Ok(hash) => {
            hash.len() == 64
                && hash.chars().all(|c| c.is_ascii_hexdigit())
                && hash == hash.to_lowercase()
        }
        Err(_) => false,
    }
}

#[cfg(feature = "full")]
/// Property: finalize_event produces valid events
fn prop_finalize_event_produces_valid(template: ArbitraryEventTemplate) -> bool {
    let secret_key = generate_secret_key();

    match finalize_event(&template.0, &secret_key) {
        Ok(event) => validate_event(&event),
        Err(_) => false,
    }
}

#[cfg(feature = "full")]
/// Property: signed events verify correctly
fn prop_signed_events_verify(template: ArbitraryEventTemplate) -> bool {
    let secret_key = generate_secret_key();

    match finalize_event(&template.0, &secret_key) {
        Ok(event) => verify_event(&event).unwrap_or_default(),
        Err(_) => false,
    }
}

#[cfg(feature = "full")]
/// Property: tampering with event content breaks verification
fn prop_tampered_event_fails_verification(template: ArbitraryEventTemplate) -> bool {
    let secret_key = generate_secret_key();

    match finalize_event(&template.0, &secret_key) {
        Ok(mut event) => {
            // Tamper with content
            event.content.push_str(" TAMPERED");

            match verify_event(&event) {
                Ok(valid) => !valid, // Should be invalid
                Err(_) => true,      // Error is also acceptable
            }
        }
        Err(_) => true, // Can't test if signing fails
    }
}

#[cfg(feature = "full")]
/// Property: signed event has valid signature format
fn prop_signed_event_signature_format(template: ArbitraryEventTemplate) -> bool {
    let secret_key = generate_secret_key();

    match finalize_event(&template.0, &secret_key) {
        Ok(event) => {
            event.sig.len() == 128
                && event.sig.chars().all(|c| c.is_ascii_hexdigit())
                && event.sig == event.sig.to_lowercase()
        }
        Err(_) => false,
    }
}

#[cfg(feature = "full")]
/// Property: event id matches hash of unsigned event
fn prop_event_id_matches_hash(template: ArbitraryEventTemplate) -> bool {
    let secret_key = generate_secret_key();

    match finalize_event(&template.0, &secret_key) {
        Ok(event) => {
            let unsigned = UnsignedEvent {
                pubkey: event.pubkey.clone(),
                created_at: event.created_at,
                kind: event.kind,
                tags: event.tags.clone(),
                content: event.content.clone(),
            };

            match get_event_hash(&unsigned) {
                Ok(hash) => hash == event.id,
                Err(_) => false,
            }
        }
        Err(_) => false,
    }
}

#[cfg(feature = "full")]
/// Property: pubkey in signed event matches secret key
fn prop_pubkey_matches_secret(template: ArbitraryEventTemplate) -> bool {
    let secret_key = generate_secret_key();

    match (
        finalize_event(&template.0, &secret_key),
        get_public_key_hex(&secret_key),
    ) {
        (Ok(event), Ok(pubkey)) => event.pubkey == pubkey,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::QuickCheck;

    #[test]
    fn test_validate_unsigned_event_wellformed() {
        QuickCheck::new().tests(50).quickcheck(
            prop_validate_unsigned_event_wellformed
                as fn(HexString64, ArbitraryEventTemplate) -> bool,
        );
    }

    #[test]
    fn test_validate_unsigned_event_rejects_uppercase() {
        assert!(prop_validate_unsigned_event_rejects_uppercase());
    }

    #[test]
    fn test_validate_unsigned_event_rejects_wrong_length() {
        assert!(prop_validate_unsigned_event_rejects_wrong_length());
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_event_hash_deterministic() {
        QuickCheck::new().tests(50).quickcheck(
            prop_event_hash_deterministic as fn(HexString64, ArbitraryEventTemplate) -> bool,
        );
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_event_hash_format() {
        QuickCheck::new()
            .tests(50)
            .quickcheck(prop_event_hash_format as fn(HexString64, ArbitraryEventTemplate) -> bool);
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_finalize_event_produces_valid() {
        QuickCheck::new()
            .tests(50)
            .quickcheck(prop_finalize_event_produces_valid as fn(ArbitraryEventTemplate) -> bool);
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_signed_events_verify() {
        QuickCheck::new()
            .tests(50)
            .quickcheck(prop_signed_events_verify as fn(ArbitraryEventTemplate) -> bool);
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_tampered_event_fails_verification() {
        QuickCheck::new().tests(50).quickcheck(
            prop_tampered_event_fails_verification as fn(ArbitraryEventTemplate) -> bool,
        );
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_signed_event_signature_format() {
        QuickCheck::new()
            .tests(50)
            .quickcheck(prop_signed_event_signature_format as fn(ArbitraryEventTemplate) -> bool);
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_event_id_matches_hash() {
        QuickCheck::new()
            .tests(50)
            .quickcheck(prop_event_id_matches_hash as fn(ArbitraryEventTemplate) -> bool);
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_pubkey_matches_secret() {
        QuickCheck::new()
            .tests(50)
            .quickcheck(prop_pubkey_matches_secret as fn(ArbitraryEventTemplate) -> bool);
    }
}
