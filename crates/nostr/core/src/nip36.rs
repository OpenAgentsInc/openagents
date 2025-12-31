//! NIP-36: Sensitive Content / Content Warning
//!
//! This module implements NIP-36, which enables users to mark event content as sensitive,
//! requiring reader approval before display. Clients can hide such content until users
//! explicitly choose to view it.
//!
//! ## How It Works
//!
//! Events can include a `content-warning` tag with an optional reason:
//!
//! ```json
//! {
//!   "tags": [
//!     ["content-warning", "reason for warning"]
//!   ]
//! }
//! ```
//!
//! The tag can also be used without a reason:
//!
//! ```json
//! {
//!   "tags": [
//!     ["content-warning"]
//!   ]
//! }
//! ```
//!
//! ## Extended Labeling (NIP-32)
//!
//! NIP-36 can be combined with NIP-32 labels for more structured categorization:
//!
//! ```json
//! {
//!   "tags": [
//!     ["content-warning"],
//!     ["L", "social.nos.ontology"],
//!     ["l", "NS-nud", "social.nos.ontology"]
//!   ]
//! }
//! ```
//!
//! ## Example
//!
//! ```
//! use nostr_core::nip36::{add_content_warning, get_content_warning, has_content_warning};
//! use nostr_core::Event;
//!
//! # fn example(event: &Event) {
//! // Check if event has content warning
//! if has_content_warning(event) {
//!     if let Some(reason) = get_content_warning(event) {
//!         println!("Content warning: {}", reason);
//!     } else {
//!         println!("Content warning (no reason specified)");
//!     }
//! }
//! # }
//! ```

use crate::nip01::Event;
use thiserror::Error;

/// The tag name used for content warnings
pub const CONTENT_WARNING_TAG: &str = "content-warning";

/// Errors that can occur during NIP-36 operations.
#[derive(Debug, Error)]
pub enum Nip36Error {
    #[error("invalid content warning format")]
    InvalidFormat,
}

/// Check if an event has a content warning tag.
///
/// # Arguments
///
/// * `event` - The event to check
///
/// # Returns
///
/// Returns `true` if the event has a `content-warning` tag.
///
/// # Example
///
/// ```
/// use nostr_core::nip36::has_content_warning;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if has_content_warning(event) {
///     println!("This content may be sensitive");
/// }
/// # }
/// ```
pub fn has_content_warning(event: &Event) -> bool {
    event
        .tags
        .iter()
        .any(|tag| tag.get(0).map(|s| s.as_str()) == Some(CONTENT_WARNING_TAG))
}

/// Get the content warning reason from an event.
///
/// Returns `None` if:
/// - The event has no content warning tag
/// - The content warning tag has no reason specified
///
/// Returns `Some(String)` with the reason if specified.
///
/// # Arguments
///
/// * `event` - The event to check
///
/// # Example
///
/// ```
/// use nostr_core::nip36::get_content_warning;
/// # use nostr_core::Event;
/// # fn example(event: &Event) {
/// if let Some(reason) = get_content_warning(event) {
///     println!("Warning reason: {}", reason);
/// }
/// # }
/// ```
pub fn get_content_warning(event: &Event) -> Option<String> {
    event
        .tags
        .iter()
        .find(|tag| tag.get(0).map(|s| s.as_str()) == Some(CONTENT_WARNING_TAG))
        .and_then(|tag| tag.get(1))
        .map(|s| s.to_string())
}

/// Add a content warning tag to event tags.
///
/// This is a helper function to add a `content-warning` tag to event tags.
/// If a reason is provided, it will be included as the second element of the tag.
///
/// # Arguments
///
/// * `tags` - The mutable vector of tags to add the content warning to
/// * `reason` - Optional reason for the content warning
///
/// # Example
///
/// ```
/// use nostr_core::nip36::add_content_warning;
///
/// let mut tags: Vec<Vec<String>> = vec![];
/// add_content_warning(&mut tags, Some("nudity"));
/// ```
pub fn add_content_warning(tags: &mut Vec<Vec<String>>, reason: Option<&str>) {
    let mut tag = vec![CONTENT_WARNING_TAG.to_string()];
    if let Some(r) = reason {
        tag.push(r.to_string());
    }
    tags.push(tag);
}

/// Remove content warning tags from event tags.
///
/// # Arguments
///
/// * `tags` - The mutable vector of tags to remove content warnings from
///
/// # Example
///
/// ```
/// use nostr_core::nip36::remove_content_warning;
///
/// let mut tags = vec![
///     vec!["content-warning".to_string(), "nsfw".to_string()],
///     vec!["p".to_string(), "pubkey".to_string()],
/// ];
/// remove_content_warning(&mut tags);
/// // Only the "p" tag remains
/// ```
pub fn remove_content_warning(tags: &mut Vec<Vec<String>>) {
    tags.retain(|tag| tag.get(0).map(|s| s.as_str()) != Some(CONTENT_WARNING_TAG));
}

/// Common content warning reasons.
///
/// These are suggested reasons based on common usage patterns, but any string
/// can be used as a reason.
pub mod reasons {
    /// Nudity or sexually explicit content
    pub const NUDITY: &str = "nudity";

    /// Profane or offensive language
    pub const PROFANITY: &str = "profanity";

    /// Violent or disturbing content
    pub const VIOLENCE: &str = "violence";

    /// Graphic or disturbing imagery
    pub const GRAPHIC: &str = "graphic";

    /// Sexual content (not necessarily nudity)
    pub const SEXUAL: &str = "sexual";

    /// General NSFW (Not Safe For Work)
    pub const NSFW: &str = "nsfw";

    /// Potentially triggering content
    pub const TRIGGER: &str = "trigger";

    /// Spoilers for media content
    pub const SPOILER: &str = "spoiler";
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
            content: "test content".to_string(),
            sig: "0".repeat(128),
        }
    }

    #[test]
    fn test_has_content_warning_true() {
        let event = create_test_event(vec![vec!["content-warning".to_string()]]);
        assert!(has_content_warning(&event));
    }

    #[test]
    fn test_has_content_warning_false() {
        let event = create_test_event(vec![]);
        assert!(!has_content_warning(&event));
    }

    #[test]
    fn test_has_content_warning_with_reason() {
        let event = create_test_event(vec![vec![
            "content-warning".to_string(),
            "nudity".to_string(),
        ]]);
        assert!(has_content_warning(&event));
    }

    #[test]
    fn test_get_content_warning_with_reason() {
        let event = create_test_event(vec![vec![
            "content-warning".to_string(),
            "violence".to_string(),
        ]]);
        assert_eq!(get_content_warning(&event), Some("violence".to_string()));
    }

    #[test]
    fn test_get_content_warning_without_reason() {
        let event = create_test_event(vec![vec!["content-warning".to_string()]]);
        assert_eq!(get_content_warning(&event), None);
    }

    #[test]
    fn test_get_content_warning_no_tag() {
        let event = create_test_event(vec![]);
        assert_eq!(get_content_warning(&event), None);
    }

    #[test]
    fn test_add_content_warning_with_reason() {
        let mut tags = vec![];
        add_content_warning(&mut tags, Some("nsfw"));

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0][0], "content-warning");
        assert_eq!(tags[0][1], "nsfw");
    }

    #[test]
    fn test_add_content_warning_without_reason() {
        let mut tags = vec![];
        add_content_warning(&mut tags, None);

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0][0], "content-warning");
        assert_eq!(tags[0].len(), 1);
    }

    #[test]
    fn test_remove_content_warning() {
        let mut tags = vec![
            vec!["content-warning".to_string(), "nsfw".to_string()],
            vec!["p".to_string(), "pubkey".to_string()],
            vec!["content-warning".to_string()],
        ];

        remove_content_warning(&mut tags);

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0][0], "p");
    }

    #[test]
    fn test_multiple_tags() {
        let event = create_test_event(vec![
            vec!["p".to_string(), "pubkey".to_string()],
            vec!["content-warning".to_string(), "spoiler".to_string()],
            vec!["e".to_string(), "eventid".to_string()],
        ]);

        assert!(has_content_warning(&event));
        assert_eq!(get_content_warning(&event), Some("spoiler".to_string()));
    }

    #[test]
    fn test_reasons_constants() {
        // Test that reason constants are defined
        assert_eq!(reasons::NUDITY, "nudity");
        assert_eq!(reasons::PROFANITY, "profanity");
        assert_eq!(reasons::VIOLENCE, "violence");
        assert_eq!(reasons::GRAPHIC, "graphic");
        assert_eq!(reasons::SEXUAL, "sexual");
        assert_eq!(reasons::NSFW, "nsfw");
        assert_eq!(reasons::TRIGGER, "trigger");
        assert_eq!(reasons::SPOILER, "spoiler");
    }

    #[test]
    fn test_using_reason_constant() {
        let mut tags = vec![];
        add_content_warning(&mut tags, Some(reasons::NUDITY));

        let event = create_test_event(tags);
        assert_eq!(get_content_warning(&event), Some("nudity".to_string()));
    }

    #[test]
    fn test_custom_reason() {
        let mut tags = vec![];
        add_content_warning(&mut tags, Some("custom warning reason"));

        let event = create_test_event(tags);
        assert_eq!(
            get_content_warning(&event),
            Some("custom warning reason".to_string())
        );
    }

    #[test]
    fn test_empty_reason() {
        let mut tags = vec![];
        add_content_warning(&mut tags, Some(""));

        let event = create_test_event(tags);
        assert_eq!(get_content_warning(&event), Some("".to_string()));
    }
}
