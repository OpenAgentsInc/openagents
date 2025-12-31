//! NIP-50: Search Capability
//!
//! Implements search filter field for REQ messages, enabling full-text search across events.
//! Relays interpret search queries and return matching events sorted by relevance.
//!
//! Features:
//! - Human-readable search queries (e.g., "best nostr apps")
//! - Extension support (key:value pairs)
//! - Standard extensions: include:spam, domain:, language:, sentiment:, nsfw:
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/50.md>

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during NIP-50 operations
#[derive(Debug, Error)]
pub enum Nip50Error {
    #[error("invalid search query: {0}")]
    InvalidQuery(String),

    #[error("invalid extension: {0}")]
    InvalidExtension(String),

    #[error("invalid language code: {0}")]
    InvalidLanguageCode(String),

    #[error("invalid sentiment: {0}")]
    InvalidSentiment(String),
}

/// Sentiment filter values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sentiment {
    Negative,
    Neutral,
    Positive,
}

impl Sentiment {
    /// Parse sentiment from string
    pub fn parse(s: &str) -> Result<Self, Nip50Error> {
        match s.to_lowercase().as_str() {
            "negative" => Ok(Sentiment::Negative),
            "neutral" => Ok(Sentiment::Neutral),
            "positive" => Ok(Sentiment::Positive),
            _ => Err(Nip50Error::InvalidSentiment(s.to_string())),
        }
    }

    /// Convert to string
    pub fn to_string(&self) -> &str {
        match self {
            Sentiment::Negative => "negative",
            Sentiment::Neutral => "neutral",
            Sentiment::Positive => "positive",
        }
    }
}

/// Search query extensions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchExtensions {
    /// Include spam in results (default: false)
    pub include_spam: bool,

    /// Filter by NIP-05 domain
    pub domain: Option<String>,

    /// Filter by ISO 639-1 language code (2 letters)
    pub language: Option<String>,

    /// Filter by sentiment
    pub sentiment: Option<Sentiment>,

    /// Include NSFW content (default: true)
    pub nsfw: bool,
}

impl Default for SearchExtensions {
    fn default() -> Self {
        Self {
            include_spam: false,
            domain: None,
            language: None,
            sentiment: None,
            nsfw: true,
        }
    }
}

impl SearchExtensions {
    /// Create new search extensions with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set include_spam
    pub fn with_include_spam(mut self, include: bool) -> Self {
        self.include_spam = include;
        self
    }

    /// Set domain filter
    pub fn with_domain(mut self, domain: String) -> Self {
        self.domain = Some(domain);
        self
    }

    /// Set language filter (2-letter ISO 639-1 code)
    pub fn with_language(mut self, language: String) -> Result<Self, Nip50Error> {
        if language.len() != 2 {
            return Err(Nip50Error::InvalidLanguageCode(format!(
                "expected 2-letter ISO 639-1 code, got '{}'",
                language
            )));
        }
        self.language = Some(language.to_lowercase());
        Ok(self)
    }

    /// Set sentiment filter
    pub fn with_sentiment(mut self, sentiment: Sentiment) -> Self {
        self.sentiment = Some(sentiment);
        self
    }

    /// Set NSFW filter
    pub fn with_nsfw(mut self, nsfw: bool) -> Self {
        self.nsfw = nsfw;
        self
    }

    /// Parse extensions from query string
    ///
    /// Extracts key:value pairs from the query string
    pub fn parse_from_query(query: &str) -> (String, Self) {
        let mut extensions = Self::new();
        let mut base_query_parts = Vec::new();

        for word in query.split_whitespace() {
            if let Some((key, value)) = word.split_once(':') {
                // This is a key:value extension
                match key.to_lowercase().as_str() {
                    "include" if value == "spam" => {
                        extensions.include_spam = true;
                    }
                    "domain" => {
                        extensions.domain = Some(value.to_string());
                    }
                    "language" => {
                        if value.len() == 2 {
                            extensions.language = Some(value.to_lowercase());
                        }
                    }
                    "sentiment" => {
                        if let Ok(s) = Sentiment::parse(value) {
                            extensions.sentiment = Some(s);
                        }
                    }
                    "nsfw" => {
                        extensions.nsfw = value.to_lowercase() != "false";
                    }
                    _ => {
                        // Unknown extension, keep in base query
                        base_query_parts.push(word);
                    }
                }
            } else {
                // Regular search term
                base_query_parts.push(word);
            }
        }

        let base_query = base_query_parts.join(" ");
        (base_query, extensions)
    }

    /// Build query string with extensions
    pub fn to_query_string(&self, base_query: &str) -> String {
        let mut parts = vec![base_query.to_string()];

        if self.include_spam {
            parts.push("include:spam".to_string());
        }

        if let Some(ref domain) = self.domain {
            parts.push(format!("domain:{}", domain));
        }

        if let Some(ref lang) = self.language {
            parts.push(format!("language:{}", lang));
        }

        if let Some(sentiment) = self.sentiment {
            parts.push(format!("sentiment:{}", sentiment.to_string()));
        }

        if !self.nsfw {
            parts.push("nsfw:false".to_string());
        }

        parts.join(" ")
    }
}

/// Search query with optional extensions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchQuery {
    /// Base search query (human-readable)
    pub query: String,

    /// Search extensions
    pub extensions: SearchExtensions,
}

impl SearchQuery {
    /// Create new search query
    pub fn new(query: String) -> Self {
        Self {
            query,
            extensions: SearchExtensions::new(),
        }
    }

    /// Parse from full query string (including extensions)
    pub fn parse(full_query: &str) -> Self {
        let (base_query, extensions) = SearchExtensions::parse_from_query(full_query);
        Self {
            query: base_query,
            extensions,
        }
    }

    /// Convert to full query string
    pub fn to_string(&self) -> String {
        self.extensions.to_query_string(&self.query)
    }

    /// Set extensions
    pub fn with_extensions(mut self, extensions: SearchExtensions) -> Self {
        self.extensions = extensions;
        self
    }
}

/// Validate that a query string is reasonable
pub fn validate_query(query: &str) -> Result<(), Nip50Error> {
    if query.is_empty() {
        return Err(Nip50Error::InvalidQuery(
            "query cannot be empty".to_string(),
        ));
    }

    if query.len() > 1000 {
        return Err(Nip50Error::InvalidQuery(
            "query is too long (max 1000 chars)".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sentiment_parse() {
        assert_eq!(Sentiment::parse("negative").unwrap(), Sentiment::Negative);
        assert_eq!(Sentiment::parse("neutral").unwrap(), Sentiment::Neutral);
        assert_eq!(Sentiment::parse("positive").unwrap(), Sentiment::Positive);
        assert_eq!(Sentiment::parse("POSITIVE").unwrap(), Sentiment::Positive);
        assert!(Sentiment::parse("unknown").is_err());
    }

    #[test]
    fn test_sentiment_to_string() {
        assert_eq!(Sentiment::Negative.to_string(), "negative");
        assert_eq!(Sentiment::Neutral.to_string(), "neutral");
        assert_eq!(Sentiment::Positive.to_string(), "positive");
    }

    #[test]
    fn test_search_extensions_default() {
        let ext = SearchExtensions::new();
        assert!(!ext.include_spam);
        assert_eq!(ext.domain, None);
        assert_eq!(ext.language, None);
        assert_eq!(ext.sentiment, None);
        assert!(ext.nsfw);
    }

    #[test]
    fn test_search_extensions_builder() {
        let ext = SearchExtensions::new()
            .with_include_spam(true)
            .with_domain("nostr.com".to_string())
            .with_language("en".to_string())
            .unwrap()
            .with_sentiment(Sentiment::Positive)
            .with_nsfw(false);

        assert!(ext.include_spam);
        assert_eq!(ext.domain, Some("nostr.com".to_string()));
        assert_eq!(ext.language, Some("en".to_string()));
        assert_eq!(ext.sentiment, Some(Sentiment::Positive));
        assert!(!ext.nsfw);
    }

    #[test]
    fn test_search_extensions_invalid_language() {
        let result = SearchExtensions::new().with_language("eng".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_from_query_simple() {
        let (base, ext) = SearchExtensions::parse_from_query("best nostr apps");
        assert_eq!(base, "best nostr apps");
        assert!(!ext.include_spam);
        assert_eq!(ext.domain, None);
    }

    #[test]
    fn test_parse_from_query_with_extensions() {
        let (base, ext) = SearchExtensions::parse_from_query(
            "best nostr apps domain:nostr.com language:en sentiment:positive",
        );
        assert_eq!(base, "best nostr apps");
        assert_eq!(ext.domain, Some("nostr.com".to_string()));
        assert_eq!(ext.language, Some("en".to_string()));
        assert_eq!(ext.sentiment, Some(Sentiment::Positive));
    }

    #[test]
    fn test_parse_from_query_include_spam() {
        let (base, ext) = SearchExtensions::parse_from_query("search terms include:spam");
        assert_eq!(base, "search terms");
        assert!(ext.include_spam);
    }

    #[test]
    fn test_parse_from_query_nsfw() {
        let (base, ext) = SearchExtensions::parse_from_query("query nsfw:false");
        assert_eq!(base, "query");
        assert!(!ext.nsfw);
    }

    #[test]
    fn test_to_query_string_simple() {
        let ext = SearchExtensions::new();
        let query = ext.to_query_string("best nostr apps");
        assert_eq!(query, "best nostr apps");
    }

    #[test]
    fn test_to_query_string_with_extensions() {
        let ext = SearchExtensions::new()
            .with_domain("nostr.com".to_string())
            .with_language("en".to_string())
            .unwrap()
            .with_sentiment(Sentiment::Positive);

        let query = ext.to_query_string("best apps");
        assert!(query.contains("best apps"));
        assert!(query.contains("domain:nostr.com"));
        assert!(query.contains("language:en"));
        assert!(query.contains("sentiment:positive"));
    }

    #[test]
    fn test_search_query_new() {
        let query = SearchQuery::new("test query".to_string());
        assert_eq!(query.query, "test query");
        assert!(!query.extensions.include_spam);
    }

    #[test]
    fn test_search_query_parse() {
        let query = SearchQuery::parse("nostr apps domain:nostr.com language:en");
        assert_eq!(query.query, "nostr apps");
        assert_eq!(query.extensions.domain, Some("nostr.com".to_string()));
        assert_eq!(query.extensions.language, Some("en".to_string()));
    }

    #[test]
    fn test_search_query_roundtrip() {
        let original = SearchQuery::new("best nostr apps".to_string()).with_extensions(
            SearchExtensions::new()
                .with_domain("nostr.com".to_string())
                .with_language("en".to_string())
                .unwrap(),
        );

        let query_string = original.to_string();
        let parsed = SearchQuery::parse(&query_string);

        assert_eq!(parsed.query, original.query);
        assert_eq!(parsed.extensions.domain, original.extensions.domain);
        assert_eq!(parsed.extensions.language, original.extensions.language);
    }

    #[test]
    fn test_validate_query() {
        assert!(validate_query("valid query").is_ok());
        assert!(validate_query("").is_err());
        assert!(validate_query(&"a".repeat(1001)).is_err());
    }

    #[test]
    fn test_search_query_unknown_extension() {
        let (base, _ext) = SearchExtensions::parse_from_query("query custom:value");
        // Unknown extensions should be kept in base query
        assert!(base.contains("custom:value"));
    }
}
