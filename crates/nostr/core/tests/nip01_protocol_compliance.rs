//! Comprehensive NIP-01 protocol compliance tests
//!
//! This test suite ensures strict compliance with NIP-01 specification,
//! covering event structure, serialization, validation, and edge cases.

use nostr::{
    Event, EventTemplate, KIND_SHORT_TEXT_NOTE, KindClassification, UnsignedEvent, classify_kind,
    finalize_event, generate_secret_key, get_event_hash, get_public_key_hex, serialize_event,
    validate_event, validate_unsigned_event, verify_event,
};

// =============================================================================
// Event Structure Validation Tests
// =============================================================================

#[test]
fn test_event_id_must_be_64_hex_chars() {
    let mut event = create_valid_event();

    // Too short
    event.id = "abc".to_string();
    assert!(!validate_event(&event), "ID too short should fail");

    // Too long
    event.id = "a".repeat(65);
    assert!(!validate_event(&event), "ID too long should fail");

    // Non-hex characters
    event.id = "g".repeat(64);
    assert!(!validate_event(&event), "Non-hex ID should fail");

    // Uppercase (accepted by validate_event, but should be lowercase per NIP-01)
    // Relay-level validation enforces lowercase
    event.id = "A".repeat(64);
    assert!(
        validate_event(&event),
        "Uppercase hex ID passes basic validation"
    );

    // Valid lowercase
    event.id = "a".repeat(64);
    assert!(validate_event(&event), "Valid lowercase hex ID should pass");
}

#[test]
fn test_event_pubkey_must_be_64_lowercase_hex() {
    let mut event = create_valid_event();

    // Too short
    event.pubkey = "abc".to_string();
    assert!(!validate_event(&event), "Pubkey too short should fail");

    // Too long
    event.pubkey = "a".repeat(65);
    assert!(!validate_event(&event), "Pubkey too long should fail");

    // Non-hex
    event.pubkey = "z".repeat(64);
    assert!(!validate_event(&event), "Non-hex pubkey should fail");

    // Uppercase
    event.pubkey = "A".repeat(64);
    assert!(!validate_event(&event), "Uppercase pubkey should fail");

    // Valid
    event.pubkey = "b".repeat(64);
    assert!(
        validate_event(&event),
        "Valid lowercase hex pubkey should pass"
    );
}

#[test]
fn test_event_signature_must_be_128_hex_chars() {
    let mut event = create_valid_event();

    // Too short
    event.sig = "abc".to_string();
    assert!(!validate_event(&event), "Sig too short should fail");

    // Too long
    event.sig = "a".repeat(129);
    assert!(!validate_event(&event), "Sig too long should fail");

    // Non-hex
    event.sig = "z".repeat(128);
    assert!(!validate_event(&event), "Non-hex sig should fail");

    // Valid (both upper and lower accepted for sig)
    event.sig = "c".repeat(128);
    assert!(validate_event(&event), "Valid hex sig should pass");
}

#[test]
fn test_unsigned_event_pubkey_must_be_lowercase() {
    let mut unsigned = create_valid_unsigned_event();

    // Uppercase should fail
    unsigned.pubkey = "A".repeat(64);
    assert!(
        !validate_unsigned_event(&unsigned),
        "Uppercase pubkey should fail validation"
    );

    // Lowercase should pass
    unsigned.pubkey = "a".repeat(64);
    assert!(
        validate_unsigned_event(&unsigned),
        "Lowercase pubkey should pass"
    );
}

// =============================================================================
// Event Serialization Tests
// =============================================================================

#[test]
fn test_event_serialization_format() {
    let pubkey = "a".repeat(64);
    let unsigned = UnsignedEvent {
        pubkey: pubkey.clone(),
        created_at: 1234567890,
        kind: 1,
        tags: vec![],
        content: "test".to_string(),
    };

    let serialized = serialize_event(&unsigned).unwrap();

    // Should be: [0, pubkey, created_at, kind, tags, content]
    let expected = format!("[0,\"{}\",1234567890,1,[],\"test\"]", pubkey);
    assert_eq!(serialized, expected, "Serialization format must match spec");
}

#[test]
fn test_event_serialization_with_tags() {
    let pubkey = "a".repeat(64);
    let unsigned = UnsignedEvent {
        pubkey: pubkey,
        created_at: 1234567890,
        kind: 1,
        tags: vec![
            vec!["e".to_string(), "event123".to_string()],
            vec!["p".to_string(), "pubkey456".to_string()],
        ],
        content: "reply".to_string(),
    };

    let serialized = serialize_event(&unsigned).unwrap();

    // Tags should be serialized as nested arrays
    assert!(serialized.contains("[[\"e\",\"event123\"],[\"p\",\"pubkey456\"]]"));
}

#[test]
fn test_event_serialization_special_characters() {
    let pubkey = "a".repeat(64);

    // Test all special characters that need escaping per NIP-01
    let test_cases = vec![
        ("\n", "\\n"),       // Line break
        ("\"", "\\\""),      // Double quote
        ("\\", "\\\\"),      // Backslash
        ("\r", "\\r"),       // Carriage return
        ("\t", "\\t"),       // Tab
        ("\u{0008}", "\\b"), // Backspace
        ("\u{000C}", "\\f"), // Form feed
    ];

    for (input, expected_escape) in test_cases {
        let unsigned = UnsignedEvent {
            pubkey: pubkey.clone(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: format!("test{}content", input),
        };

        let serialized = serialize_event(&unsigned).unwrap();
        assert!(
            serialized.contains(expected_escape),
            "Failed to escape '{}' correctly",
            input.escape_debug()
        );
    }
}

#[test]
fn test_event_serialization_no_extra_whitespace() {
    let pubkey = "a".repeat(64);
    let unsigned = UnsignedEvent {
        pubkey: pubkey,
        created_at: 1234567890,
        kind: 1,
        tags: vec![vec!["t".to_string(), "nostr".to_string()]],
        content: "test".to_string(),
    };

    let serialized = serialize_event(&unsigned).unwrap();

    // Should not contain any newlines or extra spaces
    assert!(!serialized.contains('\n'), "Should not contain newlines");
    assert!(
        !serialized.contains("  "),
        "Should not contain double spaces"
    );
}

// =============================================================================
// Event Hash (ID) Tests
// =============================================================================

#[test]
fn test_event_hash_is_deterministic() {
    let pubkey = "a".repeat(64);
    let unsigned = UnsignedEvent {
        pubkey: pubkey,
        created_at: 1234567890,
        kind: 1,
        tags: vec![],
        content: "deterministic test".to_string(),
    };

    let hash1 = get_event_hash(&unsigned).unwrap();
    let hash2 = get_event_hash(&unsigned).unwrap();
    let hash3 = get_event_hash(&unsigned).unwrap();

    assert_eq!(hash1, hash2, "Same event should produce same hash");
    assert_eq!(hash2, hash3, "Hash should be deterministic");
}

#[test]
fn test_event_hash_changes_with_content() {
    let pubkey = "a".repeat(64);

    let unsigned1 = UnsignedEvent {
        pubkey: pubkey.clone(),
        created_at: 1234567890,
        kind: 1,
        tags: vec![],
        content: "content A".to_string(),
    };

    let unsigned2 = UnsignedEvent {
        pubkey: pubkey,
        created_at: 1234567890,
        kind: 1,
        tags: vec![],
        content: "content B".to_string(),
    };

    let hash1 = get_event_hash(&unsigned1).unwrap();
    let hash2 = get_event_hash(&unsigned2).unwrap();

    assert_ne!(
        hash1, hash2,
        "Different content should produce different hash"
    );
}

#[test]
fn test_event_hash_lowercase_hex() {
    let pubkey = "a".repeat(64);
    let unsigned = UnsignedEvent {
        pubkey,
        created_at: 1234567890,
        kind: 1,
        tags: vec![],
        content: "test".to_string(),
    };

    let hash = get_event_hash(&unsigned).unwrap();

    assert_eq!(hash.len(), 64, "Hash should be 64 characters");
    assert_eq!(hash, hash.to_lowercase(), "Hash should be lowercase hex");
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "Hash should be valid hex"
    );
}

// =============================================================================
// Event Signing and Verification Tests
// =============================================================================

#[test]
fn test_sign_and_verify_roundtrip() {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "Sign and verify test".to_string(),
        created_at: 1234567890,
    };

    let event = finalize_event(&template, &secret_key).unwrap();

    // Verification should succeed
    assert!(verify_event(&event).unwrap(), "Event should verify");
}

#[test]
fn test_verify_rejects_tampered_content() {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "Original content".to_string(),
        created_at: 1234567890,
    };

    let mut event = finalize_event(&template, &secret_key).unwrap();

    // Tamper with content
    event.content = "Modified content".to_string();

    // Verification should fail
    assert!(
        !verify_event(&event).unwrap(),
        "Tampered event should not verify"
    );
}

#[test]
fn test_verify_rejects_wrong_signature() {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "Test".to_string(),
        created_at: 1234567890,
    };

    let mut event = finalize_event(&template, &secret_key).unwrap();

    // Replace signature with invalid one
    event.sig = "0".repeat(128);

    // Verification should fail
    assert!(
        !verify_event(&event).unwrap(),
        "Invalid signature should not verify"
    );
}

#[test]
fn test_verify_rejects_wrong_pubkey() {
    let secret_key1 = generate_secret_key();
    let secret_key2 = generate_secret_key();

    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "Test".to_string(),
        created_at: 1234567890,
    };

    let mut event = finalize_event(&template, &secret_key1).unwrap();

    // Replace pubkey with a different one
    event.pubkey = get_public_key_hex(&secret_key2).unwrap();

    // Verification should fail
    assert!(
        !verify_event(&event).unwrap(),
        "Wrong pubkey should not verify"
    );
}

#[test]
fn test_verify_checks_id_matches_hash() {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "Test".to_string(),
        created_at: 1234567890,
    };

    let mut event = finalize_event(&template, &secret_key).unwrap();

    // Replace ID with incorrect hash
    event.id = "a".repeat(64);

    // Verification should fail
    assert!(!verify_event(&event).unwrap(), "Wrong ID should not verify");
}

// =============================================================================
// Kind Classification Tests
// =============================================================================

#[test]
fn test_regular_kinds() {
    // Regular: 1000 <= n < 10000 || 4 <= n < 45 || n == 1 || n == 2
    assert_eq!(classify_kind(1), KindClassification::Regular);
    assert_eq!(classify_kind(2), KindClassification::Regular);
    assert_eq!(classify_kind(4), KindClassification::Regular);
    assert_eq!(classify_kind(44), KindClassification::Regular);
    assert_eq!(classify_kind(1000), KindClassification::Regular);
    assert_eq!(classify_kind(5000), KindClassification::Regular);
    assert_eq!(classify_kind(9999), KindClassification::Regular);

    // Not regular
    assert_ne!(classify_kind(0), KindClassification::Regular);
    assert_ne!(classify_kind(3), KindClassification::Regular);
    assert_ne!(classify_kind(45), KindClassification::Regular);
    assert_ne!(classify_kind(10000), KindClassification::Regular);
}

#[test]
fn test_replaceable_kinds() {
    // Replaceable: 10000 <= n < 20000 || n == 0 || n == 3
    assert_eq!(classify_kind(0), KindClassification::Replaceable);
    assert_eq!(classify_kind(3), KindClassification::Replaceable);
    assert_eq!(classify_kind(10000), KindClassification::Replaceable);
    assert_eq!(classify_kind(15000), KindClassification::Replaceable);
    assert_eq!(classify_kind(19999), KindClassification::Replaceable);

    // Not replaceable
    assert_ne!(classify_kind(1), KindClassification::Replaceable);
    assert_ne!(classify_kind(20000), KindClassification::Replaceable);
}

#[test]
fn test_ephemeral_kinds() {
    // Ephemeral: 20000 <= n < 30000
    assert_eq!(classify_kind(20000), KindClassification::Ephemeral);
    assert_eq!(classify_kind(25000), KindClassification::Ephemeral);
    assert_eq!(classify_kind(29999), KindClassification::Ephemeral);

    // Not ephemeral
    assert_ne!(classify_kind(19999), KindClassification::Ephemeral);
    assert_ne!(classify_kind(30000), KindClassification::Ephemeral);
}

#[test]
fn test_addressable_kinds() {
    // Addressable: 30000 <= n < 40000
    assert_eq!(classify_kind(30000), KindClassification::Addressable);
    assert_eq!(classify_kind(35000), KindClassification::Addressable);
    assert_eq!(classify_kind(39999), KindClassification::Addressable);

    // Not addressable
    assert_ne!(classify_kind(29999), KindClassification::Addressable);
    assert_ne!(classify_kind(40000), KindClassification::Addressable);
}

#[test]
fn test_unknown_kinds() {
    assert_eq!(classify_kind(40000), KindClassification::Unknown);
    assert_eq!(classify_kind(50000), KindClassification::Unknown);
    assert_eq!(classify_kind(65535), KindClassification::Unknown);
}

// =============================================================================
// Tag Tests
// =============================================================================

#[test]
fn test_empty_tags_allowed() {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "No tags".to_string(),
        created_at: 1234567890,
    };

    let event = finalize_event(&template, &secret_key).unwrap();
    assert!(verify_event(&event).unwrap());
}

#[test]
fn test_single_element_tags() {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![vec!["t".to_string()]],
        content: "Single element tag".to_string(),
        created_at: 1234567890,
    };

    let event = finalize_event(&template, &secret_key).unwrap();
    assert!(verify_event(&event).unwrap());
}

#[test]
fn test_multi_element_tags() {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![vec![
            "e".to_string(),
            "event_id".to_string(),
            "wss://relay.example.com".to_string(),
            "pubkey".to_string(),
        ]],
        content: "Multi-element tag".to_string(),
        created_at: 1234567890,
    };

    let event = finalize_event(&template, &secret_key).unwrap();
    assert!(verify_event(&event).unwrap());
}

#[test]
fn test_tags_with_special_characters() {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![vec![
            "content-warning".to_string(),
            "This has \"quotes\" and \\ backslash".to_string(),
        ]],
        content: "Tagged content".to_string(),
        created_at: 1234567890,
    };

    let event = finalize_event(&template, &secret_key).unwrap();
    assert!(verify_event(&event).unwrap());
}

// =============================================================================
// Content Tests
// =============================================================================

#[test]
fn test_empty_content() {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "".to_string(),
        created_at: 1234567890,
    };

    let event = finalize_event(&template, &secret_key).unwrap();
    assert!(verify_event(&event).unwrap());
}

#[test]
fn test_very_long_content() {
    let secret_key = generate_secret_key();
    let long_content = "a".repeat(100000); // 100KB

    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: long_content.clone(),
        created_at: 1234567890,
    };

    let event = finalize_event(&template, &secret_key).unwrap();
    assert!(verify_event(&event).unwrap());
    assert_eq!(event.content, long_content);
}

#[test]
fn test_content_with_unicode() {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "Hello 世界 🌍 مرحبا Привет".to_string(),
        created_at: 1234567890,
    };

    let event = finalize_event(&template, &secret_key).unwrap();
    assert!(verify_event(&event).unwrap());
}

#[test]
fn test_content_with_emoji() {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "Test 🔥 🚀 ⚡ 🎉".to_string(),
        created_at: 1234567890,
    };

    let event = finalize_event(&template, &secret_key).unwrap();
    assert!(verify_event(&event).unwrap());
}

// =============================================================================
// Timestamp Tests
// =============================================================================

#[test]
fn test_zero_timestamp() {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "Zero timestamp".to_string(),
        created_at: 0,
    };

    let event = finalize_event(&template, &secret_key).unwrap();
    assert!(verify_event(&event).unwrap());
    assert_eq!(event.created_at, 0);
}

#[test]
fn test_max_timestamp() {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "Max timestamp".to_string(),
        created_at: u64::MAX,
    };

    let event = finalize_event(&template, &secret_key).unwrap();
    assert!(verify_event(&event).unwrap());
    assert_eq!(event.created_at, u64::MAX);
}

// =============================================================================
// JSON Serialization/Deserialization Tests
// =============================================================================

#[test]
fn test_event_json_roundtrip() {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![
            vec!["e".to_string(), "abc123".to_string()],
            vec!["p".to_string(), "def456".to_string()],
        ],
        content: "JSON roundtrip test".to_string(),
        created_at: 1234567890,
    };

    let event1 = finalize_event(&template, &secret_key).unwrap();

    // Serialize to JSON
    let json = serde_json::to_string(&event1).unwrap();

    // Deserialize back
    let event2: Event = serde_json::from_str(&json).unwrap();

    // Should be identical
    assert_eq!(event1, event2);
    assert!(verify_event(&event2).unwrap());
}

#[test]
fn test_event_pretty_json_roundtrip() {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: KIND_SHORT_TEXT_NOTE,
        tags: vec![],
        content: "Pretty JSON test".to_string(),
        created_at: 1234567890,
    };

    let event1 = finalize_event(&template, &secret_key).unwrap();

    // Serialize to pretty JSON (with whitespace)
    let json = serde_json::to_string_pretty(&event1).unwrap();

    // Deserialize back
    let event2: Event = serde_json::from_str(&json).unwrap();

    // Should still be identical
    assert_eq!(event1, event2);
}

// =============================================================================
// Helper Functions
// =============================================================================

fn create_valid_event() -> Event {
    Event {
        id: "a".repeat(64),
        pubkey: "b".repeat(64),
        created_at: 1234567890,
        kind: 1,
        tags: vec![],
        content: "test".to_string(),
        sig: "c".repeat(128),
    }
}

fn create_valid_unsigned_event() -> UnsignedEvent {
    UnsignedEvent {
        pubkey: "a".repeat(64),
        created_at: 1234567890,
        kind: 1,
        tags: vec![],
        content: "test".to_string(),
    }
}
