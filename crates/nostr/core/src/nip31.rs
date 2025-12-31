//! NIP-31: Dealing with Unknown Event Kinds
//!
//! This NIP defines the `alt` tag for providing human-readable summaries of custom
//! event kinds. This enables graceful degradation when clients encounter unknown events.
//!
//! ## Purpose
//!
//! When creating custom event kinds that aren't meant to be read as text (like kind:1),
//! the `alt` tag provides context for clients that don't understand the event kind.
//!
//! ## Use Cases
//!
//! - Custom protocol events appearing in kind:1-centric timelines
//! - References to unknown events in user notes
//! - Graceful degradation for unsupported event types
//!
//! ## Examples
//!
//! ```
//! use nostr::nip31;
//!
//! // Add alt tag to custom event
//! let mut tags = Vec::new();
//! nip31::add_alt_tag(&mut tags, "User reacted with 👍 to a post");
//!
//! // Get alt tag from tags
//! let alt = nip31::get_alt_tag(&tags);
//! assert_eq!(alt, Some("User reacted with 👍 to a post"));
//! ```

use thiserror::Error;

/// Tag name for the alt tag.
pub const ALT_TAG: &str = "alt";

/// Errors that can occur during NIP-31 operations.
#[derive(Debug, Error)]
pub enum Nip31Error {
    #[error("alt tag summary cannot be empty")]
    EmptySummary,

    #[error("alt tag not found")]
    AltTagNotFound,
}

/// Get the alt tag value from event tags.
///
/// Returns the human-readable summary if present.
///
/// # Example
///
/// ```
/// use nostr::nip31;
///
/// let tags = vec![
///     vec!["alt".to_string(), "User reacted with 👍".to_string()],
/// ];
/// let alt = nip31::get_alt_tag(&tags);
/// assert_eq!(alt, Some("User reacted with 👍"));
/// ```
pub fn get_alt_tag(tags: &[Vec<String>]) -> Option<&str> {
    tags.iter()
        .find(|tag| tag.len() >= 2 && tag[0] == ALT_TAG)
        .map(|tag| tag[1].as_str())
}

/// Check if tags contain an alt tag.
///
/// # Example
///
/// ```
/// use nostr::nip31;
///
/// let tags = vec![
///     vec!["alt".to_string(), "Summary".to_string()],
/// ];
/// assert!(nip31::has_alt_tag(&tags));
/// ```
pub fn has_alt_tag(tags: &[Vec<String>]) -> bool {
    get_alt_tag(tags).is_some()
}

/// Add an alt tag to event tags.
///
/// # Example
///
/// ```
/// use nostr::nip31;
///
/// let mut tags = Vec::new();
/// nip31::add_alt_tag(&mut tags, "User created a calendar event");
/// assert_eq!(tags.len(), 1);
/// assert_eq!(tags[0][0], "alt");
/// ```
pub fn add_alt_tag(tags: &mut Vec<Vec<String>>, summary: impl Into<String>) {
    tags.push(vec![ALT_TAG.to_string(), summary.into()]);
}

/// Set or update the alt tag in event tags.
///
/// Removes existing alt tags and adds a new one.
///
/// # Example
///
/// ```
/// use nostr::nip31;
///
/// let mut tags = vec![
///     vec!["alt".to_string(), "Old summary".to_string()],
/// ];
/// nip31::set_alt_tag(&mut tags, "New summary");
/// assert_eq!(nip31::get_alt_tag(&tags), Some("New summary"));
/// ```
pub fn set_alt_tag(tags: &mut Vec<Vec<String>>, summary: impl Into<String>) {
    remove_alt_tag(tags);
    add_alt_tag(tags, summary);
}

/// Remove alt tag from event tags.
///
/// # Example
///
/// ```
/// use nostr::nip31;
///
/// let mut tags = vec![
///     vec!["alt".to_string(), "Summary".to_string()],
///     vec!["other".to_string(), "tag".to_string()],
/// ];
/// nip31::remove_alt_tag(&mut tags);
/// assert!(!nip31::has_alt_tag(&tags));
/// assert_eq!(tags.len(), 1);
/// ```
pub fn remove_alt_tag(tags: &mut Vec<Vec<String>>) {
    tags.retain(|tag| tag.is_empty() || tag[0] != ALT_TAG);
}

/// Validate an alt tag summary.
///
/// The summary should not be empty.
pub fn validate_alt_summary(summary: &str) -> Result<(), Nip31Error> {
    if summary.trim().is_empty() {
        return Err(Nip31Error::EmptySummary);
    }
    Ok(())
}

/// Create a simple alt tag for common event kinds.
///
/// This helper generates basic summaries for well-known custom event kinds.
pub fn create_default_alt(kind: u64, action: &str) -> String {
    match kind {
        // Reactions
        7 => format!("User reacted with {}", action),
        // Reposts
        6 | 16 => "User reposted a note".to_string(),
        // Zaps
        9735 => "User sent a Lightning Zap".to_string(),
        // Badges
        30008 => "User created a badge".to_string(),
        8 => "User awarded a badge".to_string(),
        // Calendar
        31922 => "User created a calendar event".to_string(),
        31923 => "User created a time-based event".to_string(),
        // Lists
        30000..=30009 => "User created a list".to_string(),
        // Live events
        30311 => "User started a live stream".to_string(),
        // Communities
        34550 => "User created a community".to_string(),
        // Default
        _ => format!("Custom event (kind {})", kind),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_alt_tag() {
        let tags = vec![
            vec!["alt".to_string(), "User reacted with 👍".to_string()],
            vec!["other".to_string(), "tag".to_string()],
        ];

        let alt = get_alt_tag(&tags);
        assert_eq!(alt, Some("User reacted with 👍"));
    }

    #[test]
    fn test_get_alt_tag_not_found() {
        let tags = vec![vec!["other".to_string(), "tag".to_string()]];

        let alt = get_alt_tag(&tags);
        assert_eq!(alt, None);
    }

    #[test]
    fn test_has_alt_tag() {
        let tags = vec![vec!["alt".to_string(), "Summary".to_string()]];
        assert!(has_alt_tag(&tags));

        let tags = vec![vec!["other".to_string(), "tag".to_string()]];
        assert!(!has_alt_tag(&tags));
    }

    #[test]
    fn test_add_alt_tag() {
        let mut tags = Vec::new();
        add_alt_tag(&mut tags, "User created an event");

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0][0], "alt");
        assert_eq!(tags[0][1], "User created an event");
    }

    #[test]
    fn test_set_alt_tag() {
        let mut tags = vec![vec!["alt".to_string(), "Old summary".to_string()]];

        set_alt_tag(&mut tags, "New summary");

        assert_eq!(get_alt_tag(&tags), Some("New summary"));
        assert_eq!(tags.len(), 1);
    }

    #[test]
    fn test_remove_alt_tag() {
        let mut tags = vec![
            vec!["alt".to_string(), "Summary".to_string()],
            vec!["other".to_string(), "tag".to_string()],
        ];

        remove_alt_tag(&mut tags);

        assert!(!has_alt_tag(&tags));
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0][0], "other");
    }

    #[test]
    fn test_validate_alt_summary() {
        assert!(validate_alt_summary("Valid summary").is_ok());
        assert!(validate_alt_summary("User did something").is_ok());

        assert!(validate_alt_summary("").is_err());
        assert!(validate_alt_summary("   ").is_err());
    }

    #[test]
    fn test_create_default_alt_reaction() {
        let alt = create_default_alt(7, "👍");
        assert_eq!(alt, "User reacted with 👍");
    }

    #[test]
    fn test_create_default_alt_repost() {
        let alt = create_default_alt(6, "");
        assert_eq!(alt, "User reposted a note");

        let alt = create_default_alt(16, "");
        assert_eq!(alt, "User reposted a note");
    }

    #[test]
    fn test_create_default_alt_zap() {
        let alt = create_default_alt(9735, "");
        assert_eq!(alt, "User sent a Lightning Zap");
    }

    #[test]
    fn test_create_default_alt_badge() {
        let alt = create_default_alt(30008, "");
        assert_eq!(alt, "User created a badge");

        let alt = create_default_alt(8, "");
        assert_eq!(alt, "User awarded a badge");
    }

    #[test]
    fn test_create_default_alt_calendar() {
        let alt = create_default_alt(31922, "");
        assert_eq!(alt, "User created a calendar event");

        let alt = create_default_alt(31923, "");
        assert_eq!(alt, "User created a time-based event");
    }

    #[test]
    fn test_create_default_alt_list() {
        let alt = create_default_alt(30000, "");
        assert_eq!(alt, "User created a list");

        let alt = create_default_alt(30005, "");
        assert_eq!(alt, "User created a list");
    }

    #[test]
    fn test_create_default_alt_live() {
        let alt = create_default_alt(30311, "");
        assert_eq!(alt, "User started a live stream");
    }

    #[test]
    fn test_create_default_alt_community() {
        let alt = create_default_alt(34550, "");
        assert_eq!(alt, "User created a community");
    }

    #[test]
    fn test_create_default_alt_unknown() {
        let alt = create_default_alt(99999, "");
        assert_eq!(alt, "Custom event (kind 99999)");
    }

    #[test]
    fn test_alt_tag_with_emoji() {
        let mut tags = Vec::new();
        add_alt_tag(&mut tags, "User reacted with 🎉");

        let alt = get_alt_tag(&tags);
        assert_eq!(alt, Some("User reacted with 🎉"));
    }

    #[test]
    fn test_alt_tag_multiline() {
        let mut tags = Vec::new();
        let summary = "User created a long-form article.\nTitle: My First Post";
        add_alt_tag(&mut tags, summary);

        let alt = get_alt_tag(&tags);
        assert_eq!(alt, Some(summary));
    }

    #[test]
    fn test_multiple_alt_tags() {
        // Should only get the first one
        let tags = vec![
            vec!["alt".to_string(), "First summary".to_string()],
            vec!["alt".to_string(), "Second summary".to_string()],
        ];

        let alt = get_alt_tag(&tags);
        assert_eq!(alt, Some("First summary"));
    }

    #[test]
    fn test_set_alt_tag_removes_duplicates() {
        let mut tags = vec![
            vec!["alt".to_string(), "First".to_string()],
            vec!["other".to_string(), "tag".to_string()],
            vec!["alt".to_string(), "Second".to_string()],
        ];

        set_alt_tag(&mut tags, "New");

        // Should have only one alt tag
        let alt_count = tags
            .iter()
            .filter(|t| !t.is_empty() && t[0] == "alt")
            .count();
        assert_eq!(alt_count, 1);
        assert_eq!(get_alt_tag(&tags), Some("New"));
    }
}
