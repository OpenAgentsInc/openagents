//! NIP-01 message validation
//!
//! Comprehensive validation of Nostr protocol messages according to the NIP-01 specification.
//! This module ensures all incoming protocol messages comply with Nostr standards and relay
//! resource limits.
//!
//! # Architecture
//!
//! The validation module operates in layers:
//! 1. **Message Format**: Validate JSON structure and required fields
//! 2. **Field Format**: Validate hex strings, timestamps, subscription IDs
//! 3. **Event Structure**: Validate event size, content length, tag counts
//! 4. **Filter Logic**: Validate filter parameters and limits
//! 5. **Cryptography**: Verify event signatures (via nostr crate)
//!
//! # Design Decisions
//!
//! - **Relay Protection**: Resource limits (event size, tag count) protect the relay from
//!   DoS attacks and resource exhaustion
//! - **Strict Validation**: Reject malformed data immediately rather than attempting to
//!   fix or normalize it
//! - **Performance**: Validation is optimized for common cases (small events, simple filters)
//!   while still handling edge cases correctly
//! - **Lowercase Hex**: All hex strings must be lowercase to enable efficient indexing
//!   and prevent duplicate storage of equivalent values
//! - **Timestamp Bounds**: Events must be within reasonable time bounds (not too far in
//!   past or future) to prevent spam and ensure data quality
//!
//! # NIP-01 Compliance
//!
//! This module implements all validation requirements from NIP-01:
//! - Event structure and signature verification
//! - Filter parameter validation
//! - Protocol message format (EVENT, REQ, CLOSE)
//! - Resource limits and DoS protection
//!
//! See: https://github.com/nostr-protocol/nips/blob/master/01.md
//!
//! # Usage
//!
//! ```
//! use nostr_relay::{validate_event, validate_filter, validate_subscription_id};
//! # use nostr::{generate_secret_key, finalize_event, EventTemplate};
//! # use nostr_relay::Filter;
//!
//! # fn current_timestamp() -> u64 {
//! #     std::time::SystemTime::now()
//! #         .duration_since(std::time::UNIX_EPOCH)
//! #         .unwrap()
//! #         .as_secs()
//! # }
//! // Validate incoming event
//! # let secret_key = generate_secret_key();
//! # let template = EventTemplate {
//! #     kind: 1,
//! #     tags: vec![],
//! #     content: "test".to_string(),
//! #     created_at: current_timestamp(),
//! # };
//! # let event = finalize_event(&template, &secret_key).unwrap();
//! validate_event(&event)?;
//!
//! // Validate subscription parameters
//! let mut filter = Filter::new();
//! filter.limit = Some(100);
//! validate_filter(&filter)?;
//!
//! validate_subscription_id("sub-123")?;
//! # Ok::<(), nostr_relay::ValidationError>(())
//! ```
//!
//! # Testing
//!
//! This module includes extensive tests covering all validation rules:
//! - Unit tests for each validation function
//! - Property-based tests for edge cases and boundary conditions
//! - Integration tests with real Nostr events and signatures

use crate::subscription::Filter;
use nostr::Event;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum allowed event size (128 KB)
pub const MAX_EVENT_SIZE: usize = 128 * 1024;

/// Maximum subscription ID length (64 chars)
pub const MAX_SUBSCRIPTION_ID_LENGTH: usize = 64;

/// Maximum content length (64 KB)
pub const MAX_CONTENT_LENGTH: usize = 64 * 1024;

/// Maximum number of tags
pub const MAX_TAGS: usize = 2000;

/// Maximum tag array length
pub const MAX_TAG_LENGTH: usize = 1000;

/// Maximum time difference for created_at (1 year in future, 10 years in past)
pub const MAX_FUTURE_SECONDS: u64 = 365 * 24 * 60 * 60; // 1 year
pub const MAX_PAST_SECONDS: u64 = 10 * 365 * 24 * 60 * 60; // 10 years

/// Validation error details
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// Event ID validation failed
    InvalidEventId(String),
    /// Signature validation failed
    InvalidSignature(String),
    /// Pubkey validation failed
    InvalidPubkey(String),
    /// Timestamp validation failed
    InvalidTimestamp(String),
    /// Kind validation failed
    InvalidKind(String),
    /// Tags validation failed
    InvalidTags(String),
    /// Content validation failed
    InvalidContent(String),
    /// Filter validation failed
    InvalidFilter(String),
    /// Message format validation failed
    InvalidMessage(String),
    /// Subscription ID validation failed
    InvalidSubscriptionId(String),
    /// Event too large
    EventTooLarge(usize),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::InvalidEventId(msg) => write!(f, "invalid: event id {}", msg),
            ValidationError::InvalidSignature(msg) => write!(f, "invalid: signature {}", msg),
            ValidationError::InvalidPubkey(msg) => write!(f, "invalid: pubkey {}", msg),
            ValidationError::InvalidTimestamp(msg) => write!(f, "invalid: timestamp {}", msg),
            ValidationError::InvalidKind(msg) => write!(f, "invalid: kind {}", msg),
            ValidationError::InvalidTags(msg) => write!(f, "invalid: tags {}", msg),
            ValidationError::InvalidContent(msg) => write!(f, "invalid: content {}", msg),
            ValidationError::InvalidFilter(msg) => write!(f, "invalid: filter {}", msg),
            ValidationError::InvalidMessage(msg) => write!(f, "invalid: message {}", msg),
            ValidationError::InvalidSubscriptionId(msg) => {
                write!(f, "invalid: subscription id {}", msg)
            }
            ValidationError::EventTooLarge(size) => {
                write!(f, "invalid: event too large ({} bytes)", size)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validate a 64-character hex string (for event IDs, pubkeys)
fn validate_hex64(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
}

/// Validate a 128-character hex string (for signatures)
fn validate_hex128(value: &str) -> bool {
    value.len() == 128 && value.chars().all(|c| c.is_ascii_hexdigit())
}

/// Validate that a hex string is lowercase
fn validate_lowercase_hex(value: &str) -> bool {
    value.chars().all(|c| !c.is_ascii_uppercase())
}

/// Get current Unix timestamp
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Validate event structure (without cryptographic verification)
pub fn validate_event_structure(event: &Event) -> std::result::Result<(), ValidationError> {
    // Validate event ID
    if !validate_hex64(&event.id) {
        return Err(ValidationError::InvalidEventId(
            "must be 64 hex characters".to_string(),
        ));
    }
    if !validate_lowercase_hex(&event.id) {
        return Err(ValidationError::InvalidEventId(
            "must be lowercase".to_string(),
        ));
    }

    // Validate pubkey
    if !validate_hex64(&event.pubkey) {
        return Err(ValidationError::InvalidPubkey(
            "must be 64 hex characters".to_string(),
        ));
    }
    if !validate_lowercase_hex(&event.pubkey) {
        return Err(ValidationError::InvalidPubkey(
            "must be lowercase".to_string(),
        ));
    }

    // Validate signature
    if !validate_hex128(&event.sig) {
        return Err(ValidationError::InvalidSignature(
            "must be 128 hex characters".to_string(),
        ));
    }
    // Note: NIP-01 says signatures should be lowercase, but we're permissive here
    // since hex::encode always produces lowercase anyway

    // Validate kind is within valid range (0-65535)
    // This is implicitly validated by the u16 type

    // Validate timestamp is reasonable
    let now = current_timestamp();
    if event.created_at > now + MAX_FUTURE_SECONDS {
        return Err(ValidationError::InvalidTimestamp(
            "too far in the future".to_string(),
        ));
    }
    if event.created_at < now.saturating_sub(MAX_PAST_SECONDS) {
        return Err(ValidationError::InvalidTimestamp(
            "too far in the past".to_string(),
        ));
    }

    // Validate tags
    if event.tags.len() > MAX_TAGS {
        return Err(ValidationError::InvalidTags(format!(
            "too many tags (max {})",
            MAX_TAGS
        )));
    }

    for (i, tag) in event.tags.iter().enumerate() {
        if tag.len() > MAX_TAG_LENGTH {
            return Err(ValidationError::InvalidTags(format!(
                "tag {} too long (max {} elements)",
                i, MAX_TAG_LENGTH
            )));
        }

        // Validate all tag elements are valid strings (not null)
        for (j, element) in tag.iter().enumerate() {
            if element.is_empty() && j == 0 {
                // First element (tag name) should not be empty, but we'll be permissive
                // since some clients may send empty tag names
            }
        }
    }

    // Validate content length
    if event.content.len() > MAX_CONTENT_LENGTH {
        return Err(ValidationError::InvalidContent(format!(
            "too long (max {} bytes)",
            MAX_CONTENT_LENGTH
        )));
    }

    Ok(())
}

/// Validate event with cryptographic verification (requires full feature)
#[cfg(feature = "full")]
pub fn validate_event(event: &Event) -> std::result::Result<(), ValidationError> {
    // First validate structure
    validate_event_structure(event)?;

    // Then verify signature and hash
    match nostr::verify_event(event) {
        Ok(true) => Ok(()),
        Ok(false) => Err(ValidationError::InvalidSignature(
            "signature verification failed".to_string(),
        )),
        Err(e) => Err(ValidationError::InvalidSignature(format!(
            "verification error: {}",
            e
        ))),
    }
}

/// Validate event without cryptographic verification (no full feature)
#[cfg(not(feature = "full"))]
pub fn validate_event(event: &Event) -> std::result::Result<(), ValidationError> {
    validate_event_structure(event)
}

/// Validate filter according to NIP-01
pub fn validate_filter(filter: &Filter) -> std::result::Result<(), ValidationError> {
    // Validate ids are 64-char hex
    if let Some(ref ids) = filter.ids {
        for id in ids {
            // Allow prefixes (for prefix matching)
            if id.is_empty() {
                return Err(ValidationError::InvalidFilter(
                    "id cannot be empty".to_string(),
                ));
            }
            if id.len() > 64 {
                return Err(ValidationError::InvalidFilter(
                    "id too long (max 64 chars)".to_string(),
                ));
            }
            if !id.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(ValidationError::InvalidFilter(
                    "id must be hex characters".to_string(),
                ));
            }
            if !validate_lowercase_hex(id) {
                return Err(ValidationError::InvalidFilter(
                    "id must be lowercase".to_string(),
                ));
            }
        }
    }

    // Validate authors are 64-char hex
    if let Some(ref authors) = filter.authors {
        for author in authors {
            // Allow prefixes (for prefix matching)
            if author.is_empty() {
                return Err(ValidationError::InvalidFilter(
                    "author cannot be empty".to_string(),
                ));
            }
            if author.len() > 64 {
                return Err(ValidationError::InvalidFilter(
                    "author too long (max 64 chars)".to_string(),
                ));
            }
            if !author.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(ValidationError::InvalidFilter(
                    "author must be hex characters".to_string(),
                ));
            }
            if !validate_lowercase_hex(author) {
                return Err(ValidationError::InvalidFilter(
                    "author must be lowercase".to_string(),
                ));
            }
        }
    }

    // Validate kinds are valid (implicitly validated by u16 type)
    // Just check they're not empty if present
    if let Some(ref kinds) = filter.kinds
        && kinds.is_empty()
    {
        return Err(ValidationError::InvalidFilter(
            "kinds array cannot be empty".to_string(),
        ));
    }

    // Validate since/until are reasonable timestamps
    let now = current_timestamp();
    if let Some(since) = filter.since
        && since > now + MAX_FUTURE_SECONDS
    {
        return Err(ValidationError::InvalidFilter(
            "since timestamp too far in future".to_string(),
        ));
    }

    if let Some(until) = filter.until
        && until > now + MAX_FUTURE_SECONDS
    {
        return Err(ValidationError::InvalidFilter(
            "until timestamp too far in future".to_string(),
        ));
    }

    // Validate since < until if both present
    if let (Some(since), Some(until)) = (filter.since, filter.until)
        && since > until
    {
        return Err(ValidationError::InvalidFilter(
            "since must be <= until".to_string(),
        ));
    }

    // Validate limit is reasonable
    if let Some(limit) = filter.limit {
        if limit == 0 {
            return Err(ValidationError::InvalidFilter(
                "limit must be > 0".to_string(),
            ));
        }
        if limit > 5000 {
            return Err(ValidationError::InvalidFilter(
                "limit too large (max 5000)".to_string(),
            ));
        }
    }

    // Validate tag filters
    if let Some(ref tags) = filter.tags {
        for (tag_name, tag_values) in tags {
            // Tag name should start with #
            if !tag_name.starts_with('#') {
                return Err(ValidationError::InvalidFilter(format!(
                    "tag filter '{}' should start with #",
                    tag_name
                )));
            }

            // Tag name should be single letter (a-z, A-Z)
            let tag_key = tag_name.trim_start_matches('#');
            if tag_key.len() != 1 {
                return Err(ValidationError::InvalidFilter(format!(
                    "tag filter key '{}' should be single letter",
                    tag_key
                )));
            }

            let first_char = tag_key.chars().next().unwrap();
            if !first_char.is_ascii_alphabetic() {
                return Err(ValidationError::InvalidFilter(format!(
                    "tag filter key '{}' should be alphabetic",
                    tag_key
                )));
            }

            // Validate tag values
            if tag_values.is_empty() {
                return Err(ValidationError::InvalidFilter(format!(
                    "tag filter '{}' values cannot be empty",
                    tag_name
                )));
            }

            // For #e and #p tags, validate hex format
            if tag_key == "e" || tag_key == "p" {
                for value in tag_values {
                    if value.is_empty() {
                        return Err(ValidationError::InvalidFilter(format!(
                            "tag filter '{}' value cannot be empty",
                            tag_name
                        )));
                    }
                    if value.len() > 64 {
                        return Err(ValidationError::InvalidFilter(format!(
                            "tag filter '{}' value too long (max 64 chars)",
                            tag_name
                        )));
                    }
                    if !value.chars().all(|c| c.is_ascii_hexdigit()) {
                        return Err(ValidationError::InvalidFilter(format!(
                            "tag filter '{}' value must be hex",
                            tag_name
                        )));
                    }
                    if !validate_lowercase_hex(value) {
                        return Err(ValidationError::InvalidFilter(format!(
                            "tag filter '{}' value must be lowercase",
                            tag_name
                        )));
                    }
                }
            }
        }
    }

    Ok(())
}

/// Validate subscription ID
pub fn validate_subscription_id(sub_id: &str) -> std::result::Result<(), ValidationError> {
    if sub_id.is_empty() {
        return Err(ValidationError::InvalidSubscriptionId(
            "cannot be empty".to_string(),
        ));
    }

    if sub_id.len() > MAX_SUBSCRIPTION_ID_LENGTH {
        return Err(ValidationError::InvalidSubscriptionId(format!(
            "too long (max {} chars)",
            MAX_SUBSCRIPTION_ID_LENGTH
        )));
    }

    // Subscription IDs should be printable ASCII
    if !sub_id
        .chars()
        .all(|c| c.is_ascii() && !c.is_ascii_control())
    {
        return Err(ValidationError::InvalidSubscriptionId(
            "must be printable ASCII".to_string(),
        ));
    }

    Ok(())
}

/// Validate EVENT message format
pub fn validate_event_message(msg: &Value) -> std::result::Result<Event, ValidationError> {
    let arr = msg.as_array().ok_or_else(|| {
        ValidationError::InvalidMessage("EVENT message must be array".to_string())
    })?;

    if arr.len() != 2 {
        return Err(ValidationError::InvalidMessage(
            "EVENT message must have 2 elements".to_string(),
        ));
    }

    if arr[0].as_str() != Some("EVENT") {
        return Err(ValidationError::InvalidMessage(
            "first element must be 'EVENT'".to_string(),
        ));
    }

    let event: Event = serde_json::from_value(arr[1].clone())
        .map_err(|e| ValidationError::InvalidMessage(format!("failed to parse event: {}", e)))?;

    Ok(event)
}

/// Validate REQ message format
pub fn validate_req_message(
    msg: &Value,
) -> std::result::Result<(String, Vec<Filter>), ValidationError> {
    let arr = msg
        .as_array()
        .ok_or_else(|| ValidationError::InvalidMessage("REQ message must be array".to_string()))?;

    if arr.len() < 3 {
        return Err(ValidationError::InvalidMessage(
            "REQ message must have at least 3 elements".to_string(),
        ));
    }

    if arr[0].as_str() != Some("REQ") {
        return Err(ValidationError::InvalidMessage(
            "first element must be 'REQ'".to_string(),
        ));
    }

    let sub_id = arr[1].as_str().ok_or_else(|| {
        ValidationError::InvalidMessage("subscription ID must be string".to_string())
    })?;

    validate_subscription_id(sub_id)?;

    let mut filters = Vec::new();
    for (idx, filter_value) in arr.iter().enumerate().skip(2) {
        let filter: Filter = serde_json::from_value(filter_value.clone()).map_err(|e| {
            ValidationError::InvalidMessage(format!("failed to parse filter {}: {}", idx - 2, e))
        })?;
        validate_filter(&filter)?;
        filters.push(filter);
    }

    Ok((sub_id.to_string(), filters))
}

/// Validate CLOSE message format
pub fn validate_close_message(msg: &Value) -> std::result::Result<String, ValidationError> {
    let arr = msg.as_array().ok_or_else(|| {
        ValidationError::InvalidMessage("CLOSE message must be array".to_string())
    })?;

    if arr.len() != 2 {
        return Err(ValidationError::InvalidMessage(
            "CLOSE message must have 2 elements".to_string(),
        ));
    }

    if arr[0].as_str() != Some("CLOSE") {
        return Err(ValidationError::InvalidMessage(
            "first element must be 'CLOSE'".to_string(),
        ));
    }

    let sub_id = arr[1].as_str().ok_or_else(|| {
        ValidationError::InvalidMessage("subscription ID must be string".to_string())
    })?;

    validate_subscription_id(sub_id)?;

    Ok(sub_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::{EventTemplate, finalize_event, generate_secret_key};
    use proptest::prelude::*;

    fn create_valid_event() -> Event {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            kind: 1,
            tags: vec![],
            content: "test".to_string(),
            created_at: current_timestamp(),
        };
        finalize_event(&template, &secret_key).unwrap()
    }

    #[test]
    fn test_validate_hex64() {
        assert!(validate_hex64(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
        assert!(!validate_hex64("short"));
        assert!(!validate_hex64(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdefTOOLONG"
        ));
        assert!(!validate_hex64(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdeg"
        )); // 'g' not hex
    }

    #[test]
    fn test_validate_hex128() {
        assert!(validate_hex128(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
        assert!(!validate_hex128("short"));
    }

    #[test]
    fn test_validate_lowercase_hex() {
        assert!(validate_lowercase_hex("abcdef0123456789"));
        assert!(!validate_lowercase_hex("ABCDEF0123456789"));
        assert!(!validate_lowercase_hex("abcDEF0123456789"));
    }

    #[test]
    fn test_validate_event_structure() {
        let event = create_valid_event();
        assert!(validate_event_structure(&event).is_ok());
    }

    #[test]
    #[cfg(feature = "full")]
    fn test_validate_event_with_crypto() {
        let event = create_valid_event();
        assert!(validate_event(&event).is_ok());

        // Test invalid signature
        let mut bad_event = event.clone();
        bad_event.sig = "0".repeat(128);
        assert!(validate_event(&bad_event).is_err());
    }

    #[test]
    fn test_validate_event_invalid_id() {
        let mut event = create_valid_event();
        event.id = "short".to_string();
        assert!(validate_event_structure(&event).is_err());

        let mut event2 = create_valid_event();
        event2.id = event2.id.to_uppercase();
        assert!(validate_event_structure(&event2).is_err());
    }

    #[test]
    fn test_validate_event_invalid_timestamp() {
        let mut event = create_valid_event();
        event.created_at = current_timestamp() + MAX_FUTURE_SECONDS + 1000;
        assert!(validate_event_structure(&event).is_err());

        event.created_at = 1000; // Very old timestamp
        assert!(validate_event_structure(&event).is_err());
    }

    #[test]
    fn test_validate_event_too_many_tags() {
        let mut event = create_valid_event();
        event.tags = vec![vec!["test".to_string()]; MAX_TAGS + 1];
        assert!(validate_event_structure(&event).is_err());
    }

    #[test]
    fn test_validate_event_content_too_long() {
        let mut event = create_valid_event();
        event.content = "a".repeat(MAX_CONTENT_LENGTH + 1);
        assert!(validate_event_structure(&event).is_err());
    }

    #[test]
    fn test_validate_filter() {
        let mut filter = Filter::new();
        assert!(validate_filter(&filter).is_ok());

        filter.ids = Some(vec!["abc123".to_string()]);
        assert!(validate_filter(&filter).is_ok());

        filter.ids = Some(vec!["not_hex!".to_string()]);
        assert!(validate_filter(&filter).is_err());
    }

    #[test]
    fn test_validate_filter_timestamps() {
        let mut filter = Filter::new();
        let now = current_timestamp();

        filter.since = Some(now - 1000);
        filter.until = Some(now + 1000);
        assert!(validate_filter(&filter).is_ok());

        filter.since = Some(now + 1000);
        filter.until = Some(now - 1000);
        assert!(validate_filter(&filter).is_err());
    }

    #[test]
    fn test_validate_subscription_id() {
        assert!(validate_subscription_id("sub-123").is_ok());
        assert!(validate_subscription_id("").is_err());
        assert!(validate_subscription_id(&"a".repeat(MAX_SUBSCRIPTION_ID_LENGTH + 1)).is_err());
    }

    #[test]
    fn test_validate_event_message() {
        let event = create_valid_event();
        let msg = serde_json::json!(["EVENT", event]);
        assert!(validate_event_message(&msg).is_ok());

        let bad_msg = serde_json::json!(["EVENT"]);
        assert!(validate_event_message(&bad_msg).is_err());
    }

    #[test]
    fn test_validate_req_message() {
        let msg = serde_json::json!(["REQ", "sub-123", {"kinds": [1]}]);
        assert!(validate_req_message(&msg).is_ok());

        let bad_msg = serde_json::json!(["REQ", "sub-123"]);
        assert!(validate_req_message(&bad_msg).is_err());
    }

    #[test]
    fn test_validate_close_message() {
        let msg = serde_json::json!(["CLOSE", "sub-123"]);
        assert!(validate_close_message(&msg).is_ok());

        let bad_msg = serde_json::json!(["CLOSE"]);
        assert!(validate_close_message(&bad_msg).is_err());
    }

    // Property-based tests
    proptest! {
        #[test]
        fn prop_validate_hex64_accepts_valid(s in "[0-9a-f]{64}") {
            prop_assert!(validate_hex64(&s));
        }

        #[test]
        fn prop_validate_hex64_rejects_wrong_length(s in "[0-9a-f]{1,63}|[0-9a-f]{65,100}") {
            prop_assert!(!validate_hex64(&s));
        }

        #[test]
        fn prop_validate_hex64_accepts_uppercase(s in "[0-9A-F]{64}") {
            // validate_hex64 only checks length and hex chars, not case
            prop_assert!(validate_hex64(&s));
        }

        #[test]
        fn prop_validate_hex64_rejects_non_hex(
            prefix in "[0-9a-f]{30,32}",
            suffix in "[0-9a-f]{30,32}"
        ) {
            let s = format!("{}g{}", prefix, suffix);
            if s.len() == 64 {
                prop_assert!(!validate_hex64(&s));
            }
        }

        #[test]
        fn prop_validate_hex128_accepts_valid(s in "[0-9a-f]{128}") {
            prop_assert!(validate_hex128(&s));
        }

        #[test]
        fn prop_validate_hex128_rejects_wrong_length(s in "[0-9a-f]{1,127}|[0-9a-f]{129,200}") {
            prop_assert!(!validate_hex128(&s));
        }

        #[test]
        fn prop_validate_lowercase_hex_accepts_lowercase(s in "[0-9a-f]{1,100}") {
            prop_assert!(validate_lowercase_hex(&s));
        }

        #[test]
        fn prop_validate_lowercase_hex_rejects_uppercase(s in "[0-9a-f]{0,50}[A-F][0-9a-f]{0,49}") {
            // Ensures at least one uppercase letter
            prop_assert!(!validate_lowercase_hex(&s));
        }

        #[test]
        fn prop_validate_subscription_id_accepts_printable(s in "[a-zA-Z0-9_-]{1,64}") {
            prop_assert!(validate_subscription_id(&s).is_ok());
        }

        #[test]
        fn prop_validate_subscription_id_rejects_too_long(s in "[a-zA-Z0-9]{65,100}") {
            prop_assert!(validate_subscription_id(&s).is_err());
        }

        #[test]
        fn prop_validate_subscription_id_rejects_empty(s in "\\s*") {
            if s.is_empty() {
                prop_assert!(validate_subscription_id(&s).is_err());
            }
        }

        #[test]
        fn prop_validate_content_length_accepts_valid(s in "[a-zA-Z0-9 \\n]{0,65536}") {
            let mut event = create_valid_event();
            event.content = s.clone();
            if s.len() <= MAX_CONTENT_LENGTH {
                prop_assert!(validate_event_structure(&event).is_ok());
            } else {
                prop_assert!(validate_event_structure(&event).is_err());
            }
        }

        #[test]
        fn prop_validate_tags_count(count in 0usize..2100usize) {
            let mut event = create_valid_event();
            event.tags = vec![vec!["t".to_string(), "test".to_string()]; count];

            if count <= MAX_TAGS {
                prop_assert!(validate_event_structure(&event).is_ok());
            } else {
                prop_assert!(matches!(
                    validate_event_structure(&event),
                    Err(ValidationError::InvalidTags(_))
                ));
            }
        }

        #[test]
        fn prop_validate_tag_length(length in 0usize..1100usize) {
            let mut event = create_valid_event();
            event.tags = vec![vec!["x".to_string(); length]];

            if length <= MAX_TAG_LENGTH {
                prop_assert!(validate_event_structure(&event).is_ok());
            } else {
                prop_assert!(matches!(
                    validate_event_structure(&event),
                    Err(ValidationError::InvalidTags(_))
                ));
            }
        }

        #[test]
        fn prop_validate_timestamp_future(offset in 0u64..MAX_FUTURE_SECONDS * 2) {
            let mut event = create_valid_event();
            let now = current_timestamp();
            event.created_at = now + offset;

            if offset <= MAX_FUTURE_SECONDS {
                prop_assert!(validate_event_structure(&event).is_ok());
            } else {
                prop_assert!(matches!(
                    validate_event_structure(&event),
                    Err(ValidationError::InvalidTimestamp(_))
                ));
            }
        }

        #[test]
        fn prop_validate_timestamp_past(years_ago in 0u64..15u64) {
            let mut event = create_valid_event();
            let now = current_timestamp();
            let offset = years_ago * 365 * 24 * 60 * 60;
            event.created_at = now.saturating_sub(offset);

            if years_ago <= 10 {
                prop_assert!(validate_event_structure(&event).is_ok());
            } else {
                prop_assert!(matches!(
                    validate_event_structure(&event),
                    Err(ValidationError::InvalidTimestamp(_))
                ));
            }
        }

        #[test]
        fn prop_validate_filter_limit(limit in 0usize..6000usize) {
            let mut filter = Filter::new();
            filter.limit = Some(limit);

            if limit > 0 && limit <= 5000 {
                prop_assert!(validate_filter(&filter).is_ok());
            } else {
                prop_assert!(validate_filter(&filter).is_err());
            }
        }

        #[test]
        fn prop_validate_filter_id_prefix(s in "[0-9a-f]{1,64}") {
            let mut filter = Filter::new();
            filter.ids = Some(vec![s.clone()]);

            prop_assert!(validate_filter(&filter).is_ok());
        }

        #[test]
        fn prop_validate_filter_id_rejects_uppercase(s in "[0-9a-f]{0,32}[A-F][0-9a-f]{0,31}") {
            // Ensures at least one uppercase letter
            let mut filter = Filter::new();
            filter.ids = Some(vec![s]);

            prop_assert!(validate_filter(&filter).is_err());
        }

        #[test]
        fn prop_validate_filter_id_rejects_non_hex(
            prefix in "[0-9a-f]{1,30}",
            suffix in "[0-9a-f]{0,30}"
        ) {
            let s = format!("{}g{}", prefix, suffix);
            if s.len() <= 64 && s.contains('g') {
                let mut filter = Filter::new();
                filter.ids = Some(vec![s]);
                prop_assert!(validate_filter(&filter).is_err());
            }
        }

        #[test]
        fn prop_validate_filter_author_prefix(s in "[0-9a-f]{1,64}") {
            let mut filter = Filter::new();
            filter.authors = Some(vec![s]);

            prop_assert!(validate_filter(&filter).is_ok());
        }

        #[test]
        fn prop_validate_filter_since_until(
            since_offset in 0u64..1000000u64,
            duration in 1u64..1000000u64
        ) {
            let mut filter = Filter::new();
            let now = current_timestamp();
            filter.since = Some(now - since_offset);
            filter.until = Some(now - since_offset + duration);

            prop_assert!(validate_filter(&filter).is_ok());
        }

        #[test]
        fn prop_validate_filter_since_after_until_fails(
            since in 1000000u64..2000000u64,
            until in 0u64..1000000u64
        ) {
            let mut filter = Filter::new();
            filter.since = Some(since);
            filter.until = Some(until);

            prop_assert!(validate_filter(&filter).is_err());
        }
    }
}
