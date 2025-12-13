//! NIP-01 subscription filters.
//!
//! Filters define which events a client wants to receive. They support:
//! - Event IDs (or prefixes)
//! - Authors/pubkeys (or prefixes)
//! - Event kinds
//! - Time ranges (since/until)
//! - Tag queries (#e, #p, etc.)
//! - Result limits

use nostr::Event;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// NIP-01 Filter for subscription requests.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Filter {
    /// Event IDs (or prefixes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ids: Option<Vec<String>>,

    /// Authors (pubkeys or prefixes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,

    /// Event kinds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kinds: Option<Vec<u16>>,

    /// Events since timestamp (exclusive)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<u64>,

    /// Events until timestamp (inclusive)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<u64>,

    /// Maximum number of events to return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,

    /// Generic tag queries (#e, #p, etc.)
    /// The key should include the # prefix (e.g., "#e", "#p")
    #[serde(flatten)]
    pub tags: HashMap<String, Vec<String>>,
}

impl Filter {
    /// Create a new empty filter (matches all events).
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by event IDs.
    pub fn ids(mut self, ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.ids = Some(ids.into_iter().map(|s| s.into()).collect());
        self
    }

    /// Filter by authors.
    pub fn authors(mut self, authors: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.authors = Some(authors.into_iter().map(|s| s.into()).collect());
        self
    }

    /// Filter by kinds.
    pub fn kinds(mut self, kinds: impl IntoIterator<Item = u16>) -> Self {
        self.kinds = Some(kinds.into_iter().collect());
        self
    }

    /// Filter events since timestamp (exclusive).
    pub fn since(mut self, timestamp: u64) -> Self {
        self.since = Some(timestamp);
        self
    }

    /// Filter events until timestamp (inclusive).
    pub fn until(mut self, timestamp: u64) -> Self {
        self.until = Some(timestamp);
        self
    }

    /// Limit the number of results.
    pub fn limit(mut self, n: u64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Add a tag filter.
    pub fn tag(mut self, tag_name: &str, values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let key = if tag_name.starts_with('#') {
            tag_name.to_string()
        } else {
            format!("#{}", tag_name)
        };
        self.tags.insert(key, values.into_iter().map(|s| s.into()).collect());
        self
    }

    /// Filter by #e (event reference) tags.
    pub fn references_events(self, event_ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tag("e", event_ids)
    }

    /// Filter by #p (pubkey reference) tags.
    pub fn references_pubkeys(self, pubkeys: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tag("p", pubkeys)
    }

    /// Check if an event matches this filter.
    pub fn matches(&self, event: &Event) -> bool {
        // Check IDs (prefix match)
        if let Some(ref ids) = self.ids {
            if !ids.iter().any(|id| event.id.starts_with(id)) {
                return false;
            }
        }

        // Check authors (prefix match)
        if let Some(ref authors) = self.authors {
            if !authors.iter().any(|a| event.pubkey.starts_with(a)) {
                return false;
            }
        }

        // Check kinds
        if let Some(ref kinds) = self.kinds {
            if !kinds.contains(&event.kind) {
                return false;
            }
        }

        // Check since (exclusive)
        if let Some(since) = self.since {
            if event.created_at <= since {
                return false;
            }
        }

        // Check until (inclusive)
        if let Some(until) = self.until {
            if event.created_at > until {
                return false;
            }
        }

        // Check tag filters
        for (tag_key, values) in &self.tags {
            if !tag_key.starts_with('#') {
                continue;
            }
            let tag_letter = &tag_key[1..];

            let has_match = event.tags.iter().any(|tag| {
                if tag.len() >= 2 && tag[0] == tag_letter {
                    values.iter().any(|v| tag[1].starts_with(v))
                } else {
                    false
                }
            });

            if !has_match {
                return false;
            }
        }

        true
    }

    /// Check if this filter would match NIP-90 job requests.
    pub fn matches_job_requests(&self) -> bool {
        if let Some(ref kinds) = self.kinds {
            kinds.iter().any(|&k| k >= 5000 && k < 6000)
        } else {
            // No kind filter means it could match anything
            true
        }
    }

    /// Check if this filter would match NIP-90 job results.
    pub fn matches_job_results(&self) -> bool {
        if let Some(ref kinds) = self.kinds {
            kinds.iter().any(|&k| k >= 6000 && k < 7000)
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(id: &str, pubkey: &str, kind: u16, created_at: u64) -> Event {
        Event {
            id: id.to_string(),
            pubkey: pubkey.to_string(),
            created_at,
            kind,
            tags: vec![],
            content: "test".to_string(),
            sig: "sig".to_string(),
        }
    }

    fn make_event_with_tags(
        id: &str,
        pubkey: &str,
        kind: u16,
        created_at: u64,
        tags: Vec<Vec<String>>,
    ) -> Event {
        Event {
            id: id.to_string(),
            pubkey: pubkey.to_string(),
            created_at,
            kind,
            tags,
            content: "test".to_string(),
            sig: "sig".to_string(),
        }
    }

    #[test]
    fn test_empty_filter_matches_all() {
        let filter = Filter::new();
        assert!(filter.matches(&make_event("abc", "xyz", 1, 1000)));
    }

    #[test]
    fn test_filter_kinds() {
        let filter = Filter::new().kinds([1, 7]);

        assert!(filter.matches(&make_event("id", "pk", 1, 1000)));
        assert!(filter.matches(&make_event("id", "pk", 7, 1000)));
        assert!(!filter.matches(&make_event("id", "pk", 2, 1000)));
    }

    #[test]
    fn test_filter_authors_prefix() {
        let filter = Filter::new().authors(["abc"]);

        assert!(filter.matches(&make_event("id", "abc123", 1, 1000)));
        assert!(filter.matches(&make_event("id", "abcdef", 1, 1000)));
        assert!(!filter.matches(&make_event("id", "xyz123", 1, 1000)));
    }

    #[test]
    fn test_filter_ids_prefix() {
        let filter = Filter::new().ids(["abc"]);

        assert!(filter.matches(&make_event("abc123", "pk", 1, 1000)));
        assert!(!filter.matches(&make_event("xyz123", "pk", 1, 1000)));
    }

    #[test]
    fn test_filter_since_until() {
        let filter = Filter::new().since(1000).until(2000);

        // since is exclusive
        assert!(!filter.matches(&make_event("id", "pk", 1, 1000)));
        assert!(filter.matches(&make_event("id", "pk", 1, 1001)));
        // until is inclusive
        assert!(filter.matches(&make_event("id", "pk", 1, 2000)));
        assert!(!filter.matches(&make_event("id", "pk", 1, 2001)));
    }

    #[test]
    fn test_filter_tags() {
        let filter = Filter::new().tag("e", ["event123"]);

        let event_with_tag = make_event_with_tags(
            "id",
            "pk",
            1,
            1000,
            vec![vec!["e".to_string(), "event123".to_string()]],
        );
        let event_without_tag = make_event("id", "pk", 1, 1000);

        assert!(filter.matches(&event_with_tag));
        assert!(!filter.matches(&event_without_tag));
    }

    #[test]
    fn test_filter_combined() {
        let filter = Filter::new()
            .kinds([1])
            .authors(["abc"])
            .since(500)
            .limit(10);

        assert!(filter.matches(&make_event("id", "abc123", 1, 1000)));
        assert!(!filter.matches(&make_event("id", "xyz", 1, 1000))); // wrong author
        assert!(!filter.matches(&make_event("id", "abc123", 2, 1000))); // wrong kind
        assert!(!filter.matches(&make_event("id", "abc123", 1, 500))); // before since
    }

    #[test]
    fn test_filter_serialization() {
        let filter = Filter::new().kinds([1, 7]).limit(10).tag("p", ["pubkey1"]);

        let json = serde_json::to_string(&filter).unwrap();
        assert!(json.contains("\"kinds\":[1,7]"));
        assert!(json.contains("\"limit\":10"));
        assert!(json.contains("\"#p\":[\"pubkey1\"]"));
    }

    #[test]
    fn test_job_request_filter() {
        let filter = Filter::new().kinds([5050]);
        assert!(filter.matches_job_requests());
        assert!(!filter.matches_job_results());

        let filter = Filter::new().kinds([6050]);
        assert!(!filter.matches_job_requests());
        assert!(filter.matches_job_results());
    }
}
