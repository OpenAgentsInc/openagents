//! NIP-84: Highlights
//!
//! This NIP defines kind 9802 for highlighting valuable content from various sources.
//! Highlights can be text snippets from articles, web pages, or references to non-text
//! media like audio/video.
//!
//! ## Features
//!
//! - Text highlights with optional context
//! - Attribution to original authors/editors
//! - Source references (Nostr events or URLs)
//! - Quote highlights with comments
//!
//! ## Examples
//!
//! ```
//! use nostr::nip84::{Highlight, HighlightSource, Attribution};
//!
//! // Simple text highlight from a URL
//! let highlight = Highlight::new(
//!     "The only way to do great work is to love what you do.",
//!     HighlightSource::Url("https://example.com/article".to_string())
//! );
//!
//! // Highlight with author attribution
//! let mut highlight = Highlight::new(
//!     "Highlighted text",
//!     HighlightSource::Url("https://example.com".to_string())
//! );
//! highlight.add_attribution(Attribution::author("pubkey-hex", Some("wss://relay.example.com".to_string())));
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind number for highlight events.
pub const KIND_HIGHLIGHT: u64 = 9802;

/// Errors that can occur during NIP-84 operations.
#[derive(Debug, Error)]
pub enum Nip84Error {
    #[error("highlight must have at least one source (e, a, or r tag)")]
    NoSource,

    #[error("invalid attribution role: {0}")]
    InvalidRole(String),
}

/// Check if a kind is a NIP-84 highlight.
pub fn is_nip84_kind(kind: u64) -> bool {
    kind == KIND_HIGHLIGHT
}

/// Source of a highlight - can be a Nostr event or external URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HighlightSource {
    /// Nostr event by ID
    Event(String),
    /// Addressable Nostr event
    Address(String),
    /// External URL (should be cleaned of trackers)
    Url(String),
}

/// Attribution to an author or editor of the highlighted content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attribution {
    /// Public key of the person
    pub pubkey: String,
    /// Optional relay URL for the person
    pub relay: Option<String>,
    /// Role (author, editor, mention)
    pub role: String,
}

impl Attribution {
    /// Create an author attribution.
    pub fn author(pubkey: impl Into<String>, relay: Option<String>) -> Self {
        Self {
            pubkey: pubkey.into(),
            relay,
            role: "author".to_string(),
        }
    }

    /// Create an editor attribution.
    pub fn editor(pubkey: impl Into<String>, relay: Option<String>) -> Self {
        Self {
            pubkey: pubkey.into(),
            relay,
            role: "editor".to_string(),
        }
    }

    /// Create a mention attribution (for quote highlights).
    pub fn mention(pubkey: impl Into<String>, relay: Option<String>) -> Self {
        Self {
            pubkey: pubkey.into(),
            relay,
            role: "mention".to_string(),
        }
    }

    /// Validate the attribution.
    pub fn validate(&self) -> Result<(), Nip84Error> {
        if !["author", "editor", "mention"].contains(&self.role.as_str()) {
            return Err(Nip84Error::InvalidRole(self.role.clone()));
        }
        Ok(())
    }
}

/// A highlight event (kind 9802).
///
/// Signals content a user finds valuable, typically a text snippet
/// from an article or web page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Highlight {
    /// The highlighted text (may be empty for non-text media)
    pub content: String,

    /// Source of the highlight
    pub source: HighlightSource,

    /// Optional context - surrounding text for subset highlights
    pub context: Option<String>,

    /// Optional comment for quote highlights
    pub comment: Option<String>,

    /// Attribution to authors/editors
    pub attributions: Vec<Attribution>,

    /// Additional URLs mentioned in comments (for quote highlights)
    /// These get `mention` attribute in r tags
    pub mentioned_urls: Vec<String>,
}

impl Highlight {
    /// Create a new highlight with just content and source.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip84::{Highlight, HighlightSource};
    ///
    /// let highlight = Highlight::new(
    ///     "The only way to do great work is to love what you do.",
    ///     HighlightSource::Url("https://example.com/article".to_string())
    /// );
    /// ```
    pub fn new(content: impl Into<String>, source: HighlightSource) -> Self {
        Self {
            content: content.into(),
            source,
            context: None,
            comment: None,
            attributions: Vec::new(),
            mentioned_urls: Vec::new(),
        }
    }

    /// Create a highlight for non-text media (empty content).
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip84::{Highlight, HighlightSource};
    ///
    /// let highlight = Highlight::media(
    ///     HighlightSource::Event("event-id".to_string())
    /// );
    /// assert_eq!(highlight.content, "");
    /// ```
    pub fn media(source: HighlightSource) -> Self {
        Self::new("", source)
    }

    /// Add context (surrounding text).
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Add a comment to create a quote highlight.
    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    /// Add an attribution (author, editor, or mention).
    pub fn add_attribution(&mut self, attribution: Attribution) {
        self.attributions.push(attribution);
    }

    /// Add a mentioned URL (for quote highlights).
    pub fn add_mentioned_url(&mut self, url: impl Into<String>) {
        self.mentioned_urls.push(url.into());
    }

    /// Check if this is a quote highlight (has a comment).
    pub fn is_quote(&self) -> bool {
        self.comment.is_some()
    }

    /// Validate the highlight.
    pub fn validate(&self) -> Result<(), Nip84Error> {
        // Validate all attributions
        for attr in &self.attributions {
            attr.validate()?;
        }

        Ok(())
    }

    /// Convert to Nostr event tags.
    ///
    /// Returns a JSON array of tags.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // Add source tag
        match &self.source {
            HighlightSource::Event(id) => {
                tags.push(vec!["e".to_string(), id.clone()]);
            }
            HighlightSource::Address(addr) => {
                tags.push(vec!["a".to_string(), addr.clone()]);
            }
            HighlightSource::Url(url) => {
                if self.is_quote() {
                    // For quote highlights, source URL must have "source" attribute
                    tags.push(vec!["r".to_string(), url.clone(), "source".to_string()]);
                } else {
                    tags.push(vec!["r".to_string(), url.clone()]);
                }
            }
        }

        // Add context if present
        if let Some(context) = &self.context {
            tags.push(vec!["context".to_string(), context.clone()]);
        }

        // Add comment if present
        if let Some(comment) = &self.comment {
            tags.push(vec!["comment".to_string(), comment.clone()]);
        }

        // Add attributions
        for attr in &self.attributions {
            let mut tag = vec!["p".to_string(), attr.pubkey.clone()];
            if let Some(relay) = &attr.relay {
                tag.push(relay.clone());
            } else {
                tag.push("".to_string());
            }
            tag.push(attr.role.clone());
            tags.push(tag);
        }

        // Add mentioned URLs (for quote highlights)
        for url in &self.mentioned_urls {
            tags.push(vec!["r".to_string(), url.clone(), "mention".to_string()]);
        }

        tags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_nip84_kind() {
        assert!(is_nip84_kind(9802));
        assert!(!is_nip84_kind(1));
        assert!(!is_nip84_kind(9801));
    }

    #[test]
    fn test_highlight_new() {
        let highlight = Highlight::new(
            "The only way to do great work is to love what you do.",
            HighlightSource::Url("https://example.com/article".to_string()),
        );

        assert_eq!(
            highlight.content,
            "The only way to do great work is to love what you do."
        );
        assert_eq!(
            highlight.source,
            HighlightSource::Url("https://example.com/article".to_string())
        );
        assert_eq!(highlight.context, None);
        assert_eq!(highlight.comment, None);
        assert!(highlight.attributions.is_empty());
        assert!(!highlight.is_quote());
    }

    #[test]
    fn test_highlight_media() {
        let highlight = Highlight::media(HighlightSource::Event("event-id".to_string()));

        assert_eq!(highlight.content, "");
        assert_eq!(
            highlight.source,
            HighlightSource::Event("event-id".to_string())
        );
    }

    #[test]
    fn test_highlight_with_context() {
        let highlight = Highlight::new(
            "do great work",
            HighlightSource::Url("https://example.com".to_string()),
        )
        .with_context("The only way to do great work is to love what you do.");

        assert_eq!(
            highlight.context,
            Some("The only way to do great work is to love what you do.".to_string())
        );
    }

    #[test]
    fn test_highlight_with_comment() {
        let highlight = Highlight::new(
            "highlighted text",
            HighlightSource::Url("https://example.com".to_string()),
        )
        .with_comment("This is really interesting!");

        assert_eq!(
            highlight.comment,
            Some("This is really interesting!".to_string())
        );
        assert!(highlight.is_quote());
    }

    #[test]
    fn test_attribution_author() {
        let attr = Attribution::author("pubkey123", Some("wss://relay.example.com".to_string()));

        assert_eq!(attr.pubkey, "pubkey123");
        assert_eq!(attr.relay, Some("wss://relay.example.com".to_string()));
        assert_eq!(attr.role, "author");
        assert!(attr.validate().is_ok());
    }

    #[test]
    fn test_attribution_editor() {
        let attr = Attribution::editor("pubkey456", None);

        assert_eq!(attr.pubkey, "pubkey456");
        assert_eq!(attr.relay, None);
        assert_eq!(attr.role, "editor");
        assert!(attr.validate().is_ok());
    }

    #[test]
    fn test_attribution_mention() {
        let attr = Attribution::mention("pubkey789", Some("wss://relay.example.com".to_string()));

        assert_eq!(attr.pubkey, "pubkey789");
        assert_eq!(attr.relay, Some("wss://relay.example.com".to_string()));
        assert_eq!(attr.role, "mention");
        assert!(attr.validate().is_ok());
    }

    #[test]
    fn test_attribution_invalid_role() {
        let attr = Attribution {
            pubkey: "pubkey".to_string(),
            relay: None,
            role: "invalid".to_string(),
        };

        assert!(attr.validate().is_err());
    }

    #[test]
    fn test_highlight_add_attribution() {
        let mut highlight = Highlight::new(
            "text",
            HighlightSource::Url("https://example.com".to_string()),
        );

        highlight.add_attribution(Attribution::author("pubkey1", None));
        highlight.add_attribution(Attribution::editor(
            "pubkey2",
            Some("wss://relay.com".to_string()),
        ));

        assert_eq!(highlight.attributions.len(), 2);
        assert_eq!(highlight.attributions[0].role, "author");
        assert_eq!(highlight.attributions[1].role, "editor");
    }

    #[test]
    fn test_highlight_to_tags_simple() {
        let highlight = Highlight::new(
            "highlighted text",
            HighlightSource::Url("https://example.com/article".to_string()),
        );

        let tags = highlight.to_tags();

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], vec!["r", "https://example.com/article"]);
    }

    #[test]
    fn test_highlight_to_tags_event_source() {
        let highlight = Highlight::new(
            "highlighted text",
            HighlightSource::Event("event-id-123".to_string()),
        );

        let tags = highlight.to_tags();

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], vec!["e", "event-id-123"]);
    }

    #[test]
    fn test_highlight_to_tags_address_source() {
        let highlight = Highlight::new(
            "highlighted text",
            HighlightSource::Address("30023:pubkey:d-tag".to_string()),
        );

        let tags = highlight.to_tags();

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], vec!["a", "30023:pubkey:d-tag"]);
    }

    #[test]
    fn test_highlight_to_tags_with_context() {
        let highlight = Highlight::new(
            "do great work",
            HighlightSource::Url("https://example.com".to_string()),
        )
        .with_context("The only way to do great work is to love what you do.");

        let tags = highlight.to_tags();

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0], vec!["r", "https://example.com"]);
        assert_eq!(
            tags[1],
            vec![
                "context",
                "The only way to do great work is to love what you do."
            ]
        );
    }

    #[test]
    fn test_highlight_to_tags_with_attribution() {
        let mut highlight = Highlight::new(
            "text",
            HighlightSource::Url("https://example.com".to_string()),
        );
        highlight.add_attribution(Attribution::author(
            "pubkey1",
            Some("wss://relay.example.com".to_string()),
        ));
        highlight.add_attribution(Attribution::editor("pubkey2", None));

        let tags = highlight.to_tags();

        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0], vec!["r", "https://example.com"]);
        assert_eq!(
            tags[1],
            vec!["p", "pubkey1", "wss://relay.example.com", "author"]
        );
        assert_eq!(tags[2], vec!["p", "pubkey2", "", "editor"]);
    }

    #[test]
    fn test_highlight_quote_with_source_attribute() {
        let highlight = Highlight::new(
            "highlighted text",
            HighlightSource::Url("https://example.com/article".to_string()),
        )
        .with_comment("Great point!");

        let tags = highlight.to_tags();

        // Source URL should have "source" attribute for quote highlights
        assert!(tags.iter().any(|tag| tag.len() == 3
            && tag[0] == "r"
            && tag[1] == "https://example.com/article"
            && tag[2] == "source"));

        // Should have comment tag
        assert!(
            tags.iter()
                .any(|tag| tag.len() == 2 && tag[0] == "comment" && tag[1] == "Great point!")
        );
    }

    #[test]
    fn test_highlight_quote_with_mentioned_urls() {
        let mut highlight = Highlight::new(
            "highlighted text",
            HighlightSource::Url("https://example.com/article".to_string()),
        )
        .with_comment("Check out https://other.com too!");

        highlight.add_mentioned_url("https://other.com");

        let tags = highlight.to_tags();

        // Source URL with "source" attribute
        assert!(tags.iter().any(|tag| tag.len() == 3
            && tag[0] == "r"
            && tag[1] == "https://example.com/article"
            && tag[2] == "source"));

        // Mentioned URL with "mention" attribute
        assert!(tags.iter().any(|tag| tag.len() == 3
            && tag[0] == "r"
            && tag[1] == "https://other.com"
            && tag[2] == "mention"));
    }

    #[test]
    fn test_highlight_quote_with_mentions() {
        let mut highlight = Highlight::new(
            "highlighted text",
            HighlightSource::Url("https://example.com".to_string()),
        )
        .with_comment("Great work nostr:npub123!");

        highlight.add_attribution(Attribution::author("author-pubkey", None));
        highlight.add_attribution(Attribution::mention("mentioned-pubkey", None));

        let tags = highlight.to_tags();

        // Should have author
        assert!(tags.iter().any(|tag| tag.len() == 4
            && tag[0] == "p"
            && tag[1] == "author-pubkey"
            && tag[3] == "author"));

        // Should have mention
        assert!(tags.iter().any(|tag| tag.len() == 4
            && tag[0] == "p"
            && tag[1] == "mentioned-pubkey"
            && tag[3] == "mention"));
    }

    #[test]
    fn test_highlight_validate() {
        let mut highlight = Highlight::new(
            "text",
            HighlightSource::Url("https://example.com".to_string()),
        );
        highlight.add_attribution(Attribution::author("pubkey", None));

        assert!(highlight.validate().is_ok());
    }

    #[test]
    fn test_highlight_validate_invalid_attribution() {
        let mut highlight = Highlight::new(
            "text",
            HighlightSource::Url("https://example.com".to_string()),
        );
        highlight.add_attribution(Attribution {
            pubkey: "pubkey".to_string(),
            relay: None,
            role: "invalid".to_string(),
        });

        assert!(highlight.validate().is_err());
    }
}
