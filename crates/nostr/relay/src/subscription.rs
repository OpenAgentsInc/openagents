//! Subscription management and event filtering
//!
//! Manages client subscriptions and filters events according to Nostr protocol filters.
//! Filters support: ids, authors, kinds, tags, since, until, and limit.

use crate::error::{RelayError, Result};
use nostr::Event;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Nostr subscription filter
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Filter {
    /// List of event IDs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ids: Option<Vec<String>>,

    /// List of author pubkeys
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,

    /// List of event kinds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kinds: Option<Vec<u16>>,

    /// Generic tag filters (e.g., #e for event references, #p for pubkey references)
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<HashMap<String, Vec<String>>>,

    /// Events must be newer than this (Unix timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<u64>,

    /// Events must be older than this (Unix timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<u64>,

    /// Maximum number of events to return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

impl Filter {
    /// Create a new empty filter
    pub fn new() -> Self {
        Self {
            ids: None,
            authors: None,
            kinds: None,
            tags: None,
            since: None,
            until: None,
            limit: None,
        }
    }

    /// Check if an event matches this filter
    pub fn matches(&self, event: &Event) -> bool {
        // Check IDs
        if let Some(ref ids) = self.ids
            && !ids.iter().any(|id| event.id.starts_with(id))
        {
            return false;
        }

        // Check authors
        if let Some(ref authors) = self.authors
            && !authors
                .iter()
                .any(|author| event.pubkey.starts_with(author))
        {
            return false;
        }

        // Check kinds
        if let Some(ref kinds) = self.kinds
            && !kinds.contains(&event.kind)
        {
            return false;
        }

        // Check timestamp (since)
        if let Some(since) = self.since
            && event.created_at < since
        {
            return false;
        }

        // Check timestamp (until)
        if let Some(until) = self.until
            && event.created_at > until
        {
            return false;
        }

        // Check tags
        if let Some(ref tag_filters) = self.tags {
            for (tag_name, tag_values) in tag_filters {
                // Tag filters are in the format "#e": ["event_id1", "event_id2"]
                // We need to check if the event has any tag matching this filter
                let tag_key = tag_name.trim_start_matches('#');

                let has_matching_tag = event.tags.iter().any(|event_tag| {
                    if event_tag.is_empty() {
                        return false;
                    }

                    // First element is the tag name
                    if event_tag[0] != tag_key {
                        return false;
                    }

                    // If there's no second element, we can't match a value
                    if event_tag.len() < 2 {
                        return false;
                    }

                    // Check if any of the filter values match
                    tag_values
                        .iter()
                        .any(|filter_value| event_tag[1].starts_with(filter_value))
                });

                if !has_matching_tag {
                    return false;
                }
            }
        }

        true
    }

    /// Check if this filter is valid
    pub fn validate(&self) -> Result<()> {
        // Ensure limit is reasonable
        if let Some(limit) = self.limit
            && limit > 5000
        {
            return Err(RelayError::Subscription(
                "limit too large (max 5000)".to_string(),
            ));
        }

        Ok(())
    }
}

impl Default for Filter {
    fn default() -> Self {
        Self::new()
    }
}

/// A client subscription
#[derive(Debug, Clone)]
pub struct Subscription {
    /// Subscription ID
    pub id: String,

    /// Filters for this subscription
    pub filters: Vec<Filter>,
}

impl Subscription {
    /// Create a new subscription
    pub fn new(id: String, filters: Vec<Filter>) -> Self {
        Self { id, filters }
    }

    /// Check if an event matches any filter in this subscription
    pub fn matches(&self, event: &Event) -> bool {
        self.filters.iter().any(|filter| filter.matches(event))
    }
}

/// Manages all subscriptions for a connection
#[derive(Debug, Default, Clone)]
pub struct SubscriptionManager {
    subscriptions: HashMap<String, Subscription>,
}

impl SubscriptionManager {
    /// Create a new subscription manager
    pub fn new() -> Self {
        Self {
            subscriptions: HashMap::new(),
        }
    }

    /// Add a subscription
    pub fn add(&mut self, subscription: Subscription) {
        self.subscriptions
            .insert(subscription.id.clone(), subscription);
    }

    /// Remove a subscription
    pub fn remove(&mut self, subscription_id: &str) -> bool {
        self.subscriptions.remove(subscription_id).is_some()
    }

    /// Get a subscription by ID
    pub fn get(&self, subscription_id: &str) -> Option<&Subscription> {
        self.subscriptions.get(subscription_id)
    }

    /// Check if an event matches any subscription
    pub fn matches_any(&self, event: &Event) -> Vec<String> {
        self.subscriptions
            .values()
            .filter(|sub| sub.matches(event))
            .map(|sub| sub.id.clone())
            .collect()
    }

    /// Get all subscription IDs
    pub fn subscription_ids(&self) -> Vec<String> {
        self.subscriptions.keys().cloned().collect()
    }

    /// Clear all subscriptions
    pub fn clear(&mut self) {
        self.subscriptions.clear();
    }

    /// Get number of subscriptions
    pub fn len(&self) -> usize {
        self.subscriptions.len()
    }

    /// Check if there are no subscriptions
    pub fn is_empty(&self) -> bool {
        self.subscriptions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::{EventTemplate, finalize_event, generate_secret_key};

    fn create_test_event(kind: u16, content: &str) -> Event {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            kind,
            tags: vec![],
            content: content.to_string(),
            created_at: 1234567890,
        };
        finalize_event(&template, &secret_key).unwrap()
    }

    fn create_event_with_tags(kind: u16, tags: Vec<Vec<String>>) -> Event {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            kind,
            tags,
            content: "test".to_string(),
            created_at: 1234567890,
        };
        finalize_event(&template, &secret_key).unwrap()
    }

    #[test]
    fn test_filter_kinds() {
        let mut filter = Filter::new();
        filter.kinds = Some(vec![1, 2, 3]);

        let event1 = create_test_event(1, "test");
        let event2 = create_test_event(4, "test");

        assert!(filter.matches(&event1));
        assert!(!filter.matches(&event2));
    }

    #[test]
    fn test_filter_authors() {
        let event = create_test_event(1, "test");
        let pubkey = event.pubkey.clone();

        let mut filter = Filter::new();
        filter.authors = Some(vec![pubkey[..8].to_string()]);

        assert!(filter.matches(&event));

        filter.authors = Some(vec!["different".to_string()]);
        assert!(!filter.matches(&event));
    }

    #[test]
    fn test_filter_ids() {
        let event = create_test_event(1, "test");
        let event_id = event.id.clone();

        let mut filter = Filter::new();
        filter.ids = Some(vec![event_id[..8].to_string()]);

        assert!(filter.matches(&event));

        filter.ids = Some(vec!["different".to_string()]);
        assert!(!filter.matches(&event));
    }

    #[test]
    fn test_filter_since_until() {
        let event = create_test_event(1, "test");

        let mut filter = Filter::new();
        filter.since = Some(1234567800);
        filter.until = Some(1234567900);

        assert!(filter.matches(&event));

        filter.since = Some(1234567900);
        assert!(!filter.matches(&event));

        filter.since = None;
        filter.until = Some(1234567800);
        assert!(!filter.matches(&event));
    }

    #[test]
    fn test_filter_tags() {
        let tags = vec![
            vec!["e".to_string(), "event123".to_string()],
            vec!["p".to_string(), "pubkey456".to_string()],
        ];
        let event = create_event_with_tags(1, tags);

        let mut filter = Filter::new();
        let mut tag_filters = HashMap::new();
        tag_filters.insert("#e".to_string(), vec!["event123".to_string()]);
        filter.tags = Some(tag_filters);

        assert!(filter.matches(&event));

        let mut tag_filters2 = HashMap::new();
        tag_filters2.insert("#e".to_string(), vec!["different".to_string()]);
        filter.tags = Some(tag_filters2);
        assert!(!filter.matches(&event));
    }

    #[test]
    fn test_filter_multiple_conditions() {
        let event = create_test_event(1, "test");
        let pubkey = event.pubkey.clone();

        let mut filter = Filter::new();
        filter.kinds = Some(vec![1]);
        filter.authors = Some(vec![pubkey[..8].to_string()]);

        assert!(filter.matches(&event));

        filter.kinds = Some(vec![2]);
        assert!(!filter.matches(&event));
    }

    #[test]
    fn test_subscription_matches() {
        let event1 = create_test_event(1, "test");
        let event2 = create_test_event(2, "test");

        let mut filter1 = Filter::new();
        filter1.kinds = Some(vec![1]);

        let mut filter2 = Filter::new();
        filter2.kinds = Some(vec![3]);

        let sub = Subscription::new("sub1".to_string(), vec![filter1, filter2]);

        assert!(sub.matches(&event1));
        assert!(!sub.matches(&event2));
    }

    #[test]
    fn test_subscription_manager() {
        let mut manager = SubscriptionManager::new();
        assert!(manager.is_empty());

        let mut filter = Filter::new();
        filter.kinds = Some(vec![1]);
        let sub = Subscription::new("sub1".to_string(), vec![filter]);

        manager.add(sub);
        assert_eq!(manager.len(), 1);
        assert!(manager.get("sub1").is_some());

        let event = create_test_event(1, "test");
        let matching = manager.matches_any(&event);
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0], "sub1");

        assert!(manager.remove("sub1"));
        assert!(manager.is_empty());
    }

    #[test]
    fn test_filter_validation() {
        let mut filter = Filter::new();
        assert!(filter.validate().is_ok());

        filter.limit = Some(4000);
        assert!(filter.validate().is_ok());

        filter.limit = Some(6000);
        assert!(filter.validate().is_err());
    }
}
