//! Subscription management for Nostr relay connections.
//!
//! This module provides utilities for managing subscriptions across multiple relays.

use crate::message::Filter;
use std::collections::HashSet;
use uuid::Uuid;

/// Generate a unique subscription ID.
pub fn generate_subscription_id() -> String {
    Uuid::new_v4().to_string()[..8].to_string()
}

/// Builder for creating subscription filters.
#[derive(Debug, Clone, Default)]
pub struct SubscriptionBuilder {
    filters: Vec<Filter>,
}

impl SubscriptionBuilder {
    /// Create a new subscription builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a filter to the subscription.
    pub fn filter(mut self, filter: Filter) -> Self {
        self.filters.push(filter);
        self
    }

    /// Add a filter for specific event kinds.
    pub fn kinds(self, kinds: Vec<u16>) -> Self {
        self.filter(Filter::new().kinds(kinds))
    }

    /// Add a filter for events from specific authors.
    pub fn authors(self, authors: Vec<String>) -> Self {
        self.filter(Filter::new().authors(authors))
    }

    /// Add a filter for specific event IDs.
    pub fn ids(self, ids: Vec<String>) -> Self {
        self.filter(Filter::new().ids(ids))
    }

    /// Add a filter for channel messages (NIP-28 kind 42).
    pub fn channel_messages(self, channel_id: String) -> Self {
        self.filter(
            Filter::new()
                .kinds(vec![42]) // KIND_CHANNEL_MESSAGE
                .event_refs(vec![channel_id]),
        )
    }

    /// Add a filter for DVM job requests (NIP-90 kinds 5000-5999).
    pub fn dvm_requests(self, kinds: Vec<u16>) -> Self {
        // Filter to only valid DVM request kinds
        let valid_kinds: Vec<u16> = kinds
            .into_iter()
            .filter(|k| *k >= 5000 && *k <= 5999)
            .collect();
        self.filter(Filter::new().kinds(valid_kinds))
    }

    /// Add a filter for DVM job results (NIP-90 kinds 6000-6999).
    pub fn dvm_results(self, request_id: String) -> Self {
        self.filter(
            Filter::new()
                .kinds(vec![]) // Will be filled based on request kind
                .event_refs(vec![request_id]),
        )
    }

    /// Add a filter for DVM job feedback (NIP-90 kind 7000).
    pub fn dvm_feedback(self, request_id: String) -> Self {
        self.filter(
            Filter::new()
                .kinds(vec![7000])
                .event_refs(vec![request_id]),
        )
    }

    /// Build the subscription filters.
    pub fn build(self) -> Vec<Filter> {
        self.filters
    }
}

/// Tracks which relays have a specific subscription.
#[derive(Debug, Clone)]
pub struct SubscriptionTracker {
    /// Subscription ID
    pub id: String,
    /// Filters for this subscription
    pub filters: Vec<Filter>,
    /// Relays that have this subscription
    pub relays: HashSet<String>,
    /// Whether EOSE has been received from all relays
    pub all_eose: bool,
    /// Relays that have sent EOSE
    pub eose_relays: HashSet<String>,
}

impl SubscriptionTracker {
    /// Create a new subscription tracker.
    pub fn new(id: impl Into<String>, filters: Vec<Filter>) -> Self {
        Self {
            id: id.into(),
            filters,
            relays: HashSet::new(),
            all_eose: false,
            eose_relays: HashSet::new(),
        }
    }

    /// Add a relay to this subscription.
    pub fn add_relay(&mut self, relay_url: impl Into<String>) {
        self.relays.insert(relay_url.into());
        self.update_all_eose();
    }

    /// Remove a relay from this subscription.
    pub fn remove_relay(&mut self, relay_url: &str) {
        self.relays.remove(relay_url);
        self.eose_relays.remove(relay_url);
        self.update_all_eose();
    }

    /// Mark EOSE received from a relay.
    pub fn mark_eose(&mut self, relay_url: impl Into<String>) {
        self.eose_relays.insert(relay_url.into());
        self.update_all_eose();
    }

    /// Update all_eose flag.
    fn update_all_eose(&mut self) {
        self.all_eose = !self.relays.is_empty() && self.relays.len() == self.eose_relays.len();
    }

    /// Check if a relay has this subscription.
    pub fn has_relay(&self, relay_url: &str) -> bool {
        self.relays.contains(relay_url)
    }

    /// Get the number of relays with this subscription.
    pub fn relay_count(&self) -> usize {
        self.relays.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_subscription_id() {
        let id1 = generate_subscription_id();
        let id2 = generate_subscription_id();

        assert_eq!(id1.len(), 8);
        assert_eq!(id2.len(), 8);
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_subscription_builder_kinds() {
        let filters = SubscriptionBuilder::new()
            .kinds(vec![1, 4])
            .build();

        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].kinds, Some(vec![1, 4]));
    }

    #[test]
    fn test_subscription_builder_authors() {
        let filters = SubscriptionBuilder::new()
            .authors(vec!["author1".to_string(), "author2".to_string()])
            .build();

        assert_eq!(filters.len(), 1);
        assert_eq!(
            filters[0].authors,
            Some(vec!["author1".to_string(), "author2".to_string()])
        );
    }

    #[test]
    fn test_subscription_builder_channel_messages() {
        let filters = SubscriptionBuilder::new()
            .channel_messages("channel123".to_string())
            .build();

        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].kinds, Some(vec![42]));
        assert!(filters[0].tags.contains_key("#e"));
    }

    #[test]
    fn test_subscription_builder_multiple_filters() {
        let filters = SubscriptionBuilder::new()
            .kinds(vec![1])
            .authors(vec!["author1".to_string()])
            .build();

        assert_eq!(filters.len(), 2);
    }

    #[test]
    fn test_subscription_tracker_new() {
        let tracker = SubscriptionTracker::new("sub1", vec![Filter::new().kinds(vec![1])]);

        assert_eq!(tracker.id, "sub1");
        assert_eq!(tracker.filters.len(), 1);
        assert!(tracker.relays.is_empty());
        assert!(!tracker.all_eose);
    }

    #[test]
    fn test_subscription_tracker_add_relay() {
        let mut tracker = SubscriptionTracker::new("sub1", vec![]);

        tracker.add_relay("wss://relay1.com");
        tracker.add_relay("wss://relay2.com");

        assert_eq!(tracker.relay_count(), 2);
        assert!(tracker.has_relay("wss://relay1.com"));
        assert!(tracker.has_relay("wss://relay2.com"));
        assert!(!tracker.has_relay("wss://relay3.com"));
    }

    #[test]
    fn test_subscription_tracker_remove_relay() {
        let mut tracker = SubscriptionTracker::new("sub1", vec![]);

        tracker.add_relay("wss://relay1.com");
        tracker.add_relay("wss://relay2.com");
        tracker.remove_relay("wss://relay1.com");

        assert_eq!(tracker.relay_count(), 1);
        assert!(!tracker.has_relay("wss://relay1.com"));
        assert!(tracker.has_relay("wss://relay2.com"));
    }

    #[test]
    fn test_subscription_tracker_eose() {
        let mut tracker = SubscriptionTracker::new("sub1", vec![]);

        tracker.add_relay("wss://relay1.com");
        tracker.add_relay("wss://relay2.com");
        assert!(!tracker.all_eose);

        tracker.mark_eose("wss://relay1.com");
        assert!(!tracker.all_eose);

        tracker.mark_eose("wss://relay2.com");
        assert!(tracker.all_eose);
    }

    #[test]
    fn test_subscription_tracker_eose_after_remove() {
        let mut tracker = SubscriptionTracker::new("sub1", vec![]);

        tracker.add_relay("wss://relay1.com");
        tracker.add_relay("wss://relay2.com");
        tracker.mark_eose("wss://relay1.com");
        tracker.mark_eose("wss://relay2.com");
        assert!(tracker.all_eose);

        // Add a new relay - should reset all_eose
        tracker.add_relay("wss://relay3.com");
        assert!(!tracker.all_eose);

        tracker.mark_eose("wss://relay3.com");
        assert!(tracker.all_eose);
    }
}
