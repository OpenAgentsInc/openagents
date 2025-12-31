//! NIP-40: Expiration Timestamp
//!
//! This module implements NIP-40, which allows event creators to specify when
//! content should be considered expired and removed by relays and clients.
//!
//! ## How It Works
//!
//! Events can include an `expiration` tag with a Unix timestamp (in seconds) indicating
//! when the event should expire:
//!
//! ```json
//! {
//!   "tags": [
//!     ["expiration", "1600000000"]
//!   ]
//! }
//! ```
//!
//! ## Client Behavior
//!
//! - Clients SHOULD check relay support via `supported_nips` before sending expiration events
//! - Clients SHOULD ignore events that have expired
//! - Clients SHOULD NOT send expiration events to relays that don't support NIP-40
//!
//! ## Relay Behavior
//!
//! - Relays SHOULD NOT send expired events to clients, even if stored
//! - Relays SHOULD drop newly published events that are already expired
//! - Relays MAY NOT delete expired messages immediately and MAY persist them indefinitely
//! - Expiration timestamps do not affect storage of ephemeral events
//!
//! ## Example
//!
//! ```
//! use nostr_core::nip40::{get_expiration, is_expired, set_expiration};
//! use nostr_core::Event;
//!
//! // Check if an event has expired
//! # fn example(event: &Event) {
//! if is_expired(event, None) {
//!     println!("Event has expired!");
//! }
//!
//! // Get expiration timestamp
//! if let Some(timestamp) = get_expiration(event) {
//!     println!("Expires at: {}", timestamp);
//! }
//! # }
//! ```

use crate::nip01::Event;
use thiserror::Error;

/// The tag name used for expiration timestamps
pub const EXPIRATION_TAG: &str = "expiration";

/// Errors that can occur during NIP-40 operations.
#[derive(Debug, Error)]
pub enum Nip40Error {
    #[error("invalid expiration timestamp: {0}")]
    InvalidTimestamp(String),

    #[error("expiration timestamp is in the past")]
    ExpiredTimestamp,
}

/// Get the expiration timestamp from an event.
///
/// Returns `None` if the event has no expiration tag.
///
/// # Arguments
///
/// * `event` - The event to check
///
/// # Example
///
/// ```
/// use nostr_core::nip40::get_expiration;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if let Some(timestamp) = get_expiration(event) {
///     println!("Event expires at: {}", timestamp);
/// }
/// # }
/// ```
pub fn get_expiration(event: &Event) -> Option<i64> {
    event
        .tags
        .iter()
        .find(|tag| tag.get(0).map(|s| s.as_str()) == Some(EXPIRATION_TAG))
        .and_then(|tag| tag.get(1))
        .and_then(|ts| ts.parse::<i64>().ok())
}

/// Check if an event has expired.
///
/// # Arguments
///
/// * `event` - The event to check
/// * `current_time` - The current Unix timestamp in seconds. If `None`, uses the system time.
///
/// # Returns
///
/// Returns `true` if the event has an expiration tag and the expiration time has passed.
/// Returns `false` if the event has no expiration tag or has not yet expired.
///
/// # Example
///
/// ```
/// use nostr_core::nip40::is_expired;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if is_expired(event, None) {
///     println!("This event has expired");
/// }
/// # }
/// ```
pub fn is_expired(event: &Event, current_time: Option<i64>) -> bool {
    if let Some(expiration) = get_expiration(event) {
        let now = current_time.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
        });
        expiration <= now
    } else {
        false
    }
}

/// Set the expiration timestamp for an event template.
///
/// This is a helper function to add an expiration tag to event tags.
///
/// # Arguments
///
/// * `tags` - The mutable vector of tags to add the expiration to
/// * `timestamp` - The Unix timestamp (in seconds) when the event should expire
///
/// # Example
///
/// ```
/// use nostr_core::nip40::set_expiration;
///
/// let mut tags: Vec<Vec<String>> = vec![];
/// let expires_at = 1600000000; // Unix timestamp
/// set_expiration(&mut tags, expires_at);
/// ```
pub fn set_expiration(tags: &mut Vec<Vec<String>>, timestamp: i64) {
    // Remove any existing expiration tags
    tags.retain(|tag| tag.get(0).map(|s| s.as_str()) != Some(EXPIRATION_TAG));

    // Add the new expiration tag
    tags.push(vec![EXPIRATION_TAG.to_string(), timestamp.to_string()]);
}

/// Validate that an expiration timestamp is not in the past.
///
/// This can be used by clients before publishing an event to ensure the
/// expiration timestamp makes sense.
///
/// # Arguments
///
/// * `timestamp` - The expiration timestamp to validate
/// * `current_time` - The current Unix timestamp in seconds. If `None`, uses the system time.
///
/// # Errors
///
/// Returns `Nip40Error::ExpiredTimestamp` if the timestamp is in the past.
///
/// # Example
///
/// ```
/// use nostr_core::nip40::validate_expiration;
///
/// let future_time = 2000000000; // Some time in the future
/// assert!(validate_expiration(future_time, None).is_ok());
/// ```
pub fn validate_expiration(timestamp: i64, current_time: Option<i64>) -> Result<(), Nip40Error> {
    let now = current_time.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    });

    if timestamp <= now {
        return Err(Nip40Error::ExpiredTimestamp);
    }

    Ok(())
}

/// Check if an event has an expiration tag.
///
/// # Example
///
/// ```
/// use nostr_core::nip40::has_expiration;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if has_expiration(event) {
///     println!("This event has an expiration timestamp");
/// }
/// # }
/// ```
pub fn has_expiration(event: &Event) -> bool {
    event
        .tags
        .iter()
        .any(|tag| tag.get(0).map(|s| s.as_str()) == Some(EXPIRATION_TAG))
}

/// Get the time remaining until expiration (in seconds).
///
/// Returns `None` if the event has no expiration tag.
/// Returns a negative value if the event has already expired.
///
/// # Arguments
///
/// * `event` - The event to check
/// * `current_time` - The current Unix timestamp in seconds. If `None`, uses the system time.
///
/// # Example
///
/// ```
/// use nostr_core::nip40::time_until_expiration;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if let Some(remaining) = time_until_expiration(event, None) {
///     if remaining > 0 {
///         println!("Event expires in {} seconds", remaining);
///     } else {
///         println!("Event expired {} seconds ago", -remaining);
///     }
/// }
/// # }
/// ```
pub fn time_until_expiration(event: &Event, current_time: Option<i64>) -> Option<i64> {
    let expiration = get_expiration(event)?;
    let now = current_time.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    });
    Some(expiration - now)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(tags: Vec<Vec<String>>) -> Event {
        Event {
            id: "0".repeat(64),
            pubkey: "0".repeat(64),
            created_at: 1000000,
            kind: 1,
            tags,
            content: "test".to_string(),
            sig: "0".repeat(128),
        }
    }

    #[test]
    fn test_get_expiration_with_tag() {
        let event = create_test_event(vec![vec![
            "expiration".to_string(),
            "1600000000".to_string(),
        ]]);
        assert_eq!(get_expiration(&event), Some(1600000000));
    }

    #[test]
    fn test_get_expiration_without_tag() {
        let event = create_test_event(vec![]);
        assert_eq!(get_expiration(&event), None);
    }

    #[test]
    fn test_get_expiration_with_other_tags() {
        let event = create_test_event(vec![
            vec!["p".to_string(), "pubkey".to_string()],
            vec!["expiration".to_string(), "1600000000".to_string()],
            vec!["e".to_string(), "eventid".to_string()],
        ]);
        assert_eq!(get_expiration(&event), Some(1600000000));
    }

    #[test]
    fn test_is_expired_true() {
        let event = create_test_event(vec![vec!["expiration".to_string(), "1000000".to_string()]]);
        assert!(is_expired(&event, Some(2000000)));
    }

    #[test]
    fn test_is_expired_false() {
        let event = create_test_event(vec![vec!["expiration".to_string(), "3000000".to_string()]]);
        assert!(!is_expired(&event, Some(2000000)));
    }

    #[test]
    fn test_is_expired_no_tag() {
        let event = create_test_event(vec![]);
        assert!(!is_expired(&event, Some(2000000)));
    }

    #[test]
    fn test_is_expired_exact_time() {
        let event = create_test_event(vec![vec!["expiration".to_string(), "2000000".to_string()]]);
        // At exact expiration time, it should be considered expired
        assert!(is_expired(&event, Some(2000000)));
    }

    #[test]
    fn test_set_expiration() {
        let mut tags = vec![];
        set_expiration(&mut tags, 1600000000);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0][0], "expiration");
        assert_eq!(tags[0][1], "1600000000");
    }

    #[test]
    fn test_set_expiration_replaces_existing() {
        let mut tags = vec![
            vec!["expiration".to_string(), "1000000".to_string()],
            vec!["p".to_string(), "pubkey".to_string()],
        ];
        set_expiration(&mut tags, 2000000);

        // Should have 2 tags: the p tag and the new expiration tag
        assert_eq!(tags.len(), 2);

        // Find the expiration tag
        let expiration_tag = tags.iter().find(|t| t[0] == "expiration").unwrap();
        assert_eq!(expiration_tag[1], "2000000");

        // Verify the p tag is still there
        assert!(tags.iter().any(|t| t[0] == "p"));
    }

    #[test]
    fn test_validate_expiration_future() {
        let result = validate_expiration(3000000, Some(2000000));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_expiration_past() {
        let result = validate_expiration(1000000, Some(2000000));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_expiration_exact_time() {
        let result = validate_expiration(2000000, Some(2000000));
        assert!(result.is_err());
    }

    #[test]
    fn test_has_expiration_true() {
        let event = create_test_event(vec![vec![
            "expiration".to_string(),
            "1600000000".to_string(),
        ]]);
        assert!(has_expiration(&event));
    }

    #[test]
    fn test_has_expiration_false() {
        let event = create_test_event(vec![]);
        assert!(!has_expiration(&event));
    }

    #[test]
    fn test_time_until_expiration_future() {
        let event = create_test_event(vec![vec!["expiration".to_string(), "3000000".to_string()]]);
        let remaining = time_until_expiration(&event, Some(2000000));
        assert_eq!(remaining, Some(1000000));
    }

    #[test]
    fn test_time_until_expiration_past() {
        let event = create_test_event(vec![vec!["expiration".to_string(), "1000000".to_string()]]);
        let remaining = time_until_expiration(&event, Some(2000000));
        assert_eq!(remaining, Some(-1000000));
    }

    #[test]
    fn test_time_until_expiration_no_tag() {
        let event = create_test_event(vec![]);
        let remaining = time_until_expiration(&event, Some(2000000));
        assert_eq!(remaining, None);
    }

    #[test]
    fn test_invalid_expiration_value() {
        let event = create_test_event(vec![vec![
            "expiration".to_string(),
            "not_a_number".to_string(),
        ]]);
        assert_eq!(get_expiration(&event), None);
        assert!(!is_expired(&event, Some(2000000)));
    }
}
