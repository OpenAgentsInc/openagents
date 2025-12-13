//! Subscription management for Nostr relay.
//!
//! Manages active subscriptions and determines which events should be
//! broadcast to which clients.

use crate::Filter;
use nostr::Event;
use std::collections::HashMap;

/// A subscription with its filters.
#[derive(Debug, Clone)]
pub struct Subscription {
    /// Subscription ID (client-provided)
    pub id: String,
    /// Filters for this subscription
    pub filters: Vec<Filter>,
}

impl Subscription {
    /// Create a new subscription.
    pub fn new(id: impl Into<String>, filters: Vec<Filter>) -> Self {
        Self {
            id: id.into(),
            filters,
        }
    }

    /// Check if an event matches any of this subscription's filters.
    pub fn matches(&self, event: &Event) -> bool {
        self.filters.iter().any(|f| f.matches(event))
    }

    /// Check if this subscription is interested in NIP-90 job requests.
    pub fn wants_job_requests(&self) -> bool {
        self.filters.iter().any(|f| f.matches_job_requests())
    }

    /// Check if this subscription is interested in NIP-90 job results.
    pub fn wants_job_results(&self) -> bool {
        self.filters.iter().any(|f| f.matches_job_results())
    }
}

/// Manages subscriptions for a single client connection.
#[derive(Debug, Default)]
pub struct SubscriptionManager {
    /// Active subscriptions by ID
    subscriptions: HashMap<String, Subscription>,
}

impl SubscriptionManager {
    /// Create a new subscription manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or replace a subscription.
    pub fn add(&mut self, id: impl Into<String>, filters: Vec<Filter>) {
        let id = id.into();
        self.subscriptions
            .insert(id.clone(), Subscription::new(id, filters));
    }

    /// Remove a subscription.
    pub fn remove(&mut self, id: &str) -> Option<Subscription> {
        self.subscriptions.remove(id)
    }

    /// Check if a subscription exists.
    pub fn has(&self, id: &str) -> bool {
        self.subscriptions.contains_key(id)
    }

    /// Get a subscription by ID.
    pub fn get(&self, id: &str) -> Option<&Subscription> {
        self.subscriptions.get(id)
    }

    /// Get all subscriptions that match an event.
    pub fn matching(&self, event: &Event) -> Vec<&Subscription> {
        self.subscriptions
            .values()
            .filter(|sub| sub.matches(event))
            .collect()
    }

    /// Get subscription IDs that match an event.
    pub fn matching_ids(&self, event: &Event) -> Vec<&str> {
        self.subscriptions
            .values()
            .filter(|sub| sub.matches(event))
            .map(|sub| sub.id.as_str())
            .collect()
    }

    /// Get the number of active subscriptions.
    pub fn len(&self) -> usize {
        self.subscriptions.len()
    }

    /// Check if there are no subscriptions.
    pub fn is_empty(&self) -> bool {
        self.subscriptions.is_empty()
    }

    /// Clear all subscriptions.
    pub fn clear(&mut self) {
        self.subscriptions.clear();
    }

    /// Iterate over all subscriptions.
    pub fn iter(&self) -> impl Iterator<Item = &Subscription> {
        self.subscriptions.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(kind: u16, pubkey: &str) -> Event {
        Event {
            id: "test".to_string(),
            pubkey: pubkey.to_string(),
            created_at: 1000,
            kind,
            tags: vec![],
            content: "test".to_string(),
            sig: "sig".to_string(),
        }
    }

    #[test]
    fn test_subscription_matches() {
        let sub = Subscription::new("sub1", vec![Filter::new().kinds([1])]);

        assert!(sub.matches(&make_event(1, "pk")));
        assert!(!sub.matches(&make_event(2, "pk")));
    }

    #[test]
    fn test_subscription_manager_add_remove() {
        let mut manager = SubscriptionManager::new();

        manager.add("sub1", vec![Filter::new().kinds([1])]);
        assert!(manager.has("sub1"));
        assert_eq!(manager.len(), 1);

        manager.remove("sub1");
        assert!(!manager.has("sub1"));
        assert!(manager.is_empty());
    }

    #[test]
    fn test_subscription_manager_matching() {
        let mut manager = SubscriptionManager::new();

        manager.add("sub1", vec![Filter::new().kinds([1])]);
        manager.add("sub2", vec![Filter::new().kinds([2])]);
        manager.add("sub3", vec![Filter::new().authors(["abc"])]);

        let event1 = make_event(1, "xyz");
        let matching = manager.matching_ids(&event1);
        assert_eq!(matching, vec!["sub1"]);

        let event2 = make_event(1, "abc123");
        let matching = manager.matching_ids(&event2);
        assert!(matching.contains(&"sub1"));
        assert!(matching.contains(&"sub3"));
    }

    #[test]
    fn test_subscription_replace() {
        let mut manager = SubscriptionManager::new();

        manager.add("sub1", vec![Filter::new().kinds([1])]);
        assert!(manager.get("sub1").unwrap().matches(&make_event(1, "pk")));

        // Replace with different filter
        manager.add("sub1", vec![Filter::new().kinds([2])]);
        assert!(!manager.get("sub1").unwrap().matches(&make_event(1, "pk")));
        assert!(manager.get("sub1").unwrap().matches(&make_event(2, "pk")));
    }
}
