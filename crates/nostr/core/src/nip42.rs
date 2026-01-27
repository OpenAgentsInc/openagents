//! NIP-42: Authentication of Clients to Relays
//!
//! This module implements NIP-42, which defines a mechanism for clients to authenticate
//! to relays using signed ephemeral events. This enables relays to restrict access to
//! resources based on user identity.
//!
//! ## How It Works
//!
//! 1. Relay sends an AUTH message with a challenge string
//! 2. Client creates a signed authentication event (kind 22242)
//! 3. Client sends the signed event back in an AUTH message
//! 4. Relay validates the event and authenticates the client
//!
//! ## AUTH Message Format
//!
//! **Relay to Client:**
//! ```json
//! ["AUTH", "<challenge-string>"]
//! ```
//!
//! **Client to Relay:**
//! ```json
//! ["AUTH", <signed-event-json>]
//! ```
//!
//! ## Authentication Event (Kind 22242)
//!
//! The authentication event must include:
//! - `kind`: 22242
//! - `created_at`: Current timestamp (within ~10 minutes of relay's time)
//! - Tags:
//!   - `["relay", "<relay-url>"]` - The relay's WebSocket URL
//!   - `["challenge", "<challenge-string>"]` - The challenge from the relay
//!
//! ## Use Cases
//!
//! - Restrict access to private messages (kind 4 DMs)
//! - Limit subscriptions to paying users
//! - Implement whitelist-based access control
//! - Rate limiting based on authenticated identity
//!
//! # Example
//!
//! ```
//! use nostr_core::nip42::{create_auth_event_tags, validate_auth_event, AUTH_KIND};
//! use nostr_core::Event;
//!
//! # fn example(event: &Event, relay_url: &str, challenge: &str) {
//! // Validate an authentication event
//! match validate_auth_event(event, relay_url, challenge, None) {
//!     Ok(()) => println!("Valid authentication"),
//!     Err(e) => println!("Invalid: {}", e),
//! }
//! # }
//! ```

use crate::nip01::{Event, EventTemplate};
use thiserror::Error;

/// Event kind for authentication events
pub const AUTH_KIND: u16 = 22242;

/// Tag name for relay URL
pub const RELAY_TAG: &str = "relay";

/// Tag name for challenge string
pub const CHALLENGE_TAG: &str = "challenge";

/// Error prefix for when authentication is required but not provided
pub const AUTH_REQUIRED_PREFIX: &str = "auth-required";

/// Error prefix for when authentication is provided but insufficient
pub const RESTRICTED_PREFIX: &str = "restricted";

/// Maximum acceptable time difference for authentication events (10 minutes in seconds)
pub const MAX_TIME_DIFF: u64 = 600;

/// Errors that can occur during NIP-42 operations.
#[derive(Debug, Error)]
pub enum Nip42Error {
    #[error("invalid authentication event: wrong kind (expected 22242, got {0})")]
    InvalidKind(u16),

    #[error("missing required tag: {0}")]
    MissingTag(String),

    #[error("timestamp out of acceptable range (must be within ~10 minutes)")]
    InvalidTimestamp,

    #[error("challenge mismatch: expected {expected}, got {actual}")]
    ChallengeMismatch { expected: String, actual: String },

    #[error("relay URL mismatch: expected {expected}, got {actual}")]
    RelayMismatch { expected: String, actual: String },

    #[error("invalid AUTH message format")]
    InvalidAuthMessage,
}

/// Check if an event is an authentication event (kind 22242).
///
/// # Example
///
/// ```
/// use nostr_core::nip42::is_auth_event;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if is_auth_event(event) {
///     println!("This is an authentication event");
/// }
/// # }
/// ```
pub fn is_auth_event(event: &Event) -> bool {
    event.kind == AUTH_KIND
}

/// Get the relay URL from an authentication event.
///
/// Returns `None` if the relay tag is not present.
///
/// # Example
///
/// ```
/// use nostr_core::nip42::get_relay_url;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if let Some(relay_url) = get_relay_url(event) {
///     println!("Relay: {}", relay_url);
/// }
/// # }
/// ```
pub fn get_relay_url(event: &Event) -> Option<String> {
    event
        .tags
        .iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(RELAY_TAG))
        .and_then(|tag| tag.get(1))
        .map(|s| s.to_string())
}

/// Get the challenge string from an authentication event.
///
/// Returns `None` if the challenge tag is not present.
///
/// # Example
///
/// ```
/// use nostr_core::nip42::get_challenge;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if let Some(challenge) = get_challenge(event) {
///     println!("Challenge: {}", challenge);
/// }
/// # }
/// ```
pub fn get_challenge(event: &Event) -> Option<String> {
    event
        .tags
        .iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(CHALLENGE_TAG))
        .and_then(|tag| tag.get(1))
        .map(|s| s.to_string())
}

/// Create tags for an authentication event.
///
/// # Arguments
///
/// * `relay_url` - The relay's WebSocket URL
/// * `challenge` - The challenge string received from the relay
///
/// # Example
///
/// ```
/// use nostr_core::nip42::create_auth_event_tags;
///
/// let tags = create_auth_event_tags(
///     "wss://relay.example.com/",
///     "random-challenge-string"
/// );
///
/// assert_eq!(tags.len(), 2);
/// assert_eq!(tags[0], vec!["relay", "wss://relay.example.com/"]);
/// assert_eq!(tags[1], vec!["challenge", "random-challenge-string"]);
/// ```
pub fn create_auth_event_tags(relay_url: &str, challenge: &str) -> Vec<Vec<String>> {
    vec![
        vec![RELAY_TAG.to_string(), relay_url.to_string()],
        vec![CHALLENGE_TAG.to_string(), challenge.to_string()],
    ]
}

/// Create an authentication event template.
///
/// This creates an `EventTemplate` that can be signed with `finalize_event`
/// to create a valid NIP-42 authentication event.
///
/// # Arguments
///
/// * `relay_url` - The relay's WebSocket URL
/// * `challenge` - The challenge string received from the relay
///
/// # Example
///
/// ```
/// use nostr_core::nip42::create_auth_event_template;
/// use nostr_core::nip01::finalize_event;
///
/// let template = create_auth_event_template(
///     "wss://relay.example.com/",
///     "random-challenge-string"
/// );
///
/// // Sign with private key to create the event
/// // let event = finalize_event(&template, &private_key)?;
/// ```
pub fn create_auth_event_template(relay_url: &str, challenge: &str) -> EventTemplate {
    EventTemplate {
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        kind: AUTH_KIND,
        tags: create_auth_event_tags(relay_url, challenge),
        content: String::new(),
    }
}

/// Validate an authentication event.
///
/// Checks that:
/// - Event kind is 22242
/// - Required tags (relay, challenge) are present
/// - Challenge matches expected value
/// - Relay URL matches expected value
/// - Timestamp is within acceptable range (~10 minutes)
///
/// # Arguments
///
/// * `event` - The authentication event to validate
/// * `expected_relay_url` - The expected relay URL
/// * `expected_challenge` - The expected challenge string
/// * `current_time` - Current Unix timestamp (if None, uses system time)
///
/// # Errors
///
/// Returns an error if any validation check fails.
///
/// # Example
///
/// ```
/// use nostr_core::nip42::validate_auth_event;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// match validate_auth_event(
///     event,
///     "wss://relay.example.com/",
///     "challenge-string",
///     None
/// ) {
///     Ok(()) => println!("Valid authentication event"),
///     Err(e) => println!("Invalid: {}", e),
/// }
/// # }
/// ```
pub fn validate_auth_event(
    event: &Event,
    expected_relay_url: &str,
    expected_challenge: &str,
    current_time: Option<u64>,
) -> Result<(), Nip42Error> {
    // Check kind
    if event.kind != AUTH_KIND {
        return Err(Nip42Error::InvalidKind(event.kind));
    }

    // Check relay tag
    let relay_url = get_relay_url(event)
        .ok_or_else(|| Nip42Error::MissingTag(format!("{} tag is required", RELAY_TAG)))?;

    // Normalize URLs for comparison (remove trailing slashes)
    let expected_normalized = expected_relay_url.trim_end_matches('/');
    let actual_normalized = relay_url.trim_end_matches('/');

    if actual_normalized != expected_normalized {
        return Err(Nip42Error::RelayMismatch {
            expected: expected_relay_url.to_string(),
            actual: relay_url,
        });
    }

    // Check challenge tag
    let challenge = get_challenge(event)
        .ok_or_else(|| Nip42Error::MissingTag(format!("{} tag is required", CHALLENGE_TAG)))?;

    if challenge != expected_challenge {
        return Err(Nip42Error::ChallengeMismatch {
            expected: expected_challenge.to_string(),
            actual: challenge,
        });
    }

    // Check timestamp (must be within ~10 minutes)
    let now = current_time.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });

    let time_diff = event.created_at.abs_diff(now);

    if time_diff > MAX_TIME_DIFF {
        return Err(Nip42Error::InvalidTimestamp);
    }

    Ok(())
}

/// Normalize a relay URL for comparison.
///
/// Removes trailing slashes and converts to lowercase for consistent comparison.
///
/// # Example
///
/// ```
/// use nostr_core::nip42::normalize_relay_url;
///
/// assert_eq!(
///     normalize_relay_url("wss://relay.example.com/"),
///     "wss://relay.example.com"
/// );
/// assert_eq!(
///     normalize_relay_url("WSS://RELAY.EXAMPLE.COM"),
///     "wss://relay.example.com"
/// );
/// ```
pub fn normalize_relay_url(url: &str) -> String {
    url.trim_end_matches('/').to_lowercase()
}

/// Create an auth-required error message.
///
/// # Example
///
/// ```
/// use nostr_core::nip42::create_auth_required_message;
///
/// let msg = create_auth_required_message("subscription requires authentication");
/// assert_eq!(msg, "auth-required: subscription requires authentication");
/// ```
pub fn create_auth_required_message(reason: &str) -> String {
    format!("{}: {}", AUTH_REQUIRED_PREFIX, reason)
}

/// Create a restricted error message.
///
/// # Example
///
/// ```
/// use nostr_core::nip42::create_restricted_message;
///
/// let msg = create_restricted_message("insufficient permissions");
/// assert_eq!(msg, "restricted: insufficient permissions");
/// ```
pub fn create_restricted_message(reason: &str) -> String {
    format!("{}: {}", RESTRICTED_PREFIX, reason)
}

/// Check if an error message indicates authentication is required.
///
/// # Example
///
/// ```
/// use nostr_core::nip42::is_auth_required_error;
///
/// assert!(is_auth_required_error("auth-required: please authenticate"));
/// assert!(!is_auth_required_error("restricted: insufficient permissions"));
/// ```
pub fn is_auth_required_error(message: &str) -> bool {
    message.starts_with(AUTH_REQUIRED_PREFIX)
}

/// Check if an error message indicates restricted access.
///
/// # Example
///
/// ```
/// use nostr_core::nip42::is_restricted_error;
///
/// assert!(is_restricted_error("restricted: insufficient permissions"));
/// assert!(!is_restricted_error("auth-required: please authenticate"));
/// ```
pub fn is_restricted_error(message: &str) -> bool {
    message.starts_with(RESTRICTED_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(kind: u16, tags: Vec<Vec<String>>, created_at: u64) -> Event {
        Event {
            id: "0".repeat(64),
            pubkey: "0".repeat(64),
            created_at,
            kind,
            tags,
            content: String::new(),
            sig: "0".repeat(128),
        }
    }

    fn current_time() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    #[test]
    fn test_is_auth_event() {
        let event = create_test_event(22242, vec![], current_time());
        assert!(is_auth_event(&event));

        let event = create_test_event(1, vec![], current_time());
        assert!(!is_auth_event(&event));
    }

    #[test]
    fn test_get_relay_url() {
        let event = create_test_event(
            22242,
            vec![vec![
                "relay".to_string(),
                "wss://relay.example.com/".to_string(),
            ]],
            current_time(),
        );
        assert_eq!(
            get_relay_url(&event),
            Some("wss://relay.example.com/".to_string())
        );
    }

    #[test]
    fn test_get_relay_url_none() {
        let event = create_test_event(22242, vec![], current_time());
        assert_eq!(get_relay_url(&event), None);
    }

    #[test]
    fn test_get_challenge() {
        let event = create_test_event(
            22242,
            vec![vec!["challenge".to_string(), "test-challenge".to_string()]],
            current_time(),
        );
        assert_eq!(get_challenge(&event), Some("test-challenge".to_string()));
    }

    #[test]
    fn test_get_challenge_none() {
        let event = create_test_event(22242, vec![], current_time());
        assert_eq!(get_challenge(&event), None);
    }

    #[test]
    fn test_create_auth_event_tags() {
        let tags = create_auth_event_tags("wss://relay.example.com/", "challenge-123");

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0], vec!["relay", "wss://relay.example.com/"]);
        assert_eq!(tags[1], vec!["challenge", "challenge-123"]);
    }

    #[test]
    fn test_validate_auth_event_valid() {
        let now = current_time();
        let tags = create_auth_event_tags("wss://relay.example.com/", "challenge-123");
        let event = create_test_event(22242, tags, now);

        let result = validate_auth_event(
            &event,
            "wss://relay.example.com/",
            "challenge-123",
            Some(now),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_auth_event_wrong_kind() {
        let now = current_time();
        let tags = create_auth_event_tags("wss://relay.example.com/", "challenge-123");
        let event = create_test_event(1, tags, now);

        let result = validate_auth_event(
            &event,
            "wss://relay.example.com/",
            "challenge-123",
            Some(now),
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            Nip42Error::InvalidKind(kind) => assert_eq!(kind, 1),
            _ => panic!("Expected InvalidKind error"),
        }
    }

    #[test]
    fn test_validate_auth_event_missing_relay_tag() {
        let now = current_time();
        let event = create_test_event(
            22242,
            vec![vec!["challenge".to_string(), "challenge-123".to_string()]],
            now,
        );

        let result = validate_auth_event(
            &event,
            "wss://relay.example.com/",
            "challenge-123",
            Some(now),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_auth_event_missing_challenge_tag() {
        let now = current_time();
        let event = create_test_event(
            22242,
            vec![vec![
                "relay".to_string(),
                "wss://relay.example.com/".to_string(),
            ]],
            now,
        );

        let result = validate_auth_event(
            &event,
            "wss://relay.example.com/",
            "challenge-123",
            Some(now),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_auth_event_challenge_mismatch() {
        let now = current_time();
        let tags = create_auth_event_tags("wss://relay.example.com/", "wrong-challenge");
        let event = create_test_event(22242, tags, now);

        let result = validate_auth_event(
            &event,
            "wss://relay.example.com/",
            "challenge-123",
            Some(now),
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            Nip42Error::ChallengeMismatch { expected, actual } => {
                assert_eq!(expected, "challenge-123");
                assert_eq!(actual, "wrong-challenge");
            }
            _ => panic!("Expected ChallengeMismatch error"),
        }
    }

    #[test]
    fn test_validate_auth_event_relay_mismatch() {
        let now = current_time();
        let tags = create_auth_event_tags("wss://wrong-relay.com/", "challenge-123");
        let event = create_test_event(22242, tags, now);

        let result = validate_auth_event(
            &event,
            "wss://relay.example.com/",
            "challenge-123",
            Some(now),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_auth_event_url_normalization() {
        let now = current_time();
        // Event has URL with trailing slash
        let tags = create_auth_event_tags("wss://relay.example.com/", "challenge-123");
        let event = create_test_event(22242, tags, now);

        // Expected URL without trailing slash should still match
        let result = validate_auth_event(
            &event,
            "wss://relay.example.com",
            "challenge-123",
            Some(now),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_auth_event_timestamp_too_old() {
        let now = current_time();
        let old_time = now - 700; // More than 10 minutes old
        let tags = create_auth_event_tags("wss://relay.example.com/", "challenge-123");
        let event = create_test_event(22242, tags, old_time);

        let result = validate_auth_event(
            &event,
            "wss://relay.example.com/",
            "challenge-123",
            Some(now),
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            Nip42Error::InvalidTimestamp => {}
            _ => panic!("Expected InvalidTimestamp error"),
        }
    }

    #[test]
    fn test_validate_auth_event_timestamp_too_future() {
        let now = current_time();
        let future_time = now + 700; // More than 10 minutes in future
        let tags = create_auth_event_tags("wss://relay.example.com/", "challenge-123");
        let event = create_test_event(22242, tags, future_time);

        let result = validate_auth_event(
            &event,
            "wss://relay.example.com/",
            "challenge-123",
            Some(now),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_auth_event_timestamp_within_range() {
        let now = current_time();
        let acceptable_time = now - 300; // 5 minutes old, within acceptable range
        let tags = create_auth_event_tags("wss://relay.example.com/", "challenge-123");
        let event = create_test_event(22242, tags, acceptable_time);

        let result = validate_auth_event(
            &event,
            "wss://relay.example.com/",
            "challenge-123",
            Some(now),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_normalize_relay_url() {
        assert_eq!(
            normalize_relay_url("wss://relay.example.com/"),
            "wss://relay.example.com"
        );
        assert_eq!(
            normalize_relay_url("wss://relay.example.com"),
            "wss://relay.example.com"
        );
        assert_eq!(
            normalize_relay_url("WSS://RELAY.EXAMPLE.COM/"),
            "wss://relay.example.com"
        );
    }

    #[test]
    fn test_create_auth_required_message() {
        let msg = create_auth_required_message("subscription requires authentication");
        assert_eq!(msg, "auth-required: subscription requires authentication");
    }

    #[test]
    fn test_create_restricted_message() {
        let msg = create_restricted_message("insufficient permissions");
        assert_eq!(msg, "restricted: insufficient permissions");
    }

    #[test]
    fn test_is_auth_required_error() {
        assert!(is_auth_required_error("auth-required: please authenticate"));
        assert!(is_auth_required_error("auth-required: "));
        assert!(!is_auth_required_error(
            "restricted: insufficient permissions"
        ));
        assert!(!is_auth_required_error("other error"));
    }

    #[test]
    fn test_is_restricted_error() {
        assert!(is_restricted_error("restricted: insufficient permissions"));
        assert!(is_restricted_error("restricted: "));
        assert!(!is_restricted_error("auth-required: please authenticate"));
        assert!(!is_restricted_error("other error"));
    }
}
