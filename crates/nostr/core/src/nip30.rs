//! NIP-30: Custom Emoji
//!
//! This NIP defines a way to add custom emoji to events by including emoji tags.
//! Custom emoji can be used in kind 0 (metadata), kind 1 (notes), kind 7 (reactions),
//! and kind 30315 (user statuses) events.
//!
//! ## Purpose
//!
//! Enable users to use custom emoji in their posts by:
//! - Defining emoji with shortcodes and image URLs
//! - Parsing :shortcode: references in content
//! - Displaying custom images instead of shortcodes
//!
//! ## Tag Format
//!
//! Emoji tags have the format: `["emoji", <shortcode>, <image-url>]`
//!
//! Where:
//! - `shortcode` must be alphanumeric characters and underscores only
//! - `image-url` is a URL to the emoji image file
//!
//! ## Examples
//!
//! ```
//! use nostr::nip30::{CustomEmoji, emojify, extract_shortcodes};
//!
//! // Create a custom emoji
//! let emoji = CustomEmoji::new("soapbox", "https://example.com/soapbox.png");
//!
//! // Extract shortcodes from text
//! let text = "Hello :soapbox: world :rocket:";
//! let shortcodes = extract_shortcodes(text);
//! assert_eq!(shortcodes, vec!["soapbox", "rocket"]);
//!
//! // Emojify text (replace :shortcode: with images)
//! let emojis = vec![emoji];
//! let result = emojify(text, &emojis);
//! ```

use thiserror::Error;

/// Tag name for custom emoji
pub const EMOJI_TAG: &str = "emoji";

/// Errors that can occur during NIP-30 operations
#[derive(Debug, Error)]
pub enum Nip30Error {
    #[error("shortcode cannot be empty")]
    EmptyShortcode,

    #[error("invalid shortcode: {0} (must contain only alphanumeric characters and underscores)")]
    InvalidShortcode(String),

    #[error("image URL cannot be empty")]
    EmptyImageUrl,

    #[error("emoji tag not found")]
    EmojiTagNotFound,
}

/// Custom emoji with shortcode and image URL
#[derive(Debug, Clone, PartialEq)]
pub struct CustomEmoji {
    /// Emoji shortcode (alphanumeric + underscores only)
    pub shortcode: String,
    /// Image URL
    pub image_url: String,
}

impl CustomEmoji {
    /// Create a new custom emoji
    pub fn new(shortcode: impl Into<String>, image_url: impl Into<String>) -> Self {
        Self {
            shortcode: shortcode.into(),
            image_url: image_url.into(),
        }
    }

    /// Convert to emoji tag
    pub fn to_tag(&self) -> Vec<String> {
        vec![
            EMOJI_TAG.to_string(),
            self.shortcode.clone(),
            self.image_url.clone(),
        ]
    }

    /// Parse from tag (expects ["emoji", shortcode, url])
    pub fn from_tag(tag: &[String]) -> Option<Self> {
        if tag.len() >= 3 && tag[0] == EMOJI_TAG {
            Some(Self::new(&tag[1], &tag[2]))
        } else {
            None
        }
    }

    /// Validate the emoji
    pub fn validate(&self) -> Result<(), Nip30Error> {
        validate_shortcode(&self.shortcode)?;
        if self.image_url.trim().is_empty() {
            return Err(Nip30Error::EmptyImageUrl);
        }
        Ok(())
    }
}

/// Validate an emoji shortcode (must be alphanumeric + underscores)
pub fn validate_shortcode(shortcode: &str) -> Result<(), Nip30Error> {
    if shortcode.is_empty() {
        return Err(Nip30Error::EmptyShortcode);
    }
    if !shortcode
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(Nip30Error::InvalidShortcode(shortcode.to_string()));
    }
    Ok(())
}

/// Get all emoji tags from event tags
pub fn get_emoji_tags(tags: &[Vec<String>]) -> Vec<CustomEmoji> {
    tags.iter()
        .filter_map(|tag| CustomEmoji::from_tag(tag))
        .collect()
}

/// Get a specific emoji by shortcode
pub fn get_emoji(tags: &[Vec<String>], shortcode: &str) -> Option<CustomEmoji> {
    get_emoji_tags(tags)
        .into_iter()
        .find(|e| e.shortcode == shortcode)
}

/// Check if tags contain an emoji with the given shortcode
pub fn has_emoji(tags: &[Vec<String>], shortcode: &str) -> bool {
    get_emoji(tags, shortcode).is_some()
}

/// Add an emoji tag to event tags
pub fn add_emoji_tag(tags: &mut Vec<Vec<String>>, emoji: &CustomEmoji) {
    tags.push(emoji.to_tag());
}

/// Remove all emoji tags with the given shortcode
pub fn remove_emoji_tag(tags: &mut Vec<Vec<String>>, shortcode: &str) {
    tags.retain(|tag| !(tag.len() >= 3 && tag[0] == EMOJI_TAG && tag[1] == shortcode));
}

/// Remove all emoji tags
pub fn remove_all_emoji_tags(tags: &mut Vec<Vec<String>>) {
    tags.retain(|tag| tag.is_empty() || tag[0] != EMOJI_TAG);
}

/// Extract all :shortcode: patterns from text
pub fn extract_shortcodes(text: &str) -> Vec<String> {
    let mut shortcodes = Vec::new();
    let chars = text.chars().peekable();
    let mut in_shortcode = false;
    let mut current_shortcode = String::new();

    for c in chars {
        if c == ':' {
            if in_shortcode {
                // End of shortcode
                if !current_shortcode.is_empty() {
                    shortcodes.push(current_shortcode.clone());
                }
                current_shortcode.clear();
                in_shortcode = false;
            } else {
                // Start of shortcode
                in_shortcode = true;
            }
        } else if in_shortcode {
            if c.is_ascii_alphanumeric() || c == '_' {
                current_shortcode.push(c);
            } else {
                // Invalid character in shortcode, abandon it
                current_shortcode.clear();
                in_shortcode = false;
            }
        }
    }

    shortcodes
}

/// Emojify text by replacing :shortcode: with HTML image tags
pub fn emojify(text: &str, emojis: &[CustomEmoji]) -> String {
    let mut result = text.to_string();

    for emoji in emojis {
        let pattern = format!(":{}:", emoji.shortcode);
        let replacement = format!(
            r#"<img src="{}" alt=":{}:" class="emoji" />"#,
            emoji.image_url, emoji.shortcode
        );
        result = result.replace(&pattern, &replacement);
    }

    result
}

/// Emojify text by replacing :shortcode: with custom replacement function
pub fn emojify_with<F>(text: &str, emojis: &[CustomEmoji], mut replacer: F) -> String
where
    F: FnMut(&CustomEmoji) -> String,
{
    let mut result = text.to_string();

    for emoji in emojis {
        let pattern = format!(":{}:", emoji.shortcode);
        let replacement = replacer(emoji);
        result = result.replace(&pattern, &replacement);
    }

    result
}

/// Check if text contains any :shortcode: patterns
pub fn contains_shortcodes(text: &str) -> bool {
    text.contains(':') && !extract_shortcodes(text).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_shortcode() {
        assert!(validate_shortcode("soapbox").is_ok());
        assert!(validate_shortcode("emoji_123").is_ok());
        assert!(validate_shortcode("ABCabc123").is_ok());

        assert!(validate_shortcode("").is_err());
        assert!(validate_shortcode("soap-box").is_err());
        assert!(validate_shortcode("soap box").is_err());
        assert!(validate_shortcode("soap.box").is_err());
    }

    #[test]
    fn test_custom_emoji() {
        let emoji = CustomEmoji::new("soapbox", "https://example.com/soapbox.png");
        assert_eq!(emoji.shortcode, "soapbox");
        assert_eq!(emoji.image_url, "https://example.com/soapbox.png");
        assert!(emoji.validate().is_ok());
    }

    #[test]
    fn test_custom_emoji_to_tag() {
        let emoji = CustomEmoji::new("soapbox", "https://example.com/soapbox.png");
        let tag = emoji.to_tag();
        assert_eq!(
            tag,
            vec!["emoji", "soapbox", "https://example.com/soapbox.png"]
        );
    }

    #[test]
    fn test_custom_emoji_from_tag() {
        let tag = vec![
            "emoji".to_string(),
            "soapbox".to_string(),
            "https://example.com/soapbox.png".to_string(),
        ];
        let emoji = CustomEmoji::from_tag(&tag).unwrap();
        assert_eq!(emoji.shortcode, "soapbox");
        assert_eq!(emoji.image_url, "https://example.com/soapbox.png");
    }

    #[test]
    fn test_custom_emoji_from_tag_invalid() {
        let tag = vec!["other".to_string(), "value".to_string()];
        assert!(CustomEmoji::from_tag(&tag).is_none());

        let tag = vec!["emoji".to_string(), "short".to_string()];
        assert!(CustomEmoji::from_tag(&tag).is_none());
    }

    #[test]
    fn test_get_emoji_tags() {
        let tags = vec![
            vec![
                "emoji".to_string(),
                "soapbox".to_string(),
                "https://example.com/soapbox.png".to_string(),
            ],
            vec!["p".to_string(), "pubkey".to_string()],
            vec![
                "emoji".to_string(),
                "rocket".to_string(),
                "https://example.com/rocket.png".to_string(),
            ],
        ];

        let emojis = get_emoji_tags(&tags);
        assert_eq!(emojis.len(), 2);
        assert_eq!(emojis[0].shortcode, "soapbox");
        assert_eq!(emojis[1].shortcode, "rocket");
    }

    #[test]
    fn test_get_emoji() {
        let tags = vec![
            vec![
                "emoji".to_string(),
                "soapbox".to_string(),
                "https://example.com/soapbox.png".to_string(),
            ],
            vec![
                "emoji".to_string(),
                "rocket".to_string(),
                "https://example.com/rocket.png".to_string(),
            ],
        ];

        let emoji = get_emoji(&tags, "rocket").unwrap();
        assert_eq!(emoji.shortcode, "rocket");

        assert!(get_emoji(&tags, "notfound").is_none());
    }

    #[test]
    fn test_has_emoji() {
        let tags = vec![vec![
            "emoji".to_string(),
            "soapbox".to_string(),
            "https://example.com/soapbox.png".to_string(),
        ]];

        assert!(has_emoji(&tags, "soapbox"));
        assert!(!has_emoji(&tags, "notfound"));
    }

    #[test]
    fn test_add_emoji_tag() {
        let mut tags = Vec::new();
        let emoji = CustomEmoji::new("soapbox", "https://example.com/soapbox.png");
        add_emoji_tag(&mut tags, &emoji);

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0][0], "emoji");
        assert_eq!(tags[0][1], "soapbox");
    }

    #[test]
    fn test_remove_emoji_tag() {
        let mut tags = vec![
            vec![
                "emoji".to_string(),
                "soapbox".to_string(),
                "url1".to_string(),
            ],
            vec![
                "emoji".to_string(),
                "rocket".to_string(),
                "url2".to_string(),
            ],
            vec!["p".to_string(), "pubkey".to_string()],
        ];

        remove_emoji_tag(&mut tags, "soapbox");
        assert_eq!(tags.len(), 2);
        assert!(has_emoji(&tags, "rocket"));
        assert!(!has_emoji(&tags, "soapbox"));
    }

    #[test]
    fn test_remove_all_emoji_tags() {
        let mut tags = vec![
            vec![
                "emoji".to_string(),
                "soapbox".to_string(),
                "url1".to_string(),
            ],
            vec![
                "emoji".to_string(),
                "rocket".to_string(),
                "url2".to_string(),
            ],
            vec!["p".to_string(), "pubkey".to_string()],
        ];

        remove_all_emoji_tags(&mut tags);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0][0], "p");
    }

    #[test]
    fn test_extract_shortcodes() {
        let text = "Hello :soapbox: world :rocket:";
        let shortcodes = extract_shortcodes(text);
        assert_eq!(shortcodes, vec!["soapbox", "rocket"]);
    }

    #[test]
    fn test_extract_shortcodes_with_underscores() {
        let text = "Hello :custom_emoji_123: world";
        let shortcodes = extract_shortcodes(text);
        assert_eq!(shortcodes, vec!["custom_emoji_123"]);
    }

    #[test]
    fn test_extract_shortcodes_invalid() {
        let text = "Hello :soap-box: world"; // hyphen not allowed
        let shortcodes = extract_shortcodes(text);
        assert_eq!(shortcodes.len(), 0);
    }

    #[test]
    fn test_extract_shortcodes_no_emoji() {
        let text = "Hello world";
        let shortcodes = extract_shortcodes(text);
        assert_eq!(shortcodes.len(), 0);
    }

    #[test]
    fn test_extract_shortcodes_adjacent() {
        let text = ":one::two:";
        let shortcodes = extract_shortcodes(text);
        assert_eq!(shortcodes, vec!["one", "two"]);
    }

    #[test]
    fn test_emojify() {
        let emojis = vec![
            CustomEmoji::new("soapbox", "https://example.com/soapbox.png"),
            CustomEmoji::new("rocket", "https://example.com/rocket.png"),
        ];

        let text = "Hello :soapbox: world :rocket:";
        let result = emojify(text, &emojis);

        assert!(result.contains(r#"<img src="https://example.com/soapbox.png""#));
        assert!(result.contains(r#"<img src="https://example.com/rocket.png""#));
        assert!(result.contains(r#"alt=":soapbox:""#));
    }

    #[test]
    fn test_emojify_no_match() {
        let emojis = vec![CustomEmoji::new(
            "soapbox",
            "https://example.com/soapbox.png",
        )];

        let text = "Hello :rocket: world";
        let result = emojify(text, &emojis);

        assert_eq!(result, "Hello :rocket: world");
    }

    #[test]
    fn test_emojify_with() {
        let emojis = vec![CustomEmoji::new(
            "soapbox",
            "https://example.com/soapbox.png",
        )];

        let text = "Hello :soapbox: world";
        let result = emojify_with(text, &emojis, |e| format!("[{}]", e.shortcode));

        assert_eq!(result, "Hello [soapbox] world");
    }

    #[test]
    fn test_contains_shortcodes() {
        assert!(contains_shortcodes("Hello :soapbox: world"));
        assert!(!contains_shortcodes("Hello world"));
        assert!(!contains_shortcodes("Hello : world"));
    }

    #[test]
    fn test_emojify_multiple_occurrences() {
        let emojis = vec![CustomEmoji::new("smile", "https://example.com/smile.png")];

        let text = ":smile: Hello :smile: world :smile:";
        let result = emojify(text, &emojis);

        let count = result
            .matches(r#"<img src="https://example.com/smile.png""#)
            .count();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_custom_emoji_validate_invalid_shortcode() {
        let emoji = CustomEmoji::new("soap-box", "https://example.com/soapbox.png");
        assert!(emoji.validate().is_err());
    }

    #[test]
    fn test_custom_emoji_validate_empty_url() {
        let emoji = CustomEmoji::new("soapbox", "");
        assert!(emoji.validate().is_err());
    }
}
