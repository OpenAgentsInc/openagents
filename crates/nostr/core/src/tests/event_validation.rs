//! Comprehensive tests for Nostr event validation
//!
//! This module provides thorough test coverage for all event validation logic
//! including signature verification, ID verification, structure validation,
//! and error handling for malformed events.

#[cfg(feature = "full")]
use crate::nip01::{
    Event, EventTemplate, KIND_SHORT_TEXT_NOTE, UnsignedEvent, finalize_event, generate_secret_key,
    get_event_hash, get_public_key_hex, serialize_event, validate_event, verify_event,
};

// =============================================================================
// Complete Event Validation Tests
// =============================================================================

#[cfg(feature = "full")]
mod complete_event_validation {
    use super::*;

    #[test]
    fn test_validate_event_valid_structure() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test event".to_string(),
        };

        let event = finalize_event(&template, &secret_key).unwrap();
        assert!(validate_event(&event), "Valid event should pass validation");
    }

    #[test]
    fn test_validate_event_invalid_id_length() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test".to_string(),
        };

        let mut event = finalize_event(&template, &secret_key).unwrap();

        // ID too short
        event.id = "a".repeat(63);
        assert!(!validate_event(&event), "63-char ID should fail validation");

        // ID too long
        event.id = "a".repeat(65);
        assert!(!validate_event(&event), "65-char ID should fail validation");

        // ID empty
        event.id = String::new();
        assert!(!validate_event(&event), "Empty ID should fail validation");
    }

    #[test]
    fn test_validate_event_invalid_id_characters() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test".to_string(),
        };

        let mut event = finalize_event(&template, &secret_key).unwrap();

        // Non-hex characters
        event.id = format!("{}xyz", "a".repeat(61));
        assert!(
            !validate_event(&event),
            "ID with non-hex chars should fail validation"
        );

        // Special characters
        event.id = format!("{}@#$", "a".repeat(61));
        assert!(
            !validate_event(&event),
            "ID with special chars should fail validation"
        );

        // Spaces
        event.id = format!("{} {}", "a".repeat(30), "b".repeat(33));
        assert!(
            !validate_event(&event),
            "ID with spaces should fail validation"
        );
    }

    #[test]
    fn test_validate_event_invalid_pubkey_length() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test".to_string(),
        };

        let mut event = finalize_event(&template, &secret_key).unwrap();

        // Pubkey too short
        event.pubkey = "a".repeat(63);
        assert!(!validate_event(&event), "63-char pubkey should fail");

        // Pubkey too long
        event.pubkey = "a".repeat(65);
        assert!(!validate_event(&event), "65-char pubkey should fail");
    }

    #[test]
    fn test_validate_event_invalid_pubkey_uppercase() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test".to_string(),
        };

        let mut event = finalize_event(&template, &secret_key).unwrap();

        // Uppercase pubkey
        event.pubkey = event.pubkey.to_uppercase();
        assert!(
            !validate_event(&event),
            "Uppercase pubkey should fail validation"
        );

        // Mixed case pubkey
        event.pubkey = format!("A{}", "a".repeat(63));
        assert!(
            !validate_event(&event),
            "Mixed case pubkey should fail validation"
        );
    }

    #[test]
    fn test_validate_event_invalid_signature_length() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test".to_string(),
        };

        let mut event = finalize_event(&template, &secret_key).unwrap();

        // Signature too short
        event.sig = "a".repeat(127);
        assert!(!validate_event(&event), "127-char sig should fail");

        // Signature too long
        event.sig = "a".repeat(129);
        assert!(!validate_event(&event), "129-char sig should fail");

        // Signature empty
        event.sig = String::new();
        assert!(!validate_event(&event), "Empty sig should fail");
    }

    #[test]
    fn test_validate_event_invalid_signature_characters() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test".to_string(),
        };

        let mut event = finalize_event(&template, &secret_key).unwrap();

        // Non-hex characters in signature
        event.sig = format!("{}xyz{}", "a".repeat(62), "b".repeat(63));
        assert!(
            !validate_event(&event),
            "Sig with non-hex chars should fail validation"
        );
    }
}

// =============================================================================
// Event Signature Verification Tests
// =============================================================================

#[cfg(feature = "full")]
mod signature_verification {
    use super::*;

    #[test]
    fn test_verify_event_valid() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test event".to_string(),
        };

        let event = finalize_event(&template, &secret_key).unwrap();
        assert!(
            verify_event(&event).unwrap(),
            "Valid event should verify successfully"
        );
    }

    #[test]
    fn test_verify_event_tampered_content() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "original content".to_string(),
        };

        let mut event = finalize_event(&template, &secret_key).unwrap();

        // Tamper with content
        event.content = "tampered content".to_string();

        assert!(
            !verify_event(&event).unwrap(),
            "Event with tampered content should fail verification"
        );
    }

    #[test]
    fn test_verify_event_tampered_kind() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "test".to_string(),
        };

        let mut event = finalize_event(&template, &secret_key).unwrap();

        // Change kind
        event.kind = 2;

        assert!(
            !verify_event(&event).unwrap(),
            "Event with tampered kind should fail verification"
        );
    }

    #[test]
    fn test_verify_event_tampered_tags() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![vec!["e".to_string(), "abc123".to_string()]],
            content: "test".to_string(),
        };

        let mut event = finalize_event(&template, &secret_key).unwrap();

        // Add an extra tag
        event.tags.push(vec!["p".to_string(), "def456".to_string()]);

        assert!(
            !verify_event(&event).unwrap(),
            "Event with tampered tags should fail verification"
        );
    }

    #[test]
    fn test_verify_event_tampered_timestamp() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test".to_string(),
        };

        let mut event = finalize_event(&template, &secret_key).unwrap();

        // Change timestamp
        event.created_at = 9999999999;

        assert!(
            !verify_event(&event).unwrap(),
            "Event with tampered timestamp should fail verification"
        );
    }

    #[test]
    fn test_verify_event_wrong_signature() {
        let secret_key1 = generate_secret_key();
        let secret_key2 = generate_secret_key();

        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test".to_string(),
        };

        let event1 = finalize_event(&template, &secret_key1).unwrap();
        let event2 = finalize_event(&template, &secret_key2).unwrap();

        // Use event1 but with event2's signature
        let mut tampered = event1;
        tampered.sig = event2.sig;

        assert!(
            !verify_event(&tampered).unwrap(),
            "Event with wrong signature should fail verification"
        );
    }

    #[test]
    fn test_verify_event_corrupted_signature() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test".to_string(),
        };

        let mut event = finalize_event(&template, &secret_key).unwrap();

        // Corrupt the signature by flipping some bits
        let mut sig_chars: Vec<char> = event.sig.chars().collect();
        for i in 0..10 {
            sig_chars[i] = if sig_chars[i] == '0' { 'f' } else { '0' };
        }
        event.sig = sig_chars.into_iter().collect();

        assert!(
            !verify_event(&event).unwrap(),
            "Event with corrupted signature should fail verification"
        );
    }

    #[test]
    fn test_verify_event_swapped_id_and_sig() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test".to_string(),
        };

        let mut event = finalize_event(&template, &secret_key).unwrap();

        // Swap id and sig (both are hex strings, but different lengths)
        let temp = event.id.clone();
        event.id = event.sig[0..64].to_string();
        event.sig = format!("{}{}", temp, temp);

        // Structure validation passes (both correct length), but verification should fail
        assert!(
            validate_event(&event),
            "Event with swapped id/sig passes structure validation"
        );
        assert!(
            !verify_event(&event).unwrap(),
            "Event with swapped id/sig should fail signature verification"
        );
    }
}

// =============================================================================
// Event Kind Creation Tests
// =============================================================================

#[cfg(feature = "full")]
mod kind_event_creation {
    use super::*;

    #[test]
    fn test_create_metadata_event_kind_0() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: 0,
            tags: vec![],
            content: "{\"name\":\"alice\",\"about\":\"test\"}".to_string(),
        };

        let event = finalize_event(&template, &secret_key).unwrap();
        assert_eq!(event.kind, 0);
        assert!(verify_event(&event).unwrap());
    }

    #[test]
    fn test_create_ephemeral_event_kind_20000() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: 20000,
            tags: vec![vec!["t".to_string(), "ephemeral".to_string()]],
            content: "ephemeral payload".to_string(),
        };

        let event = finalize_event(&template, &secret_key).unwrap();
        assert_eq!(event.kind, 20000);
        assert!(verify_event(&event).unwrap());
    }

    #[test]
    fn test_create_parameterized_replaceable_event_kind_30000() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: 30000,
            tags: vec![vec!["d".to_string(), "profile".to_string()]],
            content: "addressable content".to_string(),
        };

        let event = finalize_event(&template, &secret_key).unwrap();
        assert_eq!(event.kind, 30000);
        assert_eq!(event.tags.len(), 1);
        assert_eq!(event.tags[0][0], "d");
        assert!(verify_event(&event).unwrap());
    }
}

// =============================================================================
// Event ID Verification Tests
// =============================================================================

#[cfg(feature = "full")]
mod id_verification {
    use super::*;

    #[test]
    fn test_verify_event_correct_id() {
        let secret_key = generate_secret_key();
        let pubkey = get_public_key_hex(&secret_key).unwrap();

        let unsigned = UnsignedEvent {
            pubkey,
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test".to_string(),
        };

        let computed_id = get_event_hash(&unsigned).unwrap();

        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test".to_string(),
        };

        let event = finalize_event(&template, &secret_key).unwrap();

        assert_eq!(event.id, computed_id, "Event ID should match computed hash");
        assert!(verify_event(&event).unwrap());
    }

    #[test]
    fn test_verify_event_wrong_id() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test".to_string(),
        };

        let mut event = finalize_event(&template, &secret_key).unwrap();

        // Replace with wrong ID
        event.id = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string();

        assert!(
            !verify_event(&event).unwrap(),
            "Event with wrong ID should fail verification"
        );
    }

    #[test]
    fn test_event_id_changes_with_content() {
        let secret_key = generate_secret_key();
        let pubkey = get_public_key_hex(&secret_key).unwrap();

        let unsigned1 = UnsignedEvent {
            pubkey: pubkey.clone(),
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test 1".to_string(),
        };

        let unsigned2 = UnsignedEvent {
            pubkey,
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test 2".to_string(),
        };

        let id1 = get_event_hash(&unsigned1).unwrap();
        let id2 = get_event_hash(&unsigned2).unwrap();

        assert_ne!(id1, id2, "Different content should produce different IDs");
    }

    #[test]
    fn test_event_id_deterministic() {
        let secret_key = generate_secret_key();
        let pubkey = get_public_key_hex(&secret_key).unwrap();

        let unsigned = UnsignedEvent {
            pubkey,
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test".to_string(),
        };

        // Compute hash multiple times
        let id1 = get_event_hash(&unsigned).unwrap();
        let id2 = get_event_hash(&unsigned).unwrap();
        let id3 = get_event_hash(&unsigned).unwrap();

        assert_eq!(id1, id2, "Event hash should be deterministic");
        assert_eq!(id2, id3, "Event hash should be deterministic");
    }
}

// =============================================================================
// Malformed Event Handling Tests
// =============================================================================

#[cfg(feature = "full")]
mod malformed_events {
    use super::*;

    #[test]
    fn test_event_with_all_fields_empty() {
        let event = Event {
            id: String::new(),
            pubkey: String::new(),
            created_at: 0,
            kind: 0,
            tags: vec![],
            content: String::new(),
            sig: String::new(),
        };

        assert!(
            !validate_event(&event),
            "Event with empty string fields should fail validation"
        );
    }

    #[test]
    fn test_event_with_whitespace_id() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test".to_string(),
        };

        let mut event = finalize_event(&template, &secret_key).unwrap();
        event.id = " ".repeat(64);

        assert!(
            !validate_event(&event),
            "Event with whitespace ID should fail"
        );
    }

    #[test]
    fn test_event_with_null_bytes_in_content() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test\0with\0nulls".to_string(),
        };

        // Should be able to create and verify event with null bytes
        let event = finalize_event(&template, &secret_key).unwrap();
        assert!(verify_event(&event).unwrap());
    }

    #[test]
    fn test_event_serialization_matches_format() {
        let secret_key = generate_secret_key();
        let pubkey = get_public_key_hex(&secret_key).unwrap();

        let unsigned = UnsignedEvent {
            pubkey: pubkey.clone(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![
                vec!["e".to_string(), "event123".to_string()],
                vec!["p".to_string(), "pubkey456".to_string()],
            ],
            content: "test content".to_string(),
        };

        let serialized = serialize_event(&unsigned).unwrap();

        // Verify it matches the expected JSON array format
        assert!(serialized.starts_with('['));
        assert!(serialized.ends_with(']'));
        assert!(serialized.contains(&pubkey));
        assert!(serialized.contains("1234567890"));
        assert!(serialized.contains("test content"));
    }
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[cfg(feature = "full")]
mod edge_cases {
    use super::*;

    #[test]
    fn test_event_with_very_long_content() {
        let secret_key = generate_secret_key();
        let content = "a".repeat(1_000_000); // 1MB of content

        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content,
        };

        let event = finalize_event(&template, &secret_key).unwrap();
        assert!(verify_event(&event).unwrap());
    }

    #[test]
    fn test_event_with_many_tags() {
        let secret_key = generate_secret_key();
        let tags: Vec<Vec<String>> = (0..1000)
            .map(|i| vec![format!("tag{}", i), format!("value{}", i)])
            .collect();

        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags,
            content: "test".to_string(),
        };

        let event = finalize_event(&template, &secret_key).unwrap();
        assert!(verify_event(&event).unwrap());
        assert_eq!(event.tags.len(), 1000);
    }

    #[test]
    fn test_event_with_deeply_nested_tag() {
        let secret_key = generate_secret_key();
        let deep_tag: Vec<String> = (0..100).map(|i| format!("element{}", i)).collect();

        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![deep_tag],
            content: "test".to_string(),
        };

        let event = finalize_event(&template, &secret_key).unwrap();
        assert!(verify_event(&event).unwrap());
        assert_eq!(event.tags[0].len(), 100);
    }

    #[test]
    fn test_two_events_same_content_different_keys() {
        let secret_key1 = generate_secret_key();
        let secret_key2 = generate_secret_key();

        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "same content".to_string(),
        };

        let event1 = finalize_event(&template, &secret_key1).unwrap();
        let event2 = finalize_event(&template, &secret_key2).unwrap();

        // Different keys should produce different IDs and signatures
        assert_ne!(event1.id, event2.id);
        assert_ne!(event1.sig, event2.sig);
        assert_ne!(event1.pubkey, event2.pubkey);

        // But both should verify
        assert!(verify_event(&event1).unwrap());
        assert!(verify_event(&event2).unwrap());
    }

    #[test]
    fn test_event_json_roundtrip_preserves_validation() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![vec!["t".to_string(), "test".to_string()]],
            content: "roundtrip test".to_string(),
        };

        let event = finalize_event(&template, &secret_key).unwrap();
        assert!(verify_event(&event).unwrap());

        // Serialize to JSON and back
        let json = serde_json::to_string(&event).unwrap();
        let event_restored: Event = serde_json::from_str(&json).unwrap();

        // Should still verify after roundtrip
        assert!(verify_event(&event_restored).unwrap());
        assert_eq!(event, event_restored);
    }
}
