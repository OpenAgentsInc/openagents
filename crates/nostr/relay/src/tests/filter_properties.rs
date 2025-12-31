//! Property-based tests for Nostr Filter validation and matching
//!
//! These tests use proptest to verify that filter parsing and matching logic
//! handle edge cases correctly: empty filters, filters with all fields populated,
//! boundary values for since/until timestamps, and large arrays of kinds/authors/tags.

use crate::subscription::Filter;
use nostr::{Event, EventTemplate, finalize_event, generate_secret_key};
use proptest::prelude::*;
use std::collections::HashMap;

// =============================================================================
// Helper Functions
// =============================================================================

fn create_event_with_fields(
    kind: u16,
    content: &str,
    tags: Vec<Vec<String>>,
    created_at: u64,
) -> Event {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind,
        tags,
        content: content.to_string(),
        created_at,
    };
    finalize_event(&template, &secret_key).unwrap()
}

// =============================================================================
// Filter Validation Property Tests
// =============================================================================

// =============================================================================
// Tests without parameters (outside proptest! macro)
// =============================================================================

#[test]
fn prop_empty_filter_valid() {
    let filter = Filter::new();
    assert!(filter.validate().is_ok());
}

#[test]
fn prop_limit_boundary_5000_valid() {
    let mut filter = Filter::new();
    filter.limit = Some(5000);
    assert!(filter.validate().is_ok());
}

#[test]
fn prop_limit_boundary_5001_invalid() {
    let mut filter = Filter::new();
    filter.limit = Some(5001);
    assert!(filter.validate().is_err());
}

#[test]
fn prop_zero_limit_valid() {
    let mut filter = Filter::new();
    filter.limit = Some(0);
    assert!(filter.validate().is_ok());
}

#[test]
fn prop_empty_ids_matches_nothing() {
    let event = create_event_with_fields(1, "test", vec![], 1234567890);
    let mut filter = Filter::new();
    filter.ids = Some(vec![]);
    assert!(!filter.matches(&event));
}

#[test]
fn prop_empty_authors_matches_nothing() {
    let event = create_event_with_fields(1, "test", vec![], 1234567890);
    let mut filter = Filter::new();
    filter.authors = Some(vec![]);
    assert!(!filter.matches(&event));
}

#[test]
fn prop_empty_tag_filter_no_match() {
    let event = create_event_with_fields(
        1,
        "test",
        vec![vec!["e".to_string(), "event123".to_string()]],
        1234567890,
    );

    let mut filter = Filter::new();
    let mut tag_filters = HashMap::new();
    tag_filters.insert("#e".to_string(), vec![]);
    filter.tags = Some(tag_filters);
    assert!(!filter.matches(&event));
}

#[test]
fn prop_missing_tag_no_match() {
    let event = create_event_with_fields(
        1,
        "test",
        vec![vec!["e".to_string(), "event123".to_string()]],
        1234567890,
    );

    let mut filter = Filter::new();
    let mut tag_filters = HashMap::new();
    tag_filters.insert("#p".to_string(), vec!["pubkey456".to_string()]);
    filter.tags = Some(tag_filters);
    assert!(!filter.matches(&event));
}

#[test]
fn prop_no_tags_no_match() {
    let event = create_event_with_fields(1, "test", vec![], 1234567890);

    let mut filter = Filter::new();
    let mut tag_filters = HashMap::new();
    tag_filters.insert("#e".to_string(), vec!["event123".to_string()]);
    filter.tags = Some(tag_filters);
    assert!(!filter.matches(&event));
}

#[test]
fn prop_zero_timestamp_works() {
    let event = create_event_with_fields(1, "test", vec![], 0);
    let mut filter = Filter::new();
    filter.since = Some(0);
    filter.until = Some(100);
    assert!(filter.matches(&event));
}

#[test]
fn prop_max_timestamp_works() {
    let event = create_event_with_fields(1, "test", vec![], u64::MAX);
    let mut filter = Filter::new();
    filter.since = Some(u64::MAX - 1000);
    assert!(filter.matches(&event));
}

#[test]
fn prop_multiple_ids_or_condition() {
    let event1 = create_event_with_fields(1, "test1", vec![], 1234567890);
    let event2 = create_event_with_fields(1, "test2", vec![], 1234567891);

    let mut filter = Filter::new();
    filter.ids = Some(vec![event1.id[..8].to_string(), event2.id[..8].to_string()]);

    assert!(filter.matches(&event1));
    assert!(filter.matches(&event2));
}

#[test]
fn prop_multiple_authors_or_condition() {
    let event1 = create_event_with_fields(1, "test1", vec![], 1234567890);
    let event2 = create_event_with_fields(1, "test2", vec![], 1234567891);

    let mut filter = Filter::new();
    filter.authors = Some(vec![
        event1.pubkey[..8].to_string(),
        event2.pubkey[..8].to_string(),
    ]);

    assert!(filter.matches(&event1));
    assert!(filter.matches(&event2));
}

#[test]
fn prop_multiple_tag_filters_and_condition() {
    let event = create_event_with_fields(
        1,
        "test",
        vec![
            vec!["e".to_string(), "event123".to_string()],
            vec!["p".to_string(), "pubkey456".to_string()],
        ],
        1234567890,
    );

    let mut filter = Filter::new();
    let mut tag_filters = HashMap::new();
    tag_filters.insert("#e".to_string(), vec!["event123".to_string()]);
    tag_filters.insert("#p".to_string(), vec!["pubkey456".to_string()]);
    filter.tags = Some(tag_filters);

    assert!(filter.matches(&event));
}

proptest! {
    /// Property: Filter with only limit field is valid
    #[test]
    fn prop_limit_only_filter_valid(limit in 1usize..=5000usize) {
        let mut filter = Filter::new();
        filter.limit = Some(limit);
        prop_assert!(filter.validate().is_ok());
    }

    /// Property: Filter with limit > 5000 is invalid
    #[test]
    fn prop_excessive_limit_invalid(limit in 5001usize..10000usize) {
        let mut filter = Filter::new();
        filter.limit = Some(limit);
        prop_assert!(filter.validate().is_err());
    }
}

// =============================================================================
// Filter Matching Property Tests - Kinds
// =============================================================================

proptest! {
    /// Property: Filter with empty kinds array matches no events
    #[test]
    fn prop_empty_kinds_matches_nothing(kind in any::<u16>()) {
        let event = create_event_with_fields(kind, "test", vec![], 1234567890);
        let mut filter = Filter::new();
        filter.kinds = Some(vec![]);

        // Empty kinds array should match nothing
        prop_assert!(!filter.matches(&event));
    }

    /// Property: Filter with many kinds still works
    #[test]
    fn prop_many_kinds_matches(num_kinds in 1usize..100usize) {
        let kinds: Vec<u16> = (0..num_kinds).map(|i| (i % 65536) as u16).collect();
        let event = create_event_with_fields(kinds[0], "test", vec![], 1234567890);

        let mut filter = Filter::new();
        filter.kinds = Some(kinds);

        prop_assert!(filter.matches(&event));
    }

    /// Property: Filter matches event with exact kind
    #[test]
    fn prop_exact_kind_match(kind in any::<u16>()) {
        let event = create_event_with_fields(kind, "test", vec![], 1234567890);
        let mut filter = Filter::new();
        filter.kinds = Some(vec![kind]);

        prop_assert!(filter.matches(&event));
    }

    /// Property: Filter doesn't match event with different kind
    #[test]
    fn prop_different_kind_no_match(kind1 in any::<u16>(), kind2 in any::<u16>()) {
        prop_assume!(kind1 != kind2);

        let event = create_event_with_fields(kind1, "test", vec![], 1234567890);
        let mut filter = Filter::new();
        filter.kinds = Some(vec![kind2]);

        prop_assert!(!filter.matches(&event));
    }
}

// =============================================================================
// Filter Matching Property Tests - Timestamps
// =============================================================================

proptest! {
    /// Property: Filter with since = event.created_at matches
    #[test]
    fn prop_since_exact_match(timestamp in any::<u64>()) {
        let event = create_event_with_fields(1, "test", vec![], timestamp);
        let mut filter = Filter::new();
        filter.since = Some(timestamp);

        prop_assert!(filter.matches(&event));
    }

    /// Property: Filter with until = event.created_at matches
    #[test]
    fn prop_until_exact_match(timestamp in any::<u64>()) {
        let event = create_event_with_fields(1, "test", vec![], timestamp);
        let mut filter = Filter::new();
        filter.until = Some(timestamp);

        prop_assert!(filter.matches(&event));
    }

    /// Property: Filter with since > event.created_at doesn't match
    #[test]
    fn prop_since_after_event_no_match(timestamp in 1000u64..u64::MAX) {
        let event = create_event_with_fields(1, "test", vec![], timestamp);
        let mut filter = Filter::new();
        filter.since = Some(timestamp + 1);

        prop_assert!(!filter.matches(&event));
    }

    /// Property: Filter with until < event.created_at doesn't match
    #[test]
    fn prop_until_before_event_no_match(timestamp in 1u64..u64::MAX) {
        let event = create_event_with_fields(1, "test", vec![], timestamp);
        let mut filter = Filter::new();
        filter.until = Some(timestamp - 1);

        prop_assert!(!filter.matches(&event));
    }

    /// Property: Filter with both since and until creates range
    #[test]
    fn prop_since_until_range(
        since in 1000u64..2000u64,
        until in 2000u64..3000u64,
    ) {
        let event_before = create_event_with_fields(1, "test", vec![], since - 1);
        let event_in_range = create_event_with_fields(1, "test", vec![], (since + until) / 2);
        let event_after = create_event_with_fields(1, "test", vec![], until + 1);

        let mut filter = Filter::new();
        filter.since = Some(since);
        filter.until = Some(until);

        prop_assert!(!filter.matches(&event_before));
        prop_assert!(filter.matches(&event_in_range));
        prop_assert!(!filter.matches(&event_after));
    }

}

// =============================================================================
// Filter Matching Property Tests - IDs and Authors
// =============================================================================

proptest! {
    /// Property: Partial ID match works (prefix matching)
    #[test]
    fn prop_partial_id_match(prefix_len in 4usize..=64usize) {
        let event = create_event_with_fields(1, "test", vec![], 1234567890);
        let id_prefix = &event.id[..prefix_len];

        let mut filter = Filter::new();
        filter.ids = Some(vec![id_prefix.to_string()]);

        prop_assert!(filter.matches(&event));
    }

    /// Property: Partial author match works (prefix matching)
    #[test]
    fn prop_partial_author_match(prefix_len in 4usize..=64usize) {
        let event = create_event_with_fields(1, "test", vec![], 1234567890);
        let author_prefix = &event.pubkey[..prefix_len];

        let mut filter = Filter::new();
        filter.authors = Some(vec![author_prefix.to_string()]);

        prop_assert!(filter.matches(&event));
    }


    /// Property: Many IDs in filter still works
    #[test]
    fn prop_many_ids_works(num_ids in 1usize..50usize) {
        let event = create_event_with_fields(1, "test", vec![], 1234567890);
        let id_prefix = &event.id[..8];

        let mut ids = vec![id_prefix.to_string()];
        for i in 0..num_ids {
            ids.push(format!("deadbeef{:08x}", i));
        }

        let mut filter = Filter::new();
        filter.ids = Some(ids);

        prop_assert!(filter.matches(&event));
    }
}

// =============================================================================
// Filter Matching Property Tests - Tags
// =============================================================================

proptest! {
    /// Property: Tag filter matches prefix
    #[test]
    fn prop_tag_prefix_match(tag_len in 4usize..=32usize) {
        let full_tag_value = "a".repeat(32);
        let prefix = &full_tag_value[..tag_len];

        let event = create_event_with_fields(
            1,
            "test",
            vec![vec!["e".to_string(), full_tag_value.clone()]],
            1234567890,
        );

        let mut filter = Filter::new();
        let mut tag_filters = HashMap::new();
        tag_filters.insert("#e".to_string(), vec![prefix.to_string()]);
        filter.tags = Some(tag_filters);

        prop_assert!(filter.matches(&event));
    }

}

// =============================================================================
// Filter Matching Property Tests - Combined Conditions
// =============================================================================

proptest! {
    /// Property: All conditions must match (AND logic)
    #[test]
    fn prop_all_conditions_and_logic(kind in any::<u16>(), timestamp in 1000u64..2000u64) {
        let event = create_event_with_fields(kind, "test", vec![], timestamp);

        // Filter that matches all conditions
        let mut filter_match = Filter::new();
        filter_match.kinds = Some(vec![kind]);
        filter_match.since = Some(timestamp - 100);
        filter_match.until = Some(timestamp + 100);
        prop_assert!(filter_match.matches(&event));

        // Filter that fails on kind
        let mut filter_kind_fail = Filter::new();
        filter_kind_fail.kinds = Some(vec![kind.wrapping_add(1)]);
        filter_kind_fail.since = Some(timestamp - 100);
        prop_assert!(!filter_kind_fail.matches(&event));

        // Filter that fails on since
        let mut filter_since_fail = Filter::new();
        filter_since_fail.kinds = Some(vec![kind]);
        filter_since_fail.since = Some(timestamp + 100);
        prop_assert!(!filter_since_fail.matches(&event));
    }

    /// Property: Filter with all fields populated works
    #[test]
    fn prop_all_fields_populated(kind in any::<u16>(), timestamp in 1000u64..2000u64) {
        let event = create_event_with_fields(
            kind,
            "test",
            vec![vec!["e".to_string(), "event123".to_string()]],
            timestamp,
        );

        let mut filter = Filter::new();
        filter.ids = Some(vec![event.id[..8].to_string()]);
        filter.authors = Some(vec![event.pubkey[..8].to_string()]);
        filter.kinds = Some(vec![kind]);
        filter.since = Some(timestamp - 100);
        filter.until = Some(timestamp + 100);
        filter.limit = Some(100);

        let mut tag_filters = HashMap::new();
        tag_filters.insert("#e".to_string(), vec!["event123".to_string()]);
        filter.tags = Some(tag_filters);

        prop_assert!(filter.matches(&event));
    }
}

// =============================================================================
// Boundary Value Tests
// =============================================================================

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn test_filter_with_limit_0() {
        let mut filter = Filter::new();
        filter.limit = Some(0);
        assert!(filter.validate().is_ok());
    }

    #[test]
    fn test_filter_with_limit_5000() {
        let mut filter = Filter::new();
        filter.limit = Some(5000);
        assert!(filter.validate().is_ok());
    }

    #[test]
    fn test_filter_with_limit_5001() {
        let mut filter = Filter::new();
        filter.limit = Some(5001);
        assert!(filter.validate().is_err());
    }

    #[test]
    fn test_filter_with_timestamp_0() {
        let event = create_event_with_fields(1, "test", vec![], 0);
        let mut filter = Filter::new();
        filter.since = Some(0);
        assert!(filter.matches(&event));
    }

    #[test]
    fn test_filter_with_timestamp_max() {
        let event = create_event_with_fields(1, "test", vec![], u64::MAX);
        let mut filter = Filter::new();
        filter.until = Some(u64::MAX);
        assert!(filter.matches(&event));
    }

    #[test]
    fn test_filter_with_single_char_id_prefix() {
        let event = create_event_with_fields(1, "test", vec![], 1234567890);
        let id_prefix = &event.id[..1];

        let mut filter = Filter::new();
        filter.ids = Some(vec![id_prefix.to_string()]);
        assert!(filter.matches(&event));
    }

    #[test]
    fn test_filter_with_single_char_author_prefix() {
        let event = create_event_with_fields(1, "test", vec![], 1234567890);
        let author_prefix = &event.pubkey[..1];

        let mut filter = Filter::new();
        filter.authors = Some(vec![author_prefix.to_string()]);
        assert!(filter.matches(&event));
    }

    #[test]
    fn test_filter_matches_event_with_empty_tag() {
        let event = create_event_with_fields(1, "test", vec![vec![]], 1234567890);
        let filter = Filter::new();
        // Filter with no conditions should match
        assert!(filter.matches(&event));
    }

    #[test]
    fn test_filter_with_tag_no_value() {
        let event = create_event_with_fields(1, "test", vec![vec!["e".to_string()]], 1234567890);

        let mut filter = Filter::new();
        let mut tag_filters = HashMap::new();
        tag_filters.insert("#e".to_string(), vec!["event123".to_string()]);
        filter.tags = Some(tag_filters);

        // Event tag has no value, so shouldn't match
        assert!(!filter.matches(&event));
    }
}
