//! NIP-22: Comment
//!
//! This module implements NIP-22, which defines a standard for comments on Nostr.
//! Comments use kind 1111 events with specific tag requirements to create threaded
//! discussions on various types of content.
//!
//! ## Comment Event Structure
//!
//! A comment event (kind 1111) includes:
//! - Plaintext content (no HTML/Markdown)
//! - Root scope tags (uppercase): K, E, A, I, P
//! - Parent item tags (lowercase): k, e, a, i, p
//!
//! ## Tag Requirements
//!
//! - Comments MUST point to root scope using uppercase tag names
//! - Comments MUST point to parent item using lowercase tag names
//! - K and k tags MUST be present to define the event kind
//!
//! ## Example
//!
//! ```json
//! {
//!   "kind": 1111,
//!   "content": "Great article!",
//!   "tags": [
//!     ["K", "30023"],
//!     ["E", "<article-event-id>", "<relay-url>"],
//!     ["k", "1111"],
//!     ["e", "<parent-comment-id>", "<relay-url>"]
//!   ]
//! }
//! ```
//!
//! # Usage
//!
//! ```
//! use nostr_core::nip22::{is_comment, get_root_kind, get_parent_event_id};
//! use nostr_core::Event;
//!
//! # fn example(event: &Event) {
//! if is_comment(event) {
//!     if let Some(kind) = get_root_kind(event) {
//!         println!("Comment on kind {} event", kind);
//!     }
//! }
//! # }
//! ```

use crate::nip01::Event;
use thiserror::Error;

/// Event kind for comments
pub const COMMENT_KIND: u16 = 1111;

/// Errors that can occur during NIP-22 operations.
#[derive(Debug, Error)]
pub enum Nip22Error {
    #[error("invalid comment: missing required tags")]
    MissingRequiredTags,

    #[error("invalid comment: cannot comment on kind 1 notes (use NIP-10)")]
    InvalidRootKind,
}

/// Check if an event is a comment (kind 1111).
///
/// # Example
///
/// ```
/// use nostr_core::nip22::is_comment;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if is_comment(event) {
///     println!("This is a comment");
/// }
/// # }
/// ```
pub fn is_comment(event: &Event) -> bool {
    event.kind == COMMENT_KIND
}

/// Get the root event kind from a comment's K tag.
///
/// Returns `None` if the K tag is not present or invalid.
///
/// # Example
///
/// ```
/// use nostr_core::nip22::get_root_kind;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if let Some(kind) = get_root_kind(event) {
///     println!("Commenting on kind {}", kind);
/// }
/// # }
/// ```
pub fn get_root_kind(event: &Event) -> Option<u16> {
    event
        .tags
        .iter()
        .find(|tag| tag.get(0).map(|s| s.as_str()) == Some("K"))
        .and_then(|tag| tag.get(1))
        .and_then(|s| s.parse::<u16>().ok())
}

/// Get the parent comment kind from a comment's k tag.
///
/// Returns `None` if the k tag is not present or invalid.
///
/// # Example
///
/// ```
/// use nostr_core::nip22::get_parent_kind;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if let Some(kind) = get_parent_kind(event) {
///     println!("Replying to kind {}", kind);
/// }
/// # }
/// ```
pub fn get_parent_kind(event: &Event) -> Option<u16> {
    event
        .tags
        .iter()
        .find(|tag| tag.get(0).map(|s| s.as_str()) == Some("k"))
        .and_then(|tag| tag.get(1))
        .and_then(|s| s.parse::<u16>().ok())
}

/// Get the root event ID from a comment's E tag.
///
/// # Example
///
/// ```
/// use nostr_core::nip22::get_root_event_id;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if let Some(event_id) = get_root_event_id(event) {
///     println!("Root event: {}", event_id);
/// }
/// # }
/// ```
pub fn get_root_event_id(event: &Event) -> Option<String> {
    event
        .tags
        .iter()
        .find(|tag| tag.get(0).map(|s| s.as_str()) == Some("E"))
        .and_then(|tag| tag.get(1))
        .map(|s| s.to_string())
}

/// Get the parent event ID from a comment's e tag.
///
/// # Example
///
/// ```
/// use nostr_core::nip22::get_parent_event_id;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if let Some(event_id) = get_parent_event_id(event) {
///     println!("Replying to: {}", event_id);
/// }
/// # }
/// ```
pub fn get_parent_event_id(event: &Event) -> Option<String> {
    event
        .tags
        .iter()
        .find(|tag| tag.get(0).map(|s| s.as_str()) == Some("e"))
        .and_then(|tag| tag.get(1))
        .map(|s| s.to_string())
}

/// Get the root address from a comment's A tag.
///
/// Address format: `<kind>:<pubkey>:<d-identifier>`
///
/// # Example
///
/// ```
/// use nostr_core::nip22::get_root_address;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if let Some(address) = get_root_address(event) {
///     println!("Commenting on addressable event: {}", address);
/// }
/// # }
/// ```
pub fn get_root_address(event: &Event) -> Option<String> {
    event
        .tags
        .iter()
        .find(|tag| tag.get(0).map(|s| s.as_str()) == Some("A"))
        .and_then(|tag| tag.get(1))
        .map(|s| s.to_string())
}

/// Get the parent address from a comment's a tag.
///
/// # Example
///
/// ```
/// use nostr_core::nip22::get_parent_address;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if let Some(address) = get_parent_address(event) {
///     println!("Replying to addressable event: {}", address);
/// }
/// # }
/// ```
pub fn get_parent_address(event: &Event) -> Option<String> {
    event
        .tags
        .iter()
        .find(|tag| tag.get(0).map(|s| s.as_str()) == Some("a"))
        .and_then(|tag| tag.get(1))
        .map(|s| s.to_string())
}

/// Validate a comment event.
///
/// Checks that:
/// - Event is kind 1111
/// - K tag is present and not kind 1
/// - k tag is present
///
/// # Errors
///
/// Returns an error if required tags are missing or the root kind is 1.
///
/// # Example
///
/// ```
/// use nostr_core::nip22::validate_comment;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// match validate_comment(event) {
///     Ok(()) => println!("Valid comment"),
///     Err(e) => println!("Invalid comment: {}", e),
/// }
/// # }
/// ```
pub fn validate_comment(event: &Event) -> Result<(), Nip22Error> {
    if !is_comment(event) {
        return Ok(()); // Not a comment, no validation needed
    }

    // K tag must be present
    let root_kind = get_root_kind(event).ok_or(Nip22Error::MissingRequiredTags)?;

    // Cannot comment on kind 1 notes (use NIP-10 instead)
    if root_kind == 1 {
        return Err(Nip22Error::InvalidRootKind);
    }

    // k tag must be present
    if get_parent_kind(event).is_none() {
        return Err(Nip22Error::MissingRequiredTags);
    }

    Ok(())
}

/// Create comment tags for a root event.
///
/// # Arguments
///
/// * `root_kind` - The kind of the root event
/// * `root_event_id` - Optional root event ID (for E tag)
/// * `root_address` - Optional root address (for A tag)
/// * `parent_kind` - The kind of the immediate parent
/// * `parent_event_id` - Optional parent event ID (for e tag)
/// * `parent_address` - Optional parent address (for a tag)
///
/// # Example
///
/// ```
/// use nostr_core::nip22::create_comment_tags;
///
/// let tags = create_comment_tags(
///     30023,
///     Some("root-event-id"),
///     None,
///     1111,
///     Some("parent-comment-id"),
///     None
/// );
/// ```
pub fn create_comment_tags(
    root_kind: u16,
    root_event_id: Option<&str>,
    root_address: Option<&str>,
    parent_kind: u16,
    parent_event_id: Option<&str>,
    parent_address: Option<&str>,
) -> Vec<Vec<String>> {
    let mut tags = vec![];

    // K tag (required)
    tags.push(vec!["K".to_string(), root_kind.to_string()]);

    // E tag (if root event ID provided)
    if let Some(id) = root_event_id {
        tags.push(vec!["E".to_string(), id.to_string()]);
    }

    // A tag (if root address provided)
    if let Some(addr) = root_address {
        tags.push(vec!["A".to_string(), addr.to_string()]);
    }

    // k tag (required)
    tags.push(vec!["k".to_string(), parent_kind.to_string()]);

    // e tag (if parent event ID provided)
    if let Some(id) = parent_event_id {
        tags.push(vec!["e".to_string(), id.to_string()]);
    }

    // a tag (if parent address provided)
    if let Some(addr) = parent_address {
        tags.push(vec!["a".to_string(), addr.to_string()]);
    }

    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(kind: u16, tags: Vec<Vec<String>>) -> Event {
        Event {
            id: "test-event-id".to_string(),
            pubkey: "test-pubkey".to_string(),
            created_at: 1000000,
            kind,
            tags,
            content: "test comment".to_string(),
            sig: "0".repeat(128),
        }
    }

    #[test]
    fn test_is_comment_true() {
        let event = create_test_event(1111, vec![]);
        assert!(is_comment(&event));
    }

    #[test]
    fn test_is_comment_false() {
        let event = create_test_event(1, vec![]);
        assert!(!is_comment(&event));
    }

    #[test]
    fn test_get_root_kind() {
        let event = create_test_event(1111, vec![vec!["K".to_string(), "30023".to_string()]]);
        assert_eq!(get_root_kind(&event), Some(30023));
    }

    #[test]
    fn test_get_root_kind_none() {
        let event = create_test_event(1111, vec![]);
        assert_eq!(get_root_kind(&event), None);
    }

    #[test]
    fn test_get_parent_kind() {
        let event = create_test_event(1111, vec![vec!["k".to_string(), "1111".to_string()]]);
        assert_eq!(get_parent_kind(&event), Some(1111));
    }

    #[test]
    fn test_get_root_event_id() {
        let event = create_test_event(
            1111,
            vec![vec!["E".to_string(), "root-event-id".to_string()]],
        );
        assert_eq!(get_root_event_id(&event), Some("root-event-id".to_string()));
    }

    #[test]
    fn test_get_parent_event_id() {
        let event = create_test_event(
            1111,
            vec![vec!["e".to_string(), "parent-event-id".to_string()]],
        );
        assert_eq!(
            get_parent_event_id(&event),
            Some("parent-event-id".to_string())
        );
    }

    #[test]
    fn test_get_root_address() {
        let event = create_test_event(
            1111,
            vec![vec!["A".to_string(), "30023:pubkey:article".to_string()]],
        );
        assert_eq!(
            get_root_address(&event),
            Some("30023:pubkey:article".to_string())
        );
    }

    #[test]
    fn test_get_parent_address() {
        let event = create_test_event(
            1111,
            vec![vec!["a".to_string(), "1111:pubkey:comment".to_string()]],
        );
        assert_eq!(
            get_parent_address(&event),
            Some("1111:pubkey:comment".to_string())
        );
    }

    #[test]
    fn test_validate_comment_valid() {
        let event = create_test_event(
            1111,
            vec![
                vec!["K".to_string(), "30023".to_string()],
                vec!["k".to_string(), "1111".to_string()],
            ],
        );
        assert!(validate_comment(&event).is_ok());
    }

    #[test]
    fn test_validate_comment_missing_k_tag() {
        let event = create_test_event(1111, vec![vec!["K".to_string(), "30023".to_string()]]);
        assert!(validate_comment(&event).is_err());
    }

    #[test]
    fn test_validate_comment_missing_capital_k_tag() {
        let event = create_test_event(1111, vec![vec!["k".to_string(), "1111".to_string()]]);
        assert!(validate_comment(&event).is_err());
    }

    #[test]
    fn test_validate_comment_kind_1_root() {
        let event = create_test_event(
            1111,
            vec![
                vec!["K".to_string(), "1".to_string()],
                vec!["k".to_string(), "1111".to_string()],
            ],
        );
        let result = validate_comment(&event);
        assert!(result.is_err());
        match result.unwrap_err() {
            Nip22Error::InvalidRootKind => {}
            _ => panic!("Expected InvalidRootKind error"),
        }
    }

    #[test]
    fn test_validate_comment_not_a_comment() {
        let event = create_test_event(1, vec![]);
        assert!(validate_comment(&event).is_ok()); // Returns Ok because it's not a comment
    }

    #[test]
    fn test_create_comment_tags() {
        let tags = create_comment_tags(30023, Some("root-id"), None, 1111, Some("parent-id"), None);

        assert_eq!(tags.len(), 4);
        assert_eq!(tags[0], vec!["K", "30023"]);
        assert_eq!(tags[1], vec!["E", "root-id"]);
        assert_eq!(tags[2], vec!["k", "1111"]);
        assert_eq!(tags[3], vec!["e", "parent-id"]);
    }

    #[test]
    fn test_create_comment_tags_with_addresses() {
        let tags = create_comment_tags(
            30023,
            None,
            Some("30023:pk:article"),
            1111,
            None,
            Some("1111:pk:comment"),
        );

        assert_eq!(tags.len(), 4);
        assert_eq!(tags[0], vec!["K", "30023"]);
        assert_eq!(tags[1], vec!["A", "30023:pk:article"]);
        assert_eq!(tags[2], vec!["k", "1111"]);
        assert_eq!(tags[3], vec!["a", "1111:pk:comment"]);
    }
}
