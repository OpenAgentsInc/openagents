//! Property-based tests for NIP-01 EventTemplate edge cases
//!
//! These tests use proptest to verify that event creation and validation
//! handle invalid inputs correctly: oversized content, extreme timestamps,
//! malformed tags, and boundary conditions.

#[cfg(feature = "full")]
use crate::nip01::{
    EventTemplate, UnsignedEvent, finalize_event, generate_secret_key, validate_unsigned_event,
};
use proptest::prelude::*;

// =============================================================================
// EventTemplate Edge Case Tests
// =============================================================================

#[cfg(feature = "full")]
proptest! {
    /// Property: EventTemplate with extremely large content still produces valid events
    #[test]
    fn prop_huge_content_creates_valid_event(content_size in 0usize..100_000usize) {
        let secret_key = generate_secret_key();
        let content = "a".repeat(content_size);

        let template = EventTemplate {
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content,
        };

        // Should successfully create an event regardless of content size
        let result = finalize_event(&template, &secret_key);
        prop_assert!(result.is_ok());
    }

    /// Property: EventTemplate with timestamp 0 is valid
    #[test]
    fn prop_zero_timestamp_valid(kind in any::<u16>()) {
        let secret_key = generate_secret_key();

        let template = EventTemplate {
            created_at: 0, // Unix epoch
            kind,
            tags: vec![],
            content: "test".to_string(),
        };

        let result = finalize_event(&template, &secret_key);
        prop_assert!(result.is_ok());
    }

    /// Property: EventTemplate with far future timestamp is valid
    #[test]
    fn prop_future_timestamp_valid(timestamp in 1234567890u64..u64::MAX) {
        let secret_key = generate_secret_key();

        let template = EventTemplate {
            created_at: timestamp,
            kind: 1,
            tags: vec![],
            content: "test".to_string(),
        };

        let result = finalize_event(&template, &secret_key);
        prop_assert!(result.is_ok());
    }

    /// Property: EventTemplate with empty content is valid
    #[test]
    fn prop_empty_content_valid(kind in any::<u16>()) {
        let secret_key = generate_secret_key();

        let template = EventTemplate {
            created_at: 1234567890,
            kind,
            tags: vec![],
            content: String::new(),
        };

        let result = finalize_event(&template, &secret_key);
        prop_assert!(result.is_ok());
    }

    /// Property: EventTemplate with many empty tags is valid
    #[test]
    fn prop_many_empty_tags_valid(num_tags in 0usize..1000usize) {
        let secret_key = generate_secret_key();

        let template = EventTemplate {
            created_at: 1234567890,
            kind: 1,
            tags: vec![vec![]; num_tags],
            content: "test".to_string(),
        };

        let result = finalize_event(&template, &secret_key);
        prop_assert!(result.is_ok());
    }

    /// Property: EventTemplate with huge number of tags is valid
    #[test]
    fn prop_many_tags_valid(num_tags in 0usize..100usize) {
        let secret_key = generate_secret_key();

        let tags: Vec<Vec<String>> = (0..num_tags)
            .map(|i| vec![format!("tag{}", i), format!("value{}", i)])
            .collect();

        let template = EventTemplate {
            created_at: 1234567890,
            kind: 1,
            tags,
            content: "test".to_string(),
        };

        let result = finalize_event(&template, &secret_key);
        prop_assert!(result.is_ok());
    }

    /// Property: EventTemplate with nested array tags works
    #[test]
    fn prop_nested_array_tags_valid(depth in 1usize..20usize) {
        let secret_key = generate_secret_key();

        let tags: Vec<Vec<String>> = vec![
            (0..depth).map(|i| format!("element{}", i)).collect()
        ];

        let template = EventTemplate {
            created_at: 1234567890,
            kind: 1,
            tags,
            content: "test".to_string(),
        };

        let result = finalize_event(&template, &secret_key);
        prop_assert!(result.is_ok());
    }

    /// Property: EventTemplate with all u16 kinds is valid
    #[test]
    fn prop_all_kinds_valid(kind in any::<u16>()) {
        let secret_key = generate_secret_key();

        let template = EventTemplate {
            created_at: 1234567890,
            kind,
            tags: vec![],
            content: "test".to_string(),
        };

        let result = finalize_event(&template, &secret_key);
        prop_assert!(result.is_ok());
    }

    /// Property: EventTemplate with unicode content is valid
    #[test]
    fn prop_unicode_content_valid(content in "\\PC{1,100}") {
        let secret_key = generate_secret_key();

        let template = EventTemplate {
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content,
        };

        let result = finalize_event(&template, &secret_key);
        prop_assert!(result.is_ok());
    }

    /// Property: EventTemplate with special characters in content is valid
    #[test]
    fn prop_special_chars_content_valid(content in "[\\x00-\\x7F]{0,100}") {
        let secret_key = generate_secret_key();

        let template = EventTemplate {
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content,
        };

        let result = finalize_event(&template, &secret_key);
        prop_assert!(result.is_ok());
    }
}

// =============================================================================
// UnsignedEvent Validation Edge Cases
// =============================================================================

proptest! {
    /// Property: UnsignedEvent with non-hex characters in pubkey is rejected
    #[test]
    fn prop_non_hex_pubkey_rejected(invalid_char in "[g-z]") {
        let mut pubkey = "a".repeat(63);
        pubkey.push_str(&invalid_char);

        let event = UnsignedEvent {
            pubkey,
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "test".to_string(),
        };

        prop_assert!(!validate_unsigned_event(&event));
    }

    /// Property: UnsignedEvent with pubkey length != 64 is rejected
    #[test]
    fn prop_wrong_length_pubkey_rejected(len in 0usize..200usize) {
        prop_assume!(len != 64);

        let pubkey = "a".repeat(len);
        let event = UnsignedEvent {
            pubkey,
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "test".to_string(),
        };

        prop_assert!(!validate_unsigned_event(&event));
    }

    /// Property: UnsignedEvent with uppercase hex pubkey is rejected
    #[test]
    fn prop_uppercase_hex_pubkey_rejected(uppercase_count in 1usize..64usize) {
        let mut pubkey = "a".repeat(64);
        // Replace some characters with uppercase
        for i in 0..uppercase_count {
            pubkey.replace_range(i..i+1, "A");
        }

        let event = UnsignedEvent {
            pubkey,
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "test".to_string(),
        };

        prop_assert!(!validate_unsigned_event(&event));
    }

    /// Property: UnsignedEvent with valid lowercase hex pubkey is accepted
    #[test]
    fn prop_valid_lowercase_hex_pubkey_accepted(hex_chars in "[0-9a-f]{64}") {
        let event = UnsignedEvent {
            pubkey: hex_chars,
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "test".to_string(),
        };

        prop_assert!(validate_unsigned_event(&event));
    }

    /// Property: UnsignedEvent validation doesn't depend on content
    #[test]
    fn prop_validation_independent_of_content(content in ".*") {
        let event = UnsignedEvent {
            pubkey: "a".repeat(64),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content,
        };

        // Should be valid regardless of content
        prop_assert!(validate_unsigned_event(&event));
    }

    /// Property: UnsignedEvent validation doesn't depend on timestamp
    #[test]
    fn prop_validation_independent_of_timestamp(timestamp in any::<u64>()) {
        let event = UnsignedEvent {
            pubkey: "a".repeat(64),
            created_at: timestamp,
            kind: 1,
            tags: vec![],
            content: "test".to_string(),
        };

        // Should be valid regardless of timestamp
        prop_assert!(validate_unsigned_event(&event));
    }

    /// Property: UnsignedEvent validation doesn't depend on kind
    #[test]
    fn prop_validation_independent_of_kind(kind in any::<u16>()) {
        let event = UnsignedEvent {
            pubkey: "a".repeat(64),
            created_at: 1234567890,
            kind,
            tags: vec![],
            content: "test".to_string(),
        };

        // Should be valid regardless of kind
        prop_assert!(validate_unsigned_event(&event));
    }

    /// Property: UnsignedEvent validation doesn't depend on tags
    #[test]
    fn prop_validation_independent_of_tags(num_tags in 0usize..50usize) {
        let tags: Vec<Vec<String>> = (0..num_tags)
            .map(|i| vec![format!("tag{}", i)])
            .collect();

        let event = UnsignedEvent {
            pubkey: "a".repeat(64),
            created_at: 1234567890,
            kind: 1,
            tags,
            content: "test".to_string(),
        };

        // Should be valid regardless of tags
        prop_assert!(validate_unsigned_event(&event));
    }
}

// =============================================================================
// Boundary Value Tests
// =============================================================================

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[cfg(feature = "full")]
    #[test]
    fn test_max_u64_timestamp() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: u64::MAX,
            kind: 1,
            tags: vec![],
            content: "test".to_string(),
        };

        assert!(finalize_event(&template, &secret_key).is_ok());
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_max_u16_kind() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: u16::MAX,
            tags: vec![],
            content: "test".to_string(),
        };

        assert!(finalize_event(&template, &secret_key).is_ok());
    }

    #[test]
    fn test_empty_pubkey_rejected() {
        let event = UnsignedEvent {
            pubkey: String::new(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "test".to_string(),
        };

        assert!(!validate_unsigned_event(&event));
    }

    #[test]
    fn test_63_char_pubkey_rejected() {
        let event = UnsignedEvent {
            pubkey: "a".repeat(63),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "test".to_string(),
        };

        assert!(!validate_unsigned_event(&event));
    }

    #[test]
    fn test_65_char_pubkey_rejected() {
        let event = UnsignedEvent {
            pubkey: "a".repeat(65),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "test".to_string(),
        };

        assert!(!validate_unsigned_event(&event));
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_content_with_newlines() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "line1\nline2\nline3".to_string(),
        };

        assert!(finalize_event(&template, &secret_key).is_ok());
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_content_with_emoji() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "Hello 🌍 World 🚀".to_string(),
        };

        assert!(finalize_event(&template, &secret_key).is_ok());
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_tags_with_empty_strings() {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            created_at: 1234567890,
            kind: 1,
            tags: vec![vec!["".to_string()], vec!["".to_string(), "".to_string()]],
            content: "test".to_string(),
        };

        assert!(finalize_event(&template, &secret_key).is_ok());
    }

    #[test]
    fn test_mixed_case_pubkey_rejected() {
        let event = UnsignedEvent {
            pubkey: "AaBbCcDdEeFf00112233445566778899AaBbCcDdEeFf00112233445566778899".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "test".to_string(),
        };

        assert!(!validate_unsigned_event(&event));
    }
}
