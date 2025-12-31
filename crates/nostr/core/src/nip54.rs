//! NIP-54: Wiki
//!
//! This NIP defines wiki articles as addressable events where multiple people
//! can write about the same subjects. Articles use Asciidoc format with wikilinks.
//!
//! ## Event Types
//!
//! - **Wiki Article** (kind 30818): Encyclopedia entries about subjects
//! - **Merge Request** (kind 818): Request to merge forked article
//! - **Wiki Redirect** (kind 30819): Redirect from one article to another
//!
//! ## Example
//!
//! ```
//! use nostr::nip54::{WikiArticle, normalize_d_tag};
//!
//! // Create a wiki article
//! let mut article = WikiArticle::new("rust-programming", "Rust Programming");
//! article.summary = Some("A systems programming language".to_string());
//!
//! // Normalize d tag
//! assert_eq!(normalize_d_tag("Hello World!"), "hello-world");
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind for wiki articles
pub const KIND_WIKI_ARTICLE: u16 = 30818;

/// Kind for merge requests
pub const KIND_WIKI_MERGE_REQUEST: u16 = 818;

/// Kind for wiki redirects
pub const KIND_WIKI_REDIRECT: u16 = 30819;

/// Errors that can occur during NIP-54 operations.
#[derive(Debug, Error)]
pub enum Nip54Error {
    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid d tag: {0}")]
    InvalidDTag(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Normalize a d tag according to NIP-54 rules
///
/// - Convert all non-letter characters to `-`
/// - Convert all letters to lowercase
///
/// # Example
///
/// ```
/// use nostr::nip54::normalize_d_tag;
///
/// assert_eq!(normalize_d_tag("Hello World!"), "hello-world");
/// assert_eq!(normalize_d_tag("Rust_Programming"), "rust-programming");
/// ```
pub fn normalize_d_tag(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphabetic() {
                c.to_lowercase().to_string()
            } else {
                "-".to_string()
            }
        })
        .collect()
}

/// Reference to another wiki article (fork or defer)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WikiReference {
    /// Event address (a tag)
    pub address: String,
    /// Optional relay URL
    pub relay: Option<String>,
    /// Event ID (e tag)
    pub event_id: Option<String>,
    /// Optional relay for event ID
    pub event_relay: Option<String>,
    /// Marker (fork or defer)
    pub marker: String,
}

impl WikiReference {
    /// Create a fork reference
    pub fn fork(address: impl Into<String>, event_id: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            relay: None,
            event_id: Some(event_id.into()),
            event_relay: None,
            marker: "fork".to_string(),
        }
    }

    /// Create a defer reference
    pub fn defer(address: impl Into<String>, event_id: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            relay: None,
            event_id: Some(event_id.into()),
            event_relay: None,
            marker: "defer".to_string(),
        }
    }
}

/// Wiki article (kind 30818)
///
/// An addressable event representing an encyclopedia entry.
/// Content should be in Asciidoc format with wikilinks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WikiArticle {
    /// Normalized identifier (d tag)
    pub d: String,
    /// Display title (if different from d tag)
    pub title: Option<String>,
    /// Summary for display in lists
    pub summary: Option<String>,
    /// References to forked/deferred articles
    pub references: Vec<WikiReference>,
}

impl WikiArticle {
    /// Create a new wiki article
    ///
    /// The d tag will be normalized according to NIP-54 rules.
    pub fn new(d: impl Into<String>, title: impl Into<String>) -> Self {
        let d_str = d.into();
        Self {
            d: normalize_d_tag(&d_str),
            title: Some(title.into()),
            summary: None,
            references: Vec::new(),
        }
    }

    /// Create a new wiki article with pre-normalized d tag
    pub fn new_normalized(d: impl Into<String>) -> Self {
        Self {
            d: d.into(),
            title: None,
            summary: None,
            references: Vec::new(),
        }
    }

    /// Add a fork reference
    pub fn add_fork(&mut self, reference: WikiReference) {
        self.references.push(reference);
    }

    /// Add a defer reference
    pub fn add_defer(&mut self, reference: WikiReference) {
        self.references.push(reference);
    }

    /// Validate the wiki article
    pub fn validate(&self) -> Result<(), Nip54Error> {
        if self.d.is_empty() {
            return Err(Nip54Error::MissingField("d".to_string()));
        }

        // Verify d tag is normalized
        let normalized = normalize_d_tag(&self.d);
        if self.d != normalized {
            return Err(Nip54Error::InvalidDTag(format!(
                "d tag '{}' is not normalized, should be '{}'",
                self.d, normalized
            )));
        }

        Ok(())
    }

    /// Convert to tags for event creation
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec!["d".to_string(), self.d.clone()]];

        if let Some(title) = &self.title {
            tags.push(vec!["title".to_string(), title.clone()]);
        }

        if let Some(summary) = &self.summary {
            tags.push(vec!["summary".to_string(), summary.clone()]);
        }

        for reference in &self.references {
            // a tag
            let mut a_tag = vec!["a".to_string(), reference.address.clone()];
            if let Some(relay) = &reference.relay {
                a_tag.push(relay.clone());
            } else {
                a_tag.push(String::new());
            }
            a_tag.push(reference.marker.clone());
            tags.push(a_tag);

            // e tag (if present)
            if let Some(event_id) = &reference.event_id {
                let mut e_tag = vec!["e".to_string(), event_id.clone()];
                if let Some(relay) = &reference.event_relay {
                    e_tag.push(relay.clone());
                } else {
                    e_tag.push(String::new());
                }
                e_tag.push(reference.marker.clone());
                tags.push(e_tag);
            }
        }

        tags
    }
}

/// Merge request (kind 818)
///
/// A request to merge a forked article into the source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WikiMergeRequest {
    /// Target article address (a tag)
    pub target_address: String,
    /// Optional relay for target
    pub target_relay: Option<String>,
    /// Destination pubkey (p tag)
    pub destination_pubkey: String,
    /// Optional base version event ID (e tag)
    pub base_version: Option<String>,
    /// Optional relay for base version
    pub base_relay: Option<String>,
    /// Source version to merge (e tag with "source" marker)
    pub source_version: String,
    /// Optional relay for source
    pub source_relay: Option<String>,
}

impl WikiMergeRequest {
    /// Create a new merge request
    pub fn new(
        target_address: impl Into<String>,
        destination_pubkey: impl Into<String>,
        source_version: impl Into<String>,
    ) -> Self {
        Self {
            target_address: target_address.into(),
            target_relay: None,
            destination_pubkey: destination_pubkey.into(),
            base_version: None,
            base_relay: None,
            source_version: source_version.into(),
            source_relay: None,
        }
    }

    /// Set base version
    pub fn with_base_version(mut self, base_version: impl Into<String>) -> Self {
        self.base_version = Some(base_version.into());
        self
    }

    /// Validate the merge request
    pub fn validate(&self) -> Result<(), Nip54Error> {
        if self.target_address.is_empty() {
            return Err(Nip54Error::MissingField("target_address".to_string()));
        }
        if self.destination_pubkey.is_empty() {
            return Err(Nip54Error::MissingField("destination_pubkey".to_string()));
        }
        if self.source_version.is_empty() {
            return Err(Nip54Error::MissingField("source_version".to_string()));
        }
        Ok(())
    }

    /// Convert to tags for event creation
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // a tag (target)
        let mut a_tag = vec!["a".to_string(), self.target_address.clone()];
        if let Some(relay) = &self.target_relay {
            a_tag.push(relay.clone());
        }
        tags.push(a_tag);

        // p tag (destination)
        tags.push(vec!["p".to_string(), self.destination_pubkey.clone()]);

        // e tag (base version, optional)
        if let Some(base_version) = &self.base_version {
            let mut e_tag = vec!["e".to_string(), base_version.clone()];
            if let Some(relay) = &self.base_relay {
                e_tag.push(relay.clone());
            }
            tags.push(e_tag);
        }

        // e tag (source version with marker)
        let mut source_tag = vec!["e".to_string(), self.source_version.clone()];
        if let Some(relay) = &self.source_relay {
            source_tag.push(relay.clone());
        } else {
            source_tag.push(String::new());
        }
        source_tag.push("source".to_string());
        tags.push(source_tag);

        tags
    }
}

/// Wiki redirect (kind 30819)
///
/// Indicates that one article should redirect to another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WikiRedirect {
    /// Source d tag (what redirects)
    pub d: String,
    /// Target article address
    pub target_address: String,
    /// Optional relay for target
    pub target_relay: Option<String>,
}

impl WikiRedirect {
    /// Create a new redirect
    pub fn new(d: impl Into<String>, target_address: impl Into<String>) -> Self {
        let d_str = d.into();
        Self {
            d: normalize_d_tag(&d_str),
            target_address: target_address.into(),
            target_relay: None,
        }
    }

    /// Validate the redirect
    pub fn validate(&self) -> Result<(), Nip54Error> {
        if self.d.is_empty() {
            return Err(Nip54Error::MissingField("d".to_string()));
        }
        if self.target_address.is_empty() {
            return Err(Nip54Error::MissingField("target_address".to_string()));
        }

        // Verify d tag is normalized
        let normalized = normalize_d_tag(&self.d);
        if self.d != normalized {
            return Err(Nip54Error::InvalidDTag(format!(
                "d tag '{}' is not normalized, should be '{}'",
                self.d, normalized
            )));
        }

        Ok(())
    }

    /// Convert to tags for event creation
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec!["d".to_string(), self.d.clone()]];

        let mut a_tag = vec!["a".to_string(), self.target_address.clone()];
        if let Some(relay) = &self.target_relay {
            a_tag.push(relay.clone());
        }
        tags.push(a_tag);

        tags
    }
}

/// Check if a kind is a NIP-54 kind
pub fn is_nip54_kind(kind: u16) -> bool {
    matches!(
        kind,
        KIND_WIKI_ARTICLE | KIND_WIKI_MERGE_REQUEST | KIND_WIKI_REDIRECT
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_d_tag() {
        assert_eq!(normalize_d_tag("Hello World"), "hello-world");
        assert_eq!(normalize_d_tag("Rust_Programming"), "rust-programming");
        assert_eq!(normalize_d_tag("NIP-54"), "nip---"); // digits are non-letters, converted to -
        assert_eq!(normalize_d_tag("Test123"), "test---");
        assert_eq!(normalize_d_tag("UPPERCASE"), "uppercase");
    }

    #[test]
    fn test_wiki_reference_fork() {
        let reference = WikiReference::fork("30818:pubkey:article", "event-id-123");
        assert_eq!(reference.marker, "fork");
        assert_eq!(reference.address, "30818:pubkey:article");
        assert_eq!(reference.event_id, Some("event-id-123".to_string()));
    }

    #[test]
    fn test_wiki_reference_defer() {
        let reference = WikiReference::defer("30818:pubkey:article", "event-id-456");
        assert_eq!(reference.marker, "defer");
        assert_eq!(reference.address, "30818:pubkey:article");
        assert_eq!(reference.event_id, Some("event-id-456".to_string()));
    }

    #[test]
    fn test_wiki_article_new() {
        let article = WikiArticle::new("Hello World", "Hello World Article");
        assert_eq!(article.d, "hello-world");
        assert_eq!(article.title, Some("Hello World Article".to_string()));
    }

    #[test]
    fn test_wiki_article_validate() {
        let article = WikiArticle::new("test", "Test");
        assert!(article.validate().is_ok());

        let invalid = WikiArticle::default();
        assert!(invalid.validate().is_err());

        // Invalid: not normalized
        let mut not_normalized = WikiArticle::default();
        not_normalized.d = "Hello World".to_string();
        assert!(not_normalized.validate().is_err());
    }

    #[test]
    fn test_wiki_article_to_tags() {
        let mut article = WikiArticle::new("rust", "Rust Programming");
        article.summary = Some("A systems language".to_string());
        article.add_fork(WikiReference::fork("30818:pubkey:old-rust", "event-123"));

        let tags = article.to_tags();
        assert!(tags.contains(&vec!["d".to_string(), "rust".to_string()]));
        assert!(tags.contains(&vec!["title".to_string(), "Rust Programming".to_string()]));
        assert!(tags.contains(&vec![
            "summary".to_string(),
            "A systems language".to_string()
        ]));

        // Check for fork tags
        assert!(
            tags.iter()
                .any(|t| t[0] == "a" && t.last() == Some(&"fork".to_string()))
        );
        assert!(
            tags.iter()
                .any(|t| t[0] == "e" && t.last() == Some(&"fork".to_string()))
        );
    }

    #[test]
    fn test_wiki_merge_request_new() {
        let request = WikiMergeRequest::new(
            "30818:dest-pubkey:article",
            "dest-pubkey",
            "source-event-id",
        );

        assert_eq!(request.target_address, "30818:dest-pubkey:article");
        assert_eq!(request.destination_pubkey, "dest-pubkey");
        assert_eq!(request.source_version, "source-event-id");
    }

    #[test]
    fn test_wiki_merge_request_validate() {
        let request = WikiMergeRequest::new("30818:dest:article", "dest-pubkey", "source-id");
        assert!(request.validate().is_ok());

        let invalid = WikiMergeRequest::new("", "dest", "source");
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_wiki_merge_request_to_tags() {
        let request = WikiMergeRequest::new("30818:dest:article", "dest-pubkey", "source-id")
            .with_base_version("base-id");

        let tags = request.to_tags();
        assert!(tags.contains(&vec!["a".to_string(), "30818:dest:article".to_string()]));
        assert!(tags.contains(&vec!["p".to_string(), "dest-pubkey".to_string()]));

        // Check for source tag with marker
        assert!(tags.iter().any(|t| t[0] == "e"
            && t[1] == "source-id"
            && t.last() == Some(&"source".to_string())));

        // Check for base version
        assert!(
            tags.iter()
                .any(|t| t[0] == "e" && t[1] == "base-id" && t.len() == 2)
        );
    }

    #[test]
    fn test_wiki_redirect_new() {
        let redirect = WikiRedirect::new("Shell Structure", "30818:pubkey:thin-shell-structure");
        assert_eq!(redirect.d, "shell-structure");
        assert_eq!(redirect.target_address, "30818:pubkey:thin-shell-structure");
    }

    #[test]
    fn test_wiki_redirect_validate() {
        let redirect = WikiRedirect::new("test", "30818:pubkey:target");
        assert!(redirect.validate().is_ok());

        let invalid = WikiRedirect::new("", "target");
        assert!(invalid.validate().is_err());

        // Invalid: not normalized
        let mut not_normalized = WikiRedirect::new("Test", "target");
        not_normalized.d = "Not Normalized".to_string();
        assert!(not_normalized.validate().is_err());
    }

    #[test]
    fn test_wiki_redirect_to_tags() {
        let redirect = WikiRedirect::new("old-name", "30818:pubkey:new-name");

        let tags = redirect.to_tags();
        assert!(tags.contains(&vec!["d".to_string(), "old-name".to_string()]));
        assert!(tags.contains(&vec!["a".to_string(), "30818:pubkey:new-name".to_string()]));
    }

    #[test]
    fn test_is_nip54_kind() {
        assert!(is_nip54_kind(KIND_WIKI_ARTICLE));
        assert!(is_nip54_kind(KIND_WIKI_MERGE_REQUEST));
        assert!(is_nip54_kind(KIND_WIKI_REDIRECT));
        assert!(!is_nip54_kind(1));
    }
}
