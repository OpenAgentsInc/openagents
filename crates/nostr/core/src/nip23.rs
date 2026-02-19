//! NIP-23: Long-form Content
//!
//! Defines how to publish long-form articles and blog posts using kind 30023
//! (parameterized replaceable events). Kind 30024 is used for draft articles.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/23.md>

use crate::Event;
use thiserror::Error;

/// Event kind for published long-form articles
pub const ARTICLE_KIND: u16 = 30023;

/// Event kind for draft articles
pub const DRAFT_ARTICLE_KIND: u16 = 30024;

/// Errors that can occur during NIP-23 operations
#[derive(Debug, Error)]
pub enum Nip23Error {
    #[error("invalid event kind: expected 30023 or 30024, got {0}")]
    InvalidKind(u16),

    #[error("missing required d-tag")]
    MissingDTag,

    #[error("invalid tag format: {0}")]
    InvalidTag(String),

    #[error("parse error: {0}")]
    Parse(String),
}

/// A long-form article or blog post
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Article {
    pub event: Event,
    pub identifier: String,
    pub title: Option<String>,
    pub published_at: Option<u64>,
    pub image: Option<String>,
    pub summary: Option<String>,
    pub hashtags: Vec<String>,
}

impl Article {
    /// Create an article from an event
    pub fn from_event(event: Event) -> Result<Self, Nip23Error> {
        if event.kind != ARTICLE_KIND && event.kind != DRAFT_ARTICLE_KIND {
            return Err(Nip23Error::InvalidKind(event.kind));
        }

        // Find the d-tag (required for addressable events)
        let mut identifier = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "d" && tag.len() > 1 {
                identifier = Some(tag[1].clone());
                break;
            }
        }

        let identifier = identifier.ok_or(Nip23Error::MissingDTag)?;

        // Find title (optional but standardized)
        let mut title = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "title" && tag.len() > 1 {
                title = Some(tag[1].clone());
                break;
            }
        }

        // Find published_at (optional but standardized)
        let mut published_at = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "published_at" && tag.len() > 1 {
                if let Ok(timestamp) = tag[1].parse::<u64>() {
                    published_at = Some(timestamp);
                }
                break;
            }
        }

        // Find image (optional but standardized)
        let mut image = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "image" && tag.len() > 1 {
                image = Some(tag[1].clone());
                break;
            }
        }

        // Find summary (optional but standardized)
        let mut summary = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "summary" && tag.len() > 1 {
                summary = Some(tag[1].clone());
                break;
            }
        }

        // Collect all t-tags (hashtags)
        let mut hashtags = Vec::new();
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "t" && tag.len() > 1 {
                hashtags.push(tag[1].clone());
            }
        }

        Ok(Self {
            event,
            identifier,
            title,
            published_at,
            image,
            summary,
            hashtags,
        })
    }

    /// Get the article's unique identifier (d-tag value)
    pub fn get_identifier(&self) -> &str {
        &self.identifier
    }

    /// Get the article title (if set)
    pub fn get_title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Get the initial publication timestamp (if set)
    pub fn get_published_at(&self) -> Option<u64> {
        self.published_at
    }

    /// Get the cover image URL (if set)
    pub fn get_image(&self) -> Option<&str> {
        self.image.as_deref()
    }

    /// Get the article summary (if set)
    pub fn get_summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    /// Get all hashtags (t-tags)
    pub fn get_hashtags(&self) -> &[String] {
        &self.hashtags
    }

    /// Get the markdown content
    pub fn get_content(&self) -> &str {
        &self.event.content
    }

    /// Get the author's public key
    pub fn get_author(&self) -> &str {
        &self.event.pubkey
    }

    /// Get the last update timestamp (created_at)
    pub fn get_updated_at(&self) -> u64 {
        self.event.created_at
    }

    /// Check if this is a draft article
    pub fn is_draft(&self) -> bool {
        self.event.kind == DRAFT_ARTICLE_KIND
    }

    /// Check if this is a published article
    pub fn is_published(&self) -> bool {
        self.event.kind == ARTICLE_KIND
    }

    /// Construct the addressable event coordinate (kind:pubkey:d-tag)
    pub fn get_coordinate(&self) -> String {
        format!(
            "{}:{}:{}",
            self.event.kind, self.event.pubkey, self.identifier
        )
    }

    /// Validate the article structure
    pub fn validate(&self) -> Result<(), Nip23Error> {
        if self.event.kind != ARTICLE_KIND && self.event.kind != DRAFT_ARTICLE_KIND {
            return Err(Nip23Error::InvalidKind(self.event.kind));
        }

        // Ensure d-tag exists
        let has_d_tag = self
            .event
            .tags
            .iter()
            .any(|tag| !tag.is_empty() && tag[0] == "d");

        if !has_d_tag {
            return Err(Nip23Error::MissingDTag);
        }

        Ok(())
    }
}

/// Helper function to check if an event kind is a long-form article
pub fn is_article_kind(kind: u16) -> bool {
    kind == ARTICLE_KIND || kind == DRAFT_ARTICLE_KIND
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_article_event(
        identifier: &str,
        title: Option<&str>,
        published_at: Option<u64>,
        content: &str,
    ) -> Event {
        let mut tags = vec![vec!["d".to_string(), identifier.to_string()]];

        if let Some(t) = title {
            tags.push(vec!["title".to_string(), t.to_string()]);
        }

        if let Some(ts) = published_at {
            tags.push(vec!["published_at".to_string(), ts.to_string()]);
        }

        Event {
            id: "article_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1675642635,
            kind: ARTICLE_KIND,
            tags,
            content: content.to_string(),
            sig: "test_sig".to_string(),
        }
    }

    #[test]
    fn test_article_from_event_minimal() {
        let event = create_test_article_event(
            "my-first-article",
            None,
            None,
            "# Hello World\n\nThis is my first article.",
        );
        let article = Article::from_event(event).unwrap();

        assert_eq!(article.get_identifier(), "my-first-article");
        assert!(article.get_title().is_none());
        assert!(article.get_published_at().is_none());
        assert_eq!(
            article.get_content(),
            "# Hello World\n\nThis is my first article."
        );
        assert!(article.is_published());
        assert!(!article.is_draft());
    }

    #[test]
    fn test_article_from_event_with_metadata() {
        let event = create_test_article_event(
            "my-article-slug",
            Some("My Amazing Article"),
            Some(1296962229),
            "Article content here",
        );
        let article = Article::from_event(event).unwrap();

        assert_eq!(article.get_identifier(), "my-article-slug");
        assert_eq!(article.get_title(), Some("My Amazing Article"));
        assert_eq!(article.get_published_at(), Some(1296962229));
    }

    #[test]
    fn test_article_with_image_and_summary() {
        let mut event = create_test_article_event("slug", Some("Title"), None, "Content");
        event.tags.push(vec![
            "image".to_string(),
            "https://example.com/cover.jpg".to_string(),
        ]);
        event.tags.push(vec![
            "summary".to_string(),
            "This is a brief summary".to_string(),
        ]);

        let article = Article::from_event(event).unwrap();
        assert_eq!(article.get_image(), Some("https://example.com/cover.jpg"));
        assert_eq!(article.get_summary(), Some("This is a brief summary"));
    }

    #[test]
    fn test_article_with_hashtags() {
        let mut event = create_test_article_event("slug", Some("Title"), None, "Content");
        event
            .tags
            .push(vec!["t".to_string(), "bitcoin".to_string()]);
        event.tags.push(vec!["t".to_string(), "nostr".to_string()]);
        event
            .tags
            .push(vec!["t".to_string(), "technology".to_string()]);

        let article = Article::from_event(event).unwrap();
        let hashtags = article.get_hashtags();
        assert_eq!(hashtags.len(), 3);
        assert_eq!(hashtags[0], "bitcoin");
        assert_eq!(hashtags[1], "nostr");
        assert_eq!(hashtags[2], "technology");
    }

    #[test]
    fn test_article_coordinate() {
        let event = create_test_article_event("my-article", None, None, "Content");
        let article = Article::from_event(event).unwrap();
        assert_eq!(article.get_coordinate(), "30023:author_pubkey:my-article");
    }

    #[test]
    fn test_article_draft() {
        let mut event =
            create_test_article_event("draft-slug", Some("Draft Title"), None, "Draft content");
        event.kind = DRAFT_ARTICLE_KIND;

        let article = Article::from_event(event).unwrap();
        assert!(article.is_draft());
        assert!(!article.is_published());
        assert_eq!(article.get_coordinate(), "30024:author_pubkey:draft-slug");
    }

    #[test]
    fn test_article_missing_d_tag() {
        let event = Event {
            id: "article_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1675642635,
            kind: ARTICLE_KIND,
            tags: vec![],
            content: "Content".to_string(),
            sig: "test_sig".to_string(),
        };

        let result = Article::from_event(event);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip23Error::MissingDTag));
    }

    #[test]
    fn test_article_invalid_kind() {
        let mut event = create_test_article_event("slug", None, None, "Content");
        event.kind = 1;

        let result = Article::from_event(event);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip23Error::InvalidKind(1)));
    }

    #[test]
    fn test_article_validate() {
        let event = create_test_article_event("slug", Some("Title"), None, "Content");
        let article = Article::from_event(event).unwrap();
        assert!(article.validate().is_ok());
    }

    #[test]
    fn test_article_get_author() {
        let event = create_test_article_event("slug", None, None, "Content");
        let article = Article::from_event(event).unwrap();
        assert_eq!(article.get_author(), "author_pubkey");
    }

    #[test]
    fn test_article_get_updated_at() {
        let event = create_test_article_event("slug", None, None, "Content");
        let article = Article::from_event(event).unwrap();
        assert_eq!(article.get_updated_at(), 1675642635);
    }

    #[test]
    fn test_is_article_kind() {
        assert!(is_article_kind(ARTICLE_KIND));
        assert!(is_article_kind(DRAFT_ARTICLE_KIND));
        assert!(!is_article_kind(1));
        assert!(!is_article_kind(7));
    }
}
