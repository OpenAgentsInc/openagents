//! NIP-12: Generic Tag Queries
//!
//! **DEPRECATED:** This NIP has been moved to NIP-01.
//!
//! NIP-12 originally defined generic tag queries for relay filters, allowing clients
//! to query events by any tag type (not just `e` and `p` tags). This functionality
//! is now part of the core Nostr protocol specification (NIP-01).
//!
//! ## Generic Tag Queries
//!
//! In NIP-01, filters can include any tag attribute using the `#<tag_name>` format:
//!
//! ```json
//! {
//!   "#e": ["event-id-1", "event-id-2"],
//!   "#p": ["pubkey-1", "pubkey-2"],
//!   "#t": ["bitcoin", "nostr"],
//!   "#r": ["https://example.com"]
//! }
//! ```
//!
//! For tag attributes, the event and filter condition values must have at least
//! one item in common for the event to match.
//!
//! ## Usage
//!
//! This module provides helper functions for working with generic tags in events.
//!
//! # Example
//!
//! ```
//! use nostr_core::nip12::{add_generic_tag, get_tag_values, has_tag};
//! use nostr_core::Event;
//!
//! # fn example(event: &Event) {
//! // Check if event has a specific tag
//! if has_tag(event, "t") {
//!     let topics = get_tag_values(event, "t");
//!     println!("Topics: {:?}", topics);
//! }
//! # }
//! ```

use crate::nip01::Event;
use thiserror::Error;

/// Errors that can occur during NIP-12 operations.
#[derive(Debug, Error)]
pub enum Nip12Error {
    #[error("invalid tag format")]
    InvalidTagFormat,
}

/// Check if an event has a specific tag type.
///
/// # Arguments
///
/// * `event` - The event to check
/// * `tag_name` - The tag name (e.g., "e", "p", "t", "r")
///
/// # Example
///
/// ```
/// use nostr_core::nip12::has_tag;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if has_tag(event, "t") {
///     println!("Event has topic tags");
/// }
/// # }
/// ```
pub fn has_tag(event: &Event, tag_name: &str) -> bool {
    event
        .tags
        .iter()
        .any(|tag| tag.get(0).map(|s| s.as_str()) == Some(tag_name))
}

/// Get all values for a specific tag type.
///
/// Returns a vector of all values found for the given tag name.
///
/// # Arguments
///
/// * `event` - The event to search
/// * `tag_name` - The tag name (e.g., "e", "p", "t", "r")
///
/// # Example
///
/// ```
/// use nostr_core::nip12::get_tag_values;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// let topics = get_tag_values(event, "t");
/// for topic in topics {
///     println!("Topic: {}", topic);
/// }
/// # }
/// ```
pub fn get_tag_values(event: &Event, tag_name: &str) -> Vec<String> {
    event
        .tags
        .iter()
        .filter(|tag| tag.get(0).map(|s| s.as_str()) == Some(tag_name))
        .filter_map(|tag| tag.get(1))
        .map(|s| s.to_string())
        .collect()
}

/// Get all values for a specific tag type with additional parameters.
///
/// Returns a vector of tuples containing the value and optional additional parameters.
///
/// # Arguments
///
/// * `event` - The event to search
/// * `tag_name` - The tag name
///
/// # Example
///
/// ```
/// use nostr_core::nip12::get_tag_values_with_params;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// // For ["e", "event-id", "relay-url", "marker"]
/// let e_tags = get_tag_values_with_params(event, "e");
/// for (value, params) in e_tags {
///     println!("Event ID: {}, params: {:?}", value, params);
/// }
/// # }
/// ```
pub fn get_tag_values_with_params(event: &Event, tag_name: &str) -> Vec<(String, Vec<String>)> {
    event
        .tags
        .iter()
        .filter(|tag| tag.get(0).map(|s| s.as_str()) == Some(tag_name))
        .filter_map(|tag| {
            let value = tag.get(1)?;
            let params: Vec<String> = tag.iter().skip(2).cloned().collect();
            Some((value.to_string(), params))
        })
        .collect()
}

/// Add a generic tag to event tags.
///
/// # Arguments
///
/// * `tags` - The mutable vector of tags
/// * `tag_name` - The tag name
/// * `value` - The tag value
/// * `additional` - Optional additional parameters
///
/// # Example
///
/// ```
/// use nostr_core::nip12::add_generic_tag;
///
/// let mut tags: Vec<Vec<String>> = vec![];
/// add_generic_tag(&mut tags, "t", "bitcoin", &[]);
/// add_generic_tag(&mut tags, "r", "https://example.com", &["web"]);
/// ```
pub fn add_generic_tag(
    tags: &mut Vec<Vec<String>>,
    tag_name: &str,
    value: &str,
    additional: &[&str],
) {
    let mut tag = vec![tag_name.to_string(), value.to_string()];
    tag.extend(additional.iter().map(|s| s.to_string()));
    tags.push(tag);
}

/// Remove all tags of a specific type.
///
/// # Arguments
///
/// * `tags` - The mutable vector of tags
/// * `tag_name` - The tag name to remove
///
/// # Example
///
/// ```
/// use nostr_core::nip12::remove_tags;
///
/// let mut tags = vec![
///     vec!["t".to_string(), "bitcoin".to_string()],
///     vec!["p".to_string(), "pubkey".to_string()],
///     vec!["t".to_string(), "nostr".to_string()],
/// ];
/// remove_tags(&mut tags, "t");
/// // Only the "p" tag remains
/// ```
pub fn remove_tags(tags: &mut Vec<Vec<String>>, tag_name: &str) {
    tags.retain(|tag| tag.get(0).map(|s| s.as_str()) != Some(tag_name));
}

/// Check if an event matches a tag filter.
///
/// A tag filter is a list of acceptable values. The event matches if it has
/// at least one tag with the specified name and at least one value from the filter.
///
/// # Arguments
///
/// * `event` - The event to check
/// * `tag_name` - The tag name to check
/// * `filter_values` - Acceptable values (event must have at least one)
///
/// # Example
///
/// ```
/// use nostr_core::nip12::matches_tag_filter;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if matches_tag_filter(event, "t", &["bitcoin", "nostr"]) {
///     println!("Event is about bitcoin or nostr");
/// }
/// # }
/// ```
pub fn matches_tag_filter(event: &Event, tag_name: &str, filter_values: &[&str]) -> bool {
    let event_values = get_tag_values(event, tag_name);
    filter_values
        .iter()
        .any(|filter_val| event_values.iter().any(|event_val| event_val == filter_val))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(tags: Vec<Vec<String>>) -> Event {
        Event {
            id: "test-event-id".to_string(),
            pubkey: "test-pubkey".to_string(),
            created_at: 1000000,
            kind: 1,
            tags,
            content: "test content".to_string(),
            sig: "0".repeat(128),
        }
    }

    #[test]
    fn test_has_tag_true() {
        let event = create_test_event(vec![vec!["t".to_string(), "bitcoin".to_string()]]);
        assert!(has_tag(&event, "t"));
    }

    #[test]
    fn test_has_tag_false() {
        let event = create_test_event(vec![vec!["p".to_string(), "pubkey".to_string()]]);
        assert!(!has_tag(&event, "t"));
    }

    #[test]
    fn test_get_tag_values() {
        let event = create_test_event(vec![
            vec!["t".to_string(), "bitcoin".to_string()],
            vec!["t".to_string(), "nostr".to_string()],
            vec!["p".to_string(), "pubkey".to_string()],
        ]);

        let topics = get_tag_values(&event, "t");
        assert_eq!(topics.len(), 2);
        assert!(topics.contains(&"bitcoin".to_string()));
        assert!(topics.contains(&"nostr".to_string()));
    }

    #[test]
    fn test_get_tag_values_empty() {
        let event = create_test_event(vec![]);
        let values = get_tag_values(&event, "t");
        assert_eq!(values.len(), 0);
    }

    #[test]
    fn test_get_tag_values_with_params() {
        let event = create_test_event(vec![
            vec![
                "e".to_string(),
                "event-id".to_string(),
                "wss://relay.com".to_string(),
                "reply".to_string(),
            ],
            vec!["p".to_string(), "pubkey".to_string()],
        ]);

        let e_tags = get_tag_values_with_params(&event, "e");
        assert_eq!(e_tags.len(), 1);
        assert_eq!(e_tags[0].0, "event-id");
        assert_eq!(e_tags[0].1, vec!["wss://relay.com", "reply"]);
    }

    #[test]
    fn test_add_generic_tag() {
        let mut tags = vec![];
        add_generic_tag(&mut tags, "t", "bitcoin", &[]);

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], vec!["t", "bitcoin"]);
    }

    #[test]
    fn test_add_generic_tag_with_params() {
        let mut tags = vec![];
        add_generic_tag(&mut tags, "r", "https://example.com", &["web"]);

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], vec!["r", "https://example.com", "web"]);
    }

    #[test]
    fn test_remove_tags() {
        let mut tags = vec![
            vec!["t".to_string(), "bitcoin".to_string()],
            vec!["p".to_string(), "pubkey".to_string()],
            vec!["t".to_string(), "nostr".to_string()],
        ];

        remove_tags(&mut tags, "t");

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0][0], "p");
    }

    #[test]
    fn test_matches_tag_filter_true() {
        let event = create_test_event(vec![
            vec!["t".to_string(), "bitcoin".to_string()],
            vec!["t".to_string(), "nostr".to_string()],
        ]);

        assert!(matches_tag_filter(&event, "t", &["bitcoin", "lightning"]));
        assert!(matches_tag_filter(&event, "t", &["nostr"]));
    }

    #[test]
    fn test_matches_tag_filter_false() {
        let event = create_test_event(vec![vec!["t".to_string(), "bitcoin".to_string()]]);

        assert!(!matches_tag_filter(&event, "t", &["lightning", "nostr"]));
    }

    #[test]
    fn test_matches_tag_filter_no_tag() {
        let event = create_test_event(vec![]);
        assert!(!matches_tag_filter(&event, "t", &["bitcoin"]));
    }

    #[test]
    fn test_multiple_tags_same_type() {
        let event = create_test_event(vec![
            vec!["e".to_string(), "id1".to_string()],
            vec!["e".to_string(), "id2".to_string()],
            vec!["e".to_string(), "id3".to_string()],
        ]);

        let event_ids = get_tag_values(&event, "e");
        assert_eq!(event_ids.len(), 3);
    }
}
