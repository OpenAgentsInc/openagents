//! NIP-09: Event Deletion Request
//!
//! This module implements NIP-09, which defines deletion requests for Nostr events.
//! A deletion request is a special event with kind 5 that references one or more events
//! that the author wants to be deleted.
//!
//! ## How It Works
//!
//! A deletion request event includes:
//! - `e` tags: References to specific event IDs to delete
//! - `a` tags: References to addressable/replaceable events to delete
//! - `k` tags: Optional kind indicators for the events being deleted
//! - Content: Optional text explaining the reason for deletion
//!
//! ## Example
//!
//! ```json
//! {
//!   "kind": 5,
//!   "pubkey": "author-pubkey",
//!   "tags": [
//!     ["e", "event-id-1"],
//!     ["e", "event-id-2"],
//!     ["k", "1"]
//!   ],
//!   "content": "These events were posted by mistake"
//! }
//! ```
//!
//! ## Relay Behavior
//!
//! - Relays SHOULD delete or stop publishing referenced events with matching pubkey
//! - Relays SHOULD continue publishing deletion requests indefinitely
//! - For `a` tags, relays should delete all versions up to the deletion request timestamp
//!
//! ## Client Behavior
//!
//! - Clients MUST validate that event pubkeys match before hiding/deleting
//! - Clients SHOULD hide or indicate deletion status for referenced events
//! - Clients MAY show events with deletion indicators or fully hide them
//!
//! # Example
//!
//! ```
//! use nostr_core::nip09::{is_deletion_request, get_deleted_event_ids};
//! use nostr_core::Event;
//!
//! # fn example(event: &Event) {
//! if is_deletion_request(event) {
//!     let deleted_ids = get_deleted_event_ids(event);
//!     println!("Deletion request for {} events", deleted_ids.len());
//! }
//! # }
//! ```

use crate::nip01::Event;
use thiserror::Error;

/// Event kind for deletion requests
pub const DELETION_REQUEST_KIND: u16 = 5;

/// Errors that can occur during NIP-09 operations.
#[derive(Debug, Error)]
pub enum Nip09Error {
    #[error("invalid deletion request: {0}")]
    InvalidDeletionRequest(String),

    #[error("pubkey mismatch: deletion request pubkey does not match event pubkey")]
    PubkeyMismatch,
}

/// Check if an event is a deletion request (kind 5).
///
/// # Example
///
/// ```
/// use nostr_core::nip09::is_deletion_request;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if is_deletion_request(event) {
///     println!("This is a deletion request");
/// }
/// # }
/// ```
pub fn is_deletion_request(event: &Event) -> bool {
    event.kind == DELETION_REQUEST_KIND
}

/// Get the list of event IDs that a deletion request wants to delete.
///
/// Returns all event IDs referenced in `e` tags.
///
/// # Example
///
/// ```
/// use nostr_core::nip09::get_deleted_event_ids;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// let event_ids = get_deleted_event_ids(event);
/// for id in event_ids {
///     println!("Requesting deletion of event: {}", id);
/// }
/// # }
/// ```
pub fn get_deleted_event_ids(event: &Event) -> Vec<String> {
    event
        .tags
        .iter()
        .filter(|tag| tag.get(0).map(|s| s.as_str()) == Some("e"))
        .filter_map(|tag| tag.get(1))
        .map(|s| s.to_string())
        .collect()
}

/// Get the list of addressable event coordinates that a deletion request wants to delete.
///
/// Returns all addressable event references in `a` tags.
/// Format: `<kind>:<pubkey>:<d-identifier>`
///
/// # Example
///
/// ```
/// use nostr_core::nip09::get_deleted_addresses;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// let addresses = get_deleted_addresses(event);
/// for addr in addresses {
///     println!("Requesting deletion of addressable event: {}", addr);
/// }
/// # }
/// ```
pub fn get_deleted_addresses(event: &Event) -> Vec<String> {
    event
        .tags
        .iter()
        .filter(|tag| tag.get(0).map(|s| s.as_str()) == Some("a"))
        .filter_map(|tag| tag.get(1))
        .map(|s| s.to_string())
        .collect()
}

/// Get the kinds of events being deleted (from `k` tags).
///
/// Returns the list of event kinds mentioned in the deletion request.
///
/// # Example
///
/// ```
/// use nostr_core::nip09::get_deleted_kinds;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// let kinds = get_deleted_kinds(event);
/// println!("Deleting events of kinds: {:?}", kinds);
/// # }
/// ```
pub fn get_deleted_kinds(event: &Event) -> Vec<u16> {
    event
        .tags
        .iter()
        .filter(|tag| tag.get(0).map(|s| s.as_str()) == Some("k"))
        .filter_map(|tag| tag.get(1))
        .filter_map(|s| s.parse::<u16>().ok())
        .collect()
}

/// Get the deletion reason from the event content.
///
/// Returns `None` if the content is empty.
///
/// # Example
///
/// ```
/// use nostr_core::nip09::get_deletion_reason;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if let Some(reason) = get_deletion_reason(event) {
///     println!("Deletion reason: {}", reason);
/// }
/// # }
/// ```
pub fn get_deletion_reason(event: &Event) -> Option<&str> {
    if event.content.is_empty() {
        None
    } else {
        Some(&event.content)
    }
}

/// Check if a deletion request should delete a specific event.
///
/// This validates that:
/// 1. The deletion request is kind 5
/// 2. The deletion request and target event have the same pubkey
/// 3. The deletion request references the target event's ID
///
/// # Arguments
///
/// * `deletion_request` - The deletion request event (kind 5)
/// * `target_event` - The event that might be deleted
///
/// # Errors
///
/// Returns an error if pubkeys don't match.
///
/// # Example
///
/// ```
/// use nostr_core::nip09::should_delete_event;
/// # use nostr_core::Event;
/// # fn example(deletion_request: &Event, target_event: &Event) {
/// match should_delete_event(deletion_request, target_event) {
///     Ok(true) => println!("Event should be deleted"),
///     Ok(false) => println!("Event is not referenced in deletion request"),
///     Err(e) => println!("Invalid deletion request: {}", e),
/// }
/// # }
/// ```
pub fn should_delete_event(
    deletion_request: &Event,
    target_event: &Event,
) -> Result<bool, Nip09Error> {
    // Verify it's a deletion request
    if !is_deletion_request(deletion_request) {
        return Ok(false);
    }

    // MUST validate pubkey match
    if deletion_request.pubkey != target_event.pubkey {
        return Err(Nip09Error::PubkeyMismatch);
    }

    // Check if the target event ID is referenced
    let deleted_ids = get_deleted_event_ids(deletion_request);
    Ok(deleted_ids.contains(&target_event.id))
}

/// Create deletion request tags for specific event IDs.
///
/// # Arguments
///
/// * `event_ids` - Event IDs to delete
/// * `kind` - Optional kind of events being deleted
///
/// # Example
///
/// ```
/// use nostr_core::nip09::create_deletion_tags;
///
/// let tags = create_deletion_tags(&["event-id-1", "event-id-2"], Some(1));
/// // Results in:
/// // [["e", "event-id-1"], ["e", "event-id-2"], ["k", "1"]]
/// ```
pub fn create_deletion_tags(event_ids: &[&str], kind: Option<u16>) -> Vec<Vec<String>> {
    let mut tags: Vec<Vec<String>> = event_ids
        .iter()
        .map(|id| vec!["e".to_string(), id.to_string()])
        .collect();

    if let Some(k) = kind {
        tags.push(vec!["k".to_string(), k.to_string()]);
    }

    tags
}

/// Create deletion request tags for addressable events.
///
/// # Arguments
///
/// * `addresses` - Addressable event coordinates (`<kind>:<pubkey>:<d-identifier>`)
/// * `kind` - Optional kind of events being deleted
///
/// # Example
///
/// ```
/// use nostr_core::nip09::create_deletion_tags_for_addresses;
///
/// let tags = create_deletion_tags_for_addresses(
///     &["30023:pubkey:article-id"],
///     Some(30023)
/// );
/// ```
pub fn create_deletion_tags_for_addresses(
    addresses: &[&str],
    kind: Option<u16>,
) -> Vec<Vec<String>> {
    let mut tags: Vec<Vec<String>> = addresses
        .iter()
        .map(|addr| vec!["a".to_string(), addr.to_string()])
        .collect();

    if let Some(k) = kind {
        tags.push(vec!["k".to_string(), k.to_string()]);
    }

    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(kind: u16, pubkey: &str, tags: Vec<Vec<String>>, content: &str) -> Event {
        Event {
            id: "test-event-id".to_string(),
            pubkey: pubkey.to_string(),
            created_at: 1000000,
            kind,
            tags,
            content: content.to_string(),
            sig: "0".repeat(128),
        }
    }

    #[test]
    fn test_is_deletion_request_true() {
        let event = create_test_event(5, "pubkey", vec![], "");
        assert!(is_deletion_request(&event));
    }

    #[test]
    fn test_is_deletion_request_false() {
        let event = create_test_event(1, "pubkey", vec![], "");
        assert!(!is_deletion_request(&event));
    }

    #[test]
    fn test_get_deleted_event_ids() {
        let tags = vec![
            vec!["e".to_string(), "event-id-1".to_string()],
            vec!["e".to_string(), "event-id-2".to_string()],
            vec!["p".to_string(), "pubkey".to_string()],
        ];
        let event = create_test_event(5, "pubkey", tags, "");

        let ids = get_deleted_event_ids(&event);
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"event-id-1".to_string()));
        assert!(ids.contains(&"event-id-2".to_string()));
    }

    #[test]
    fn test_get_deleted_addresses() {
        let tags = vec![
            vec!["a".to_string(), "30023:pubkey:article-1".to_string()],
            vec!["e".to_string(), "event-id".to_string()],
        ];
        let event = create_test_event(5, "pubkey", tags, "");

        let addresses = get_deleted_addresses(&event);
        assert_eq!(addresses.len(), 1);
        assert_eq!(addresses[0], "30023:pubkey:article-1");
    }

    #[test]
    fn test_get_deleted_kinds() {
        let tags = vec![
            vec!["k".to_string(), "1".to_string()],
            vec!["k".to_string(), "30023".to_string()],
            vec!["e".to_string(), "event-id".to_string()],
        ];
        let event = create_test_event(5, "pubkey", tags, "");

        let kinds = get_deleted_kinds(&event);
        assert_eq!(kinds.len(), 2);
        assert!(kinds.contains(&1));
        assert!(kinds.contains(&30023));
    }

    #[test]
    fn test_get_deletion_reason_with_content() {
        let event = create_test_event(5, "pubkey", vec![], "Posted by mistake");
        assert_eq!(get_deletion_reason(&event), Some("Posted by mistake"));
    }

    #[test]
    fn test_get_deletion_reason_empty() {
        let event = create_test_event(5, "pubkey", vec![], "");
        assert_eq!(get_deletion_reason(&event), None);
    }

    #[test]
    fn test_should_delete_event_valid() {
        let deletion = create_test_event(
            5,
            "same-pubkey",
            vec![vec!["e".to_string(), "target-id".to_string()]],
            "",
        );

        let mut target = create_test_event(1, "same-pubkey", vec![], "content");
        target.id = "target-id".to_string();

        let result = should_delete_event(&deletion, &target);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_should_delete_event_not_referenced() {
        let deletion = create_test_event(
            5,
            "same-pubkey",
            vec![vec!["e".to_string(), "other-id".to_string()]],
            "",
        );

        let mut target = create_test_event(1, "same-pubkey", vec![], "content");
        target.id = "target-id".to_string();

        let result = should_delete_event(&deletion, &target);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_should_delete_event_pubkey_mismatch() {
        let deletion = create_test_event(
            5,
            "pubkey-1",
            vec![vec!["e".to_string(), "target-id".to_string()]],
            "",
        );

        let mut target = create_test_event(1, "pubkey-2", vec![], "content");
        target.id = "target-id".to_string();

        let result = should_delete_event(&deletion, &target);
        assert!(result.is_err());
        match result.unwrap_err() {
            Nip09Error::PubkeyMismatch => {}
            _ => panic!("Expected PubkeyMismatch error"),
        }
    }

    #[test]
    fn test_should_delete_event_not_deletion_kind() {
        let not_deletion = create_test_event(
            1,
            "same-pubkey",
            vec![vec!["e".to_string(), "target-id".to_string()]],
            "",
        );

        let mut target = create_test_event(1, "same-pubkey", vec![], "content");
        target.id = "target-id".to_string();

        let result = should_delete_event(&not_deletion, &target);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_create_deletion_tags() {
        let tags = create_deletion_tags(&["id1", "id2"], Some(1));

        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0], vec!["e", "id1"]);
        assert_eq!(tags[1], vec!["e", "id2"]);
        assert_eq!(tags[2], vec!["k", "1"]);
    }

    #[test]
    fn test_create_deletion_tags_no_kind() {
        let tags = create_deletion_tags(&["id1"], None);

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], vec!["e", "id1"]);
    }

    #[test]
    fn test_create_deletion_tags_for_addresses() {
        let tags = create_deletion_tags_for_addresses(&["30023:pk:id"], Some(30023));

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0], vec!["a", "30023:pk:id"]);
        assert_eq!(tags[1], vec!["k", "30023"]);
    }

    #[test]
    fn test_empty_deletion_request() {
        let event = create_test_event(5, "pubkey", vec![], "");

        assert!(is_deletion_request(&event));
        assert_eq!(get_deleted_event_ids(&event).len(), 0);
        assert_eq!(get_deleted_addresses(&event).len(), 0);
        assert_eq!(get_deleted_kinds(&event).len(), 0);
    }
}
