//! NIP-27: Text Note References
//!
//! This module implements NIP-27, which standardizes how clients handle inline references
//! to other events and profiles within the `.content` field of text-based events.
//!
//! ## How It Works
//!
//! When creating an event, clients include mentions to other profiles and events using
//! NIP-21 `nostr:` URIs embedded directly in the content. For example:
//!
//! ```text
//! Check out this profile nostr:nprofile1... and this note nostr:note1...
//! ```
//!
//! Optionally, clients can include corresponding NIP-10 tags (e.g., `["p", <hex-id>]` or
//! `["e", <hex-id>]`) for notification purposes or to mark the event as a reply.
//!
//! ## Reader Client Behavior
//!
//! When a reader client receives an event with `nostr:` mentions in the content, it can:
//! - Link to the referenced profile or event
//! - Show a preview of the mentioned content
//! - Display it as `@username` or similar
//! - Apply any other desired context augmentation
//!
//! ## Example
//!
//! ```
//! use nostr_core::nip27::{extract_references, MentionReference};
//!
//! let content = "Hey nostr:npub1... check out nostr:note1...";
//! let refs = extract_references(content);
//!
//! for mention in refs {
//!     println!("Found mention at position {}: {:?}", mention.start, mention.entity);
//! }
//! ```

use crate::nip19::Nip19Entity;
use crate::nip21::from_nostr_uri;
use thiserror::Error;

/// Errors that can occur during NIP-27 operations.
#[derive(Debug, Error)]
pub enum Nip27Error {
    #[error("NIP-21 URI parsing error: {0}")]
    Nip21(#[from] crate::nip21::Nip21Error),
}

/// A reference to another entity (profile or event) found in text content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionReference {
    /// The decoded NIP-19 entity (pubkey, note, profile, event, or address)
    pub entity: Nip19Entity,
    /// The original `nostr:` URI string found in the content
    pub uri: String,
    /// The byte position where this reference starts in the content
    pub start: usize,
    /// The byte position where this reference ends in the content
    pub end: usize,
}

/// Extract all `nostr:` URI references from text content.
///
/// This function scans the content for `nostr:` URIs and attempts to parse each one.
/// Invalid URIs are silently skipped.
///
/// # Arguments
///
/// * `content` - The text content to scan for references
///
/// # Returns
///
/// A vector of `MentionReference` structs, one for each valid `nostr:` URI found.
///
/// # Example
///
/// ```
/// use nostr_core::nip27::extract_references;
///
/// let content = "Check out nostr:npub1... and nostr:note1...";
/// let refs = extract_references(content);
/// assert_eq!(refs.len(), 2);
/// ```
pub fn extract_references(content: &str) -> Vec<MentionReference> {
    let mut references = Vec::new();
    let mut search_from = 0;

    while let Some(start) = content[search_from..].find("nostr:") {
        let abs_start = search_from + start;

        // Find the end of the URI (next whitespace or end of string)
        let remaining = &content[abs_start..];
        let end_offset = remaining
            .find(|c: char| c.is_whitespace())
            .unwrap_or(remaining.len());

        let abs_end = abs_start + end_offset;
        let uri = &content[abs_start..abs_end];

        // Try to parse the URI
        if let Ok(entity) = from_nostr_uri(uri) {
            references.push(MentionReference {
                entity,
                uri: uri.to_string(),
                start: abs_start,
                end: abs_end,
            });
        }

        // Continue searching after this URI
        search_from = abs_end;
    }

    references
}

/// Check if the content contains any `nostr:` URI references.
///
/// This is a quick check that doesn't validate the URIs, just looks for the `nostr:` prefix.
///
/// # Example
///
/// ```
/// use nostr_core::nip27::has_references;
///
/// assert!(has_references("Check out nostr:npub1..."));
/// assert!(!has_references("Just plain text"));
/// ```
pub fn has_references(content: &str) -> bool {
    content.contains("nostr:")
}

/// Extract profile references (npub or nprofile) from content.
///
/// # Example
///
/// ```
/// use nostr_core::nip27::extract_profile_references;
///
/// let content = "Hey nostr:npub1... and nostr:note1...";
/// let profiles = extract_profile_references(content);
/// // Will only include the npub, not the note
/// ```
pub fn extract_profile_references(content: &str) -> Vec<MentionReference> {
    extract_references(content)
        .into_iter()
        .filter(|r| matches!(r.entity, Nip19Entity::Pubkey(_) | Nip19Entity::Profile(_)))
        .collect()
}

/// Extract event references (note, nevent, or naddr) from content.
///
/// # Example
///
/// ```
/// use nostr_core::nip27::extract_event_references;
///
/// let content = "Hey nostr:npub1... and nostr:note1...";
/// let events = extract_event_references(content);
/// // Will only include the note, not the npub
/// ```
pub fn extract_event_references(content: &str) -> Vec<MentionReference> {
    extract_references(content)
        .into_iter()
        .filter(|r| {
            matches!(
                r.entity,
                Nip19Entity::Note(_) | Nip19Entity::Event(_) | Nip19Entity::Address(_)
            )
        })
        .collect()
}

/// Get public keys from profile references.
///
/// Extracts the public key bytes from npub and nprofile entities found in the content.
///
/// # Example
///
/// ```
/// use nostr_core::nip27::get_mentioned_pubkeys;
///
/// let content = "Hey nostr:npub1... check this out!";
/// let pubkeys = get_mentioned_pubkeys(content);
/// ```
pub fn get_mentioned_pubkeys(content: &str) -> Vec<[u8; 32]> {
    extract_profile_references(content)
        .into_iter()
        .map(|r| match r.entity {
            Nip19Entity::Pubkey(pk) => pk,
            Nip19Entity::Profile(p) => p.pubkey,
            _ => unreachable!(), // Filtered by extract_profile_references
        })
        .collect()
}

/// Get event IDs from event references.
///
/// Extracts event ID bytes from note and nevent entities found in the content.
/// Does not include naddr (addressable events) as they use a different identifier scheme.
///
/// # Example
///
/// ```
/// use nostr_core::nip27::get_mentioned_event_ids;
///
/// let content = "See nostr:note1... for details";
/// let event_ids = get_mentioned_event_ids(content);
/// ```
pub fn get_mentioned_event_ids(content: &str) -> Vec<[u8; 32]> {
    extract_event_references(content)
        .into_iter()
        .filter_map(|r| match r.entity {
            Nip19Entity::Note(id) => Some(id),
            Nip19Entity::Event(e) => Some(e.id),
            _ => None, // Skip naddr
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nip19::{ProfilePointer, encode_note, encode_nprofile, encode_npub};

    #[test]
    fn test_extract_single_npub() {
        let pubkey = [1u8; 32];
        let npub = encode_npub(&pubkey).unwrap();
        let content = format!("Check out nostr:{}", npub);

        let refs = extract_references(&content);
        assert_eq!(refs.len(), 1);
        assert!(matches!(refs[0].entity, Nip19Entity::Pubkey(_)));
        assert!(refs[0].uri.starts_with("nostr:npub"));
    }

    #[test]
    fn test_extract_multiple_references() {
        let pubkey = [2u8; 32];
        let note_id = [3u8; 32];
        let npub = encode_npub(&pubkey).unwrap();
        let note = encode_note(&note_id).unwrap();

        let content = format!("Hey nostr:{} check out nostr:{} awesome!", npub, note);

        let refs = extract_references(&content);
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn test_extract_with_whitespace() {
        let pubkey = [4u8; 32];
        let npub = encode_npub(&pubkey).unwrap();
        let content = format!("Start nostr:{} middle\nnostr:{} end", npub, npub);

        let refs = extract_references(&content);
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn test_has_references() {
        assert!(has_references("Check out nostr:npub1..."));
        assert!(has_references("nostr:note1..."));
        assert!(!has_references("No references here"));
        assert!(!has_references(""));
    }

    #[test]
    fn test_extract_profile_references() {
        let pubkey = [5u8; 32];
        let note_id = [6u8; 32];
        let npub = encode_npub(&pubkey).unwrap();
        let note = encode_note(&note_id).unwrap();

        let content = format!("Profile nostr:{} and note nostr:{}", npub, note);

        let profiles = extract_profile_references(&content);
        assert_eq!(profiles.len(), 1);
        assert!(matches!(profiles[0].entity, Nip19Entity::Pubkey(_)));
    }

    #[test]
    fn test_extract_event_references() {
        let pubkey = [7u8; 32];
        let note_id = [8u8; 32];
        let npub = encode_npub(&pubkey).unwrap();
        let note = encode_note(&note_id).unwrap();

        let content = format!("Profile nostr:{} and note nostr:{}", npub, note);

        let events = extract_event_references(&content);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].entity, Nip19Entity::Note(_)));
    }

    #[test]
    fn test_get_mentioned_pubkeys() {
        let pubkey1 = [9u8; 32];
        let pubkey2 = [10u8; 32];
        let npub1 = encode_npub(&pubkey1).unwrap();

        let profile2 = ProfilePointer {
            pubkey: pubkey2,
            relays: vec!["wss://relay.example.com".to_string()],
        };
        let nprofile2 = encode_nprofile(&profile2).unwrap();

        let content = format!("Check nostr:{} and nostr:{}", npub1, nprofile2);

        let pubkeys = get_mentioned_pubkeys(&content);
        assert_eq!(pubkeys.len(), 2);
        assert!(pubkeys.contains(&pubkey1));
        assert!(pubkeys.contains(&pubkey2));
    }

    #[test]
    fn test_get_mentioned_event_ids() {
        let note_id = [11u8; 32];
        let note = encode_note(&note_id).unwrap();
        let content = format!("See nostr:{} for details", note);

        let event_ids = get_mentioned_event_ids(&content);
        assert_eq!(event_ids.len(), 1);
        assert_eq!(event_ids[0], note_id);
    }

    #[test]
    fn test_empty_content() {
        let refs = extract_references("");
        assert_eq!(refs.len(), 0);

        assert!(!has_references(""));
    }

    #[test]
    fn test_malformed_uri_skipped() {
        // Invalid bech32 should be skipped
        let content = "Check nostr:invalid_bech32_xxx and continue";
        let refs = extract_references(content);
        assert_eq!(refs.len(), 0);
    }

    #[test]
    fn test_reference_positions() {
        let pubkey = [12u8; 32];
        let npub = encode_npub(&pubkey).unwrap();
        let uri = format!("nostr:{}", npub);
        let content = format!("Start {} end", uri);

        let refs = extract_references(&content);
        assert_eq!(refs.len(), 1);

        // Check that positions are correct
        assert_eq!(refs[0].start, 6); // After "Start "
        assert_eq!(&content[refs[0].start..refs[0].end], uri);
    }

    #[test]
    fn test_uri_at_end_of_string() {
        let pubkey = [13u8; 32];
        let npub = encode_npub(&pubkey).unwrap();
        let content = format!("Check this out nostr:{}", npub);

        let refs = extract_references(&content);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].end, content.len());
    }
}
