//! NIP-33: Parameterized Replaceable Events
//!
//! **DEPRECATED:** This NIP has been renamed to "Addressable events" and moved to NIP-01.
//!
//! NIP-33 originally defined parameterized replaceable events, now known as "addressable events."
//! These are events with kinds in the range 30000-39999 that are uniquely identified by the
//! combination of (kind, pubkey, d tag value).
//!
//! ## Addressable Events
//!
//! For kind `n` such that `30000 <= n < 40000`, events are addressable by their kind, pubkey,
//! and `d` tag value. For each unique combination of these three values, only the latest event
//! MUST be stored by relays; older versions MAY be discarded.
//!
//! ## The "d" Tag
//!
//! The `d` tag serves as an identifier parameter, allowing users to maintain multiple independent
//! replaceable events of the same kind. The tag format is:
//!
//! ```json
//! ["d", "<identifier>"]
//! ```
//!
//! ## Address Format
//!
//! Addressable events are referenced using the format:
//!
//! ```
//! <kind>:<pubkey>:<d-tag-value>
//! ```
//!
//! For example: `30023:3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d:my-article`
//!
//! ## Event Tag Format
//!
//! When referencing an addressable event in tags:
//!
//! ```json
//! ["a", "<kind>:<pubkey>:<d-tag-value>", "<optional-relay-url>"]
//! ```
//!
//! # Usage
//!
//! ```
//! use nostr_core::nip33::{is_addressable, get_d_tag, create_address};
//! use nostr_core::Event;
//!
//! # fn example(event: &Event) {
//! if is_addressable(event) {
//!     if let Some(d_tag) = get_d_tag(event) {
//!         let address = create_address(event.kind, &event.pubkey, &d_tag);
//!         println!("Addressable event: {}", address);
//!     }
//! }
//! # }
//! ```

use crate::nip01::Event;
use thiserror::Error;

/// Minimum kind for addressable events
pub const ADDRESSABLE_KIND_MIN: u16 = 30000;

/// Maximum kind for addressable events (inclusive)
pub const ADDRESSABLE_KIND_MAX: u16 = 39999;

/// Tag name for the d-identifier
pub const D_TAG: &str = "d";

/// Errors that can occur during NIP-33 operations.
#[derive(Debug, Error)]
pub enum Nip33Error {
    #[error("event is not addressable (kind must be 30000-39999)")]
    NotAddressable,

    #[error("missing required d tag")]
    MissingDTag,

    #[error("invalid address format: {0}")]
    InvalidAddress(String),

    #[error("invalid d tag value: {0}")]
    InvalidDTag(String),
}

/// Check if an event kind is addressable (30000-39999).
///
/// # Arguments
///
/// * `kind` - The event kind to check
///
/// # Example
///
/// ```
/// use nostr_core::nip33::is_addressable_kind;
///
/// assert!(is_addressable_kind(30023)); // Long-form content
/// assert!(is_addressable_kind(30000));
/// assert!(is_addressable_kind(39999));
/// assert!(!is_addressable_kind(1));    // Regular note
/// assert!(!is_addressable_kind(10000)); // Replaceable event
/// ```
pub fn is_addressable_kind(kind: u16) -> bool {
    (ADDRESSABLE_KIND_MIN..=ADDRESSABLE_KIND_MAX).contains(&kind)
}

/// Check if an event is addressable.
///
/// # Example
///
/// ```
/// use nostr_core::nip33::is_addressable;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if is_addressable(event) {
///     println!("This is an addressable event");
/// }
/// # }
/// ```
pub fn is_addressable(event: &Event) -> bool {
    is_addressable_kind(event.kind)
}

/// Get the d tag value from an addressable event.
///
/// Returns `None` if the event has no d tag.
///
/// # Example
///
/// ```
/// use nostr_core::nip33::get_d_tag;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if let Some(d) = get_d_tag(event) {
///     println!("D-tag: {}", d);
/// }
/// # }
/// ```
pub fn get_d_tag(event: &Event) -> Option<String> {
    event
        .tags
        .iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(D_TAG))
        .and_then(|tag| tag.get(1))
        .map(|s| s.to_string())
}

/// Create an address string for an addressable event.
///
/// The address format is: `<kind>:<pubkey>:<d-tag-value>`
///
/// # Arguments
///
/// * `kind` - The event kind (should be 30000-39999)
/// * `pubkey` - The creator's public key (32-byte hex)
/// * `d_tag` - The d tag identifier
///
/// # Example
///
/// ```
/// use nostr_core::nip33::create_address;
///
/// let address = create_address(
///     30023,
///     "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d",
///     "my-article"
/// );
/// assert_eq!(
///     address,
///     "30023:3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d:my-article"
/// );
/// ```
pub fn create_address(kind: u16, pubkey: &str, d_tag: &str) -> String {
    format!("{}:{}:{}", kind, pubkey, d_tag)
}

/// Parse an address string into its components.
///
/// Returns `(kind, pubkey, d_tag)` if successful.
///
/// # Arguments
///
/// * `address` - The address string in format `<kind>:<pubkey>:<d-tag>`
///
/// # Errors
///
/// Returns an error if the address format is invalid.
///
/// # Example
///
/// ```
/// use nostr_core::nip33::parse_address;
///
/// let (kind, pubkey, d_tag) = parse_address(
///     "30023:3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d:my-article"
/// ).unwrap();
///
/// assert_eq!(kind, 30023);
/// assert_eq!(pubkey, "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d");
/// assert_eq!(d_tag, "my-article");
/// ```
pub fn parse_address(address: &str) -> Result<(u16, String, String), Nip33Error> {
    let parts: Vec<&str> = address.splitn(3, ':').collect();

    if parts.len() != 3 {
        return Err(Nip33Error::InvalidAddress(
            "address must have format kind:pubkey:dtag".to_string(),
        ));
    }

    let kind = parts[0]
        .parse::<u16>()
        .map_err(|_| Nip33Error::InvalidAddress("invalid kind".to_string()))?;

    if !is_addressable_kind(kind) {
        return Err(Nip33Error::InvalidAddress(format!(
            "kind {} is not addressable (must be 30000-39999)",
            kind
        )));
    }

    let pubkey = parts[1].to_string();
    if pubkey.len() != 64 {
        return Err(Nip33Error::InvalidAddress(
            "pubkey must be 64 hex characters".to_string(),
        ));
    }

    let d_tag = parts[2].to_string();

    Ok((kind, pubkey, d_tag))
}

/// Get the address of an addressable event.
///
/// # Errors
///
/// Returns an error if the event is not addressable or is missing the d tag.
///
/// # Example
///
/// ```
/// use nostr_core::nip33::get_event_address;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// match get_event_address(event) {
///     Ok(address) => println!("Event address: {}", address),
///     Err(e) => println!("Error: {}", e),
/// }
/// # }
/// ```
pub fn get_event_address(event: &Event) -> Result<String, Nip33Error> {
    if !is_addressable(event) {
        return Err(Nip33Error::NotAddressable);
    }

    let d_tag = get_d_tag(event).ok_or(Nip33Error::MissingDTag)?;

    Ok(create_address(event.kind, &event.pubkey, &d_tag))
}

/// Set the d tag for an addressable event.
///
/// This is a helper function to add or update the d tag in event tags.
///
/// # Arguments
///
/// * `tags` - The mutable vector of tags
/// * `d_tag` - The d tag identifier
///
/// # Example
///
/// ```
/// use nostr_core::nip33::set_d_tag;
///
/// let mut tags: Vec<Vec<String>> = vec![];
/// set_d_tag(&mut tags, "my-article");
/// assert_eq!(tags[0], vec!["d", "my-article"]);
/// ```
pub fn set_d_tag(tags: &mut Vec<Vec<String>>, d_tag: &str) {
    // Remove any existing d tags
    tags.retain(|tag| tag.first().map(|s| s.as_str()) != Some(D_TAG));

    // Add the new d tag
    tags.push(vec![D_TAG.to_string(), d_tag.to_string()]);
}

/// Create an "a" tag for referencing an addressable event.
///
/// # Arguments
///
/// * `kind` - The event kind
/// * `pubkey` - The creator's public key
/// * `d_tag` - The d tag identifier
/// * `relay_url` - Optional relay URL hint
///
/// # Example
///
/// ```
/// use nostr_core::nip33::create_a_tag;
///
/// let tag = create_a_tag(
///     30023,
///     "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d",
///     "my-article",
///     Some("wss://relay.example.com")
/// );
///
/// assert_eq!(tag[0], "a");
/// assert_eq!(tag[1], "30023:3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d:my-article");
/// assert_eq!(tag[2], "wss://relay.example.com");
/// ```
pub fn create_a_tag(kind: u16, pubkey: &str, d_tag: &str, relay_url: Option<&str>) -> Vec<String> {
    let address = create_address(kind, pubkey, d_tag);
    let mut tag = vec!["a".to_string(), address];

    if let Some(url) = relay_url {
        tag.push(url.to_string());
    }

    tag
}

/// Get all "a" tag addresses from an event.
///
/// Returns a vector of address strings.
///
/// # Example
///
/// ```
/// use nostr_core::nip33::get_a_tags;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// let addresses = get_a_tags(event);
/// for address in addresses {
///     println!("Referenced address: {}", address);
/// }
/// # }
/// ```
pub fn get_a_tags(event: &Event) -> Vec<String> {
    event
        .tags
        .iter()
        .filter(|tag| tag.first().map(|s| s.as_str()) == Some("a"))
        .filter_map(|tag| tag.get(1))
        .map(|s| s.to_string())
        .collect()
}

/// Validate that an addressable event has a d tag.
///
/// # Errors
///
/// Returns an error if the event is addressable but missing the d tag.
///
/// # Example
///
/// ```
/// use nostr_core::nip33::validate_addressable_event;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// match validate_addressable_event(event) {
///     Ok(()) => println!("Valid addressable event"),
///     Err(e) => println!("Invalid: {}", e),
/// }
/// # }
/// ```
pub fn validate_addressable_event(event: &Event) -> Result<(), Nip33Error> {
    if !is_addressable(event) {
        return Ok(()); // Not an addressable event, no validation needed
    }

    if get_d_tag(event).is_none() {
        return Err(Nip33Error::MissingDTag);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(kind: u16, tags: Vec<Vec<String>>) -> Event {
        Event {
            id: "0".repeat(64),
            pubkey: "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d".to_string(),
            created_at: 1000000,
            kind,
            tags,
            content: "test content".to_string(),
            sig: "0".repeat(128),
        }
    }

    #[test]
    fn test_is_addressable_kind() {
        assert!(is_addressable_kind(30000));
        assert!(is_addressable_kind(30023));
        assert!(is_addressable_kind(39999));
        assert!(!is_addressable_kind(1));
        assert!(!is_addressable_kind(10000));
        assert!(!is_addressable_kind(29999));
        assert!(!is_addressable_kind(40000));
    }

    #[test]
    fn test_is_addressable() {
        let event = create_test_event(30023, vec![]);
        assert!(is_addressable(&event));

        let event = create_test_event(1, vec![]);
        assert!(!is_addressable(&event));
    }

    #[test]
    fn test_get_d_tag() {
        let event = create_test_event(30023, vec![vec!["d".to_string(), "my-article".to_string()]]);
        assert_eq!(get_d_tag(&event), Some("my-article".to_string()));
    }

    #[test]
    fn test_get_d_tag_none() {
        let event = create_test_event(30023, vec![]);
        assert_eq!(get_d_tag(&event), None);
    }

    #[test]
    fn test_create_address() {
        let address = create_address(
            30023,
            "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d",
            "my-article",
        );
        assert_eq!(
            address,
            "30023:3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d:my-article"
        );
    }

    #[test]
    fn test_parse_address() {
        let (kind, pubkey, d_tag) = parse_address(
            "30023:3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d:my-article",
        )
        .unwrap();

        assert_eq!(kind, 30023);
        assert_eq!(
            pubkey,
            "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d"
        );
        assert_eq!(d_tag, "my-article");
    }

    #[test]
    fn test_parse_address_with_colon_in_dtag() {
        let (kind, _pubkey, d_tag) = parse_address(
            "30023:3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d:my:complex:identifier",
        )
        .unwrap();

        assert_eq!(kind, 30023);
        assert_eq!(d_tag, "my:complex:identifier");
    }

    #[test]
    fn test_parse_address_invalid_format() {
        let result = parse_address("30023:pubkey");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_address_invalid_kind() {
        let result = parse_address("not_a_number:pubkey:dtag");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_address_non_addressable_kind() {
        let result = parse_address(
            "1:3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d:dtag",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_address_invalid_pubkey_length() {
        let result = parse_address("30023:shortpubkey:dtag");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_event_address() {
        let event = create_test_event(30023, vec![vec!["d".to_string(), "my-article".to_string()]]);

        let address = get_event_address(&event).unwrap();
        assert_eq!(
            address,
            "30023:3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d:my-article"
        );
    }

    #[test]
    fn test_get_event_address_not_addressable() {
        let event = create_test_event(1, vec![vec!["d".to_string(), "test".to_string()]]);
        let result = get_event_address(&event);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_event_address_missing_d_tag() {
        let event = create_test_event(30023, vec![]);
        let result = get_event_address(&event);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_d_tag() {
        let mut tags = vec![];
        set_d_tag(&mut tags, "my-article");

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], vec!["d", "my-article"]);
    }

    #[test]
    fn test_set_d_tag_replaces_existing() {
        let mut tags = vec![
            vec!["d".to_string(), "old-id".to_string()],
            vec!["p".to_string(), "pubkey".to_string()],
        ];

        set_d_tag(&mut tags, "new-id");

        // Should have 2 tags: p tag and new d tag
        assert_eq!(tags.len(), 2);

        let d_tag = tags.iter().find(|t| t[0] == "d").unwrap();
        assert_eq!(d_tag[1], "new-id");

        assert!(tags.iter().any(|t| t[0] == "p"));
    }

    #[test]
    fn test_create_a_tag() {
        let tag = create_a_tag(
            30023,
            "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d",
            "my-article",
            Some("wss://relay.example.com"),
        );

        assert_eq!(tag.len(), 3);
        assert_eq!(tag[0], "a");
        assert_eq!(
            tag[1],
            "30023:3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d:my-article"
        );
        assert_eq!(tag[2], "wss://relay.example.com");
    }

    #[test]
    fn test_create_a_tag_without_relay() {
        let tag = create_a_tag(
            30023,
            "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d",
            "my-article",
            None,
        );

        assert_eq!(tag.len(), 2);
        assert_eq!(tag[0], "a");
    }

    #[test]
    fn test_get_a_tags() {
        let event = create_test_event(
            1,
            vec![
                vec![
                    "a".to_string(),
                    "30023:3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d:article1"
                        .to_string(),
                ],
                vec!["p".to_string(), "pubkey".to_string()],
                vec![
                    "a".to_string(),
                    "30023:3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d:article2"
                        .to_string(),
                ],
            ],
        );

        let addresses = get_a_tags(&event);
        assert_eq!(addresses.len(), 2);
        assert!(
            addresses.contains(
                &"30023:3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d:article1"
                    .to_string()
            )
        );
        assert!(
            addresses.contains(
                &"30023:3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d:article2"
                    .to_string()
            )
        );
    }

    #[test]
    fn test_validate_addressable_event_valid() {
        let event = create_test_event(30023, vec![vec!["d".to_string(), "my-article".to_string()]]);
        assert!(validate_addressable_event(&event).is_ok());
    }

    #[test]
    fn test_validate_addressable_event_missing_d_tag() {
        let event = create_test_event(30023, vec![]);
        assert!(validate_addressable_event(&event).is_err());
    }

    #[test]
    fn test_validate_addressable_event_not_addressable() {
        let event = create_test_event(1, vec![]);
        // Should be OK because it's not an addressable event
        assert!(validate_addressable_event(&event).is_ok());
    }
}
