//! NIP-08: Handling Mentions
//!
//! This module implements NIP-08 which standardizes inline mentions of events and pubkeys
//! using the #[index] notation that references the tags array.
//!
//! **Warning**: This NIP is deprecated in favor of NIP-27 (Text Note References), but is still
//! used by some clients and may be encountered in the wild.
//!
//! # Overview
//!
//! NIP-08 defines how clients should:
//! - Replace mentions of events/pubkeys with #[index] notation pointing to tags array
//! - Parse #[index] references back to actual pubkeys/event IDs
//! - Validate index bounds and tag types
//!
//! # Example
//!
//! ```
//! use nostr::nip08::{parse_mention, format_mention, replace_mentions, extract_mentions};
//!
//! // Parse a mention from content
//! let mention = parse_mention("#[0]").unwrap();
//! assert_eq!(mention.index, 0);
//!
//! // Format a mention
//! let formatted = format_mention(0);
//! assert_eq!(formatted, "#[0]");
//!
//! // Extract all mentions from content
//! let content = "Hello #[0] and #[1]!";
//! let mentions = extract_mentions(content);
//! assert_eq!(mentions, vec![0, 1]);
//!
//! // Replace mentions with actual values
//! let tags = vec![
//!     vec!["p".to_string(), "pubkey123".to_string()],
//!     vec!["e".to_string(), "event456".to_string()],
//! ];
//! let replaced = replace_mentions(content, &tags);
//! assert_eq!(replaced, "Hello pubkey123 and event456!");
//! ```

use thiserror::Error;

/// NIP-08 error types
#[derive(Debug, Error, Clone, PartialEq)]
pub enum Nip08Error {
    /// Index is out of bounds
    #[error("mention index {0} is out of bounds (tags array has {1} elements)")]
    IndexOutOfBounds(usize, usize),

    /// Tag at index is not a p or e tag
    #[error("tag at index {0} is not a p or e tag (found: {1})")]
    InvalidTagType(usize, String),

    /// Tag at index is missing the required second element
    #[error("tag at index {0} is missing the required value")]
    MissingTagValue(usize),

    /// Invalid mention format
    #[error("invalid mention format: {0}")]
    InvalidFormat(String),

    /// Failed to parse mention index
    #[error("failed to parse mention index: {0}")]
    ParseError(String),
}

/// Represents a parsed mention reference
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    /// The index into the tags array
    pub index: usize,
    /// The original text of the mention (e.g., "#[0]")
    pub original: String,
    /// The position in the content where this mention starts
    pub position: usize,
}

/// Parse a single mention from text
///
/// # Example
///
/// ```
/// use nostr::nip08::parse_mention;
///
/// let mention = parse_mention("#[0]").unwrap();
/// assert_eq!(mention.index, 0);
/// assert_eq!(mention.original, "#[0]");
/// ```
pub fn parse_mention(text: &str) -> Result<Mention, Nip08Error> {
    if !text.starts_with("#[") || !text.ends_with(']') {
        return Err(Nip08Error::InvalidFormat(text.to_string()));
    }

    let index_str = &text[2..text.len() - 1];
    let index = index_str
        .parse::<usize>()
        .map_err(|e| Nip08Error::ParseError(e.to_string()))?;

    Ok(Mention {
        index,
        original: text.to_string(),
        position: 0, // Will be set by extract_mentions_detailed
    })
}

/// Format a mention from an index
///
/// # Example
///
/// ```
/// use nostr::nip08::format_mention;
///
/// assert_eq!(format_mention(0), "#[0]");
/// assert_eq!(format_mention(42), "#[42]");
/// ```
pub fn format_mention(index: usize) -> String {
    format!("#[{}]", index)
}

/// Extract all mention indices from content
///
/// Returns the indices in the order they appear in the content.
///
/// # Example
///
/// ```
/// use nostr::nip08::extract_mentions;
///
/// let content = "Hello #[0] and #[1]! Also #[0] again.";
/// let mentions = extract_mentions(content);
/// assert_eq!(mentions, vec![0, 1, 0]);
/// ```
pub fn extract_mentions(content: &str) -> Vec<usize> {
    extract_mentions_detailed(content)
        .iter()
        .map(|m| m.index)
        .collect()
}

/// Extract all mentions from content with full details
///
/// Returns all mentions with their positions and original text.
///
/// # Example
///
/// ```
/// use nostr::nip08::extract_mentions_detailed;
///
/// let content = "Hello #[0] and #[1]!";
/// let mentions = extract_mentions_detailed(content);
/// assert_eq!(mentions.len(), 2);
/// assert_eq!(mentions[0].index, 0);
/// assert_eq!(mentions[1].index, 1);
/// ```
pub fn extract_mentions_detailed(content: &str) -> Vec<Mention> {
    let mut mentions = Vec::new();
    let mut chars = content.char_indices().peekable();

    while let Some((pos, ch)) = chars.next() {
        if ch == '#'
            && let Some(&(_, '[')) = chars.peek()
        {
            chars.next(); // consume '['

            // Collect digits
            let mut index_str = String::new();
            let mut found_close = false;

            while let Some(&(_, ch)) = chars.peek() {
                if ch.is_ascii_digit() {
                    index_str.push(ch);
                    chars.next();
                } else if ch == ']' {
                    found_close = true;
                    chars.next();
                    break;
                } else {
                    break;
                }
            }

            if found_close
                && !index_str.is_empty()
                && let Ok(index) = index_str.parse::<usize>()
            {
                let original = format!("#[{}]", index);
                mentions.push(Mention {
                    index,
                    original,
                    position: pos,
                });
            }
        }
    }

    mentions
}

/// Validate that a mention index is valid for the given tags array
///
/// Returns Ok(()) if the mention is valid (points to a p or e tag with a value),
/// otherwise returns an appropriate error.
///
/// # Example
///
/// ```
/// use nostr::nip08::validate_mention;
///
/// let tags = vec![
///     vec!["p".to_string(), "pubkey123".to_string()],
///     vec!["e".to_string(), "event456".to_string()],
/// ];
///
/// assert!(validate_mention(0, &tags).is_ok());
/// assert!(validate_mention(1, &tags).is_ok());
/// assert!(validate_mention(2, &tags).is_err()); // out of bounds
/// ```
pub fn validate_mention(index: usize, tags: &[Vec<String>]) -> Result<(), Nip08Error> {
    // Check bounds
    if index >= tags.len() {
        return Err(Nip08Error::IndexOutOfBounds(index, tags.len()));
    }

    let tag = &tags[index];

    // Check if tag is empty
    if tag.is_empty() {
        return Err(Nip08Error::InvalidTagType(index, "<empty>".to_string()));
    }

    // Check if tag is p or e
    let tag_type = &tag[0];
    if tag_type != "p" && tag_type != "e" {
        return Err(Nip08Error::InvalidTagType(index, tag_type.clone()));
    }

    // Check if tag has a value
    if tag.len() < 2 {
        return Err(Nip08Error::MissingTagValue(index));
    }

    Ok(())
}

/// Get the value from a tag at the given index
///
/// Returns the pubkey/event ID if the tag is valid, or None if invalid.
///
/// # Example
///
/// ```
/// use nostr::nip08::get_mention_value;
///
/// let tags = vec![
///     vec!["p".to_string(), "pubkey123".to_string()],
///     vec!["e".to_string(), "event456".to_string()],
/// ];
///
/// assert_eq!(get_mention_value(0, &tags), Some("pubkey123".to_string()));
/// assert_eq!(get_mention_value(1, &tags), Some("event456".to_string()));
/// assert_eq!(get_mention_value(2, &tags), None); // out of bounds
/// ```
pub fn get_mention_value(index: usize, tags: &[Vec<String>]) -> Option<String> {
    validate_mention(index, tags)
        .ok()
        .map(|_| tags[index][1].clone())
}

/// Replace all mentions in content with their actual values from tags
///
/// Invalid mentions (out of bounds or non-p/e tags) are left as-is.
///
/// # Example
///
/// ```
/// use nostr::nip08::replace_mentions;
///
/// let content = "Hello #[0] and #[1]!";
/// let tags = vec![
///     vec!["p".to_string(), "alice".to_string()],
///     vec!["e".to_string(), "event123".to_string()],
/// ];
///
/// let replaced = replace_mentions(content, &tags);
/// assert_eq!(replaced, "Hello alice and event123!");
/// ```
pub fn replace_mentions(content: &str, tags: &[Vec<String>]) -> String {
    let mentions = extract_mentions_detailed(content);

    // Sort mentions by position in reverse order so we can replace from end to start
    let mut mentions = mentions;
    mentions.sort_by(|a, b| b.position.cmp(&a.position));

    let mut result = content.to_string();

    for mention in mentions {
        if let Some(value) = get_mention_value(mention.index, tags) {
            let start = mention.position;
            let end = start + mention.original.len();
            result.replace_range(start..end, &value);
        }
    }

    result
}

/// Create tags array from mentioned pubkeys and event IDs
///
/// This is the reverse operation - given a list of pubkeys and event IDs,
/// create the tags array that would be used in the event.
///
/// Returns (tags, content) where content has mentions replaced with #[index] notation.
///
/// # Example
///
/// ```
/// use nostr::nip08::create_tags_from_mentions;
///
/// let pubkeys = vec!["alice".to_string(), "bob".to_string()];
/// let event_ids = vec!["event123".to_string()];
///
/// let (tags, _content) = create_tags_from_mentions(&pubkeys, &event_ids);
/// assert_eq!(tags.len(), 3);
/// assert_eq!(tags[0], vec!["p", "alice"]);
/// assert_eq!(tags[1], vec!["p", "bob"]);
/// assert_eq!(tags[2], vec!["e", "event123"]);
/// ```
pub fn create_tags_from_mentions(
    pubkeys: &[String],
    event_ids: &[String],
) -> (Vec<Vec<String>>, String) {
    let mut tags = Vec::new();

    // Add p tags for pubkeys
    for pubkey in pubkeys {
        tags.push(vec!["p".to_string(), pubkey.clone()]);
    }

    // Add e tags for event IDs
    for event_id in event_ids {
        tags.push(vec!["e".to_string(), event_id.clone()]);
    }

    // Create content with #[index] references
    let mut content_parts = Vec::new();
    for i in 0..tags.len() {
        content_parts.push(format_mention(i));
    }
    let content = content_parts.join(" ");

    (tags, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mention() {
        let mention = parse_mention("#[0]").unwrap();
        assert_eq!(mention.index, 0);
        assert_eq!(mention.original, "#[0]");

        let mention = parse_mention("#[42]").unwrap();
        assert_eq!(mention.index, 42);
        assert_eq!(mention.original, "#[42]");

        assert!(parse_mention("#0]").is_err());
        assert!(parse_mention("#[0").is_err());
        assert!(parse_mention("#[]").is_err());
        assert!(parse_mention("notamention").is_err());
    }

    #[test]
    fn test_format_mention() {
        assert_eq!(format_mention(0), "#[0]");
        assert_eq!(format_mention(42), "#[42]");
        assert_eq!(format_mention(999), "#[999]");
    }

    #[test]
    fn test_extract_mentions() {
        let content = "Hello #[0] and #[1]!";
        let mentions = extract_mentions(content);
        assert_eq!(mentions, vec![0, 1]);

        let content = "Just #[0]";
        let mentions = extract_mentions(content);
        assert_eq!(mentions, vec![0]);

        let content = "No mentions here!";
        let mentions = extract_mentions(content);
        assert!(mentions.is_empty());

        let content = "Repeated #[0] and #[0] again";
        let mentions = extract_mentions(content);
        assert_eq!(mentions, vec![0, 0]);
    }

    #[test]
    fn test_extract_mentions_detailed() {
        let content = "Hello #[0] and #[1]!";
        let mentions = extract_mentions_detailed(content);

        assert_eq!(mentions.len(), 2);
        assert_eq!(mentions[0].index, 0);
        assert_eq!(mentions[0].position, 6);
        assert_eq!(mentions[1].index, 1);
        assert_eq!(mentions[1].position, 15);
    }

    #[test]
    fn test_validate_mention() {
        let tags = vec![
            vec!["p".to_string(), "pubkey123".to_string()],
            vec!["e".to_string(), "event456".to_string()],
            vec!["t".to_string(), "hashtag".to_string()],
        ];

        assert!(validate_mention(0, &tags).is_ok());
        assert!(validate_mention(1, &tags).is_ok());

        // Invalid: not a p or e tag
        assert!(matches!(
            validate_mention(2, &tags),
            Err(Nip08Error::InvalidTagType(2, _))
        ));

        // Invalid: out of bounds
        assert!(matches!(
            validate_mention(3, &tags),
            Err(Nip08Error::IndexOutOfBounds(3, 3))
        ));
    }

    #[test]
    fn test_get_mention_value() {
        let tags = vec![
            vec!["p".to_string(), "pubkey123".to_string()],
            vec!["e".to_string(), "event456".to_string()],
            vec!["t".to_string(), "hashtag".to_string()],
        ];

        assert_eq!(get_mention_value(0, &tags), Some("pubkey123".to_string()));
        assert_eq!(get_mention_value(1, &tags), Some("event456".to_string()));
        assert_eq!(get_mention_value(2, &tags), None); // not a p or e tag
        assert_eq!(get_mention_value(3, &tags), None); // out of bounds
    }

    #[test]
    fn test_replace_mentions() {
        let tags = vec![
            vec!["p".to_string(), "alice".to_string()],
            vec!["e".to_string(), "event123".to_string()],
            vec!["t".to_string(), "nostr".to_string()],
        ];

        let content = "Hello #[0] and #[1]!";
        let replaced = replace_mentions(content, &tags);
        assert_eq!(replaced, "Hello alice and event123!");

        // Invalid mention (t tag) should be left as-is
        let content = "Hello #[2]!";
        let replaced = replace_mentions(content, &tags);
        assert_eq!(replaced, "Hello #[2]!");

        // Out of bounds mention should be left as-is
        let content = "Hello #[99]!";
        let replaced = replace_mentions(content, &tags);
        assert_eq!(replaced, "Hello #[99]!");
    }

    #[test]
    fn test_create_tags_from_mentions() {
        let pubkeys = vec!["alice".to_string(), "bob".to_string()];
        let event_ids = vec!["event123".to_string()];

        let (tags, content) = create_tags_from_mentions(&pubkeys, &event_ids);

        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0], vec!["p", "alice"]);
        assert_eq!(tags[1], vec!["p", "bob"]);
        assert_eq!(tags[2], vec!["e", "event123"]);
        assert_eq!(content, "#[0] #[1] #[2]");
    }

    #[test]
    fn test_validate_mention_missing_value() {
        let tags = vec![
            vec!["p".to_string()], // Missing value
        ];

        assert!(matches!(
            validate_mention(0, &tags),
            Err(Nip08Error::MissingTagValue(0))
        ));
    }

    #[test]
    fn test_validate_mention_empty_tag() {
        let tags = vec![
            vec![], // Empty tag
        ];

        assert!(matches!(
            validate_mention(0, &tags),
            Err(Nip08Error::InvalidTagType(0, _))
        ));
    }

    #[test]
    fn test_extract_mentions_edge_cases() {
        // Malformed mentions should be ignored
        assert_eq!(extract_mentions("#["), Vec::<usize>::new());
        assert_eq!(extract_mentions("#[]"), Vec::<usize>::new());
        assert_eq!(extract_mentions("#[abc]"), Vec::<usize>::new());
        assert_eq!(extract_mentions("#[0"), Vec::<usize>::new());

        // Valid mention in the middle of other text
        assert_eq!(extract_mentions("prefix#[0]suffix"), vec![0]);

        // Multiple mentions
        assert_eq!(extract_mentions("#[0]#[1]#[2]"), vec![0, 1, 2]);
    }
}
