use openagents::nostr::event::Event;
use openagents::nostr::subscription::ReqFilter;
use std::collections::HashMap;

#[test]
fn test_filter_matching() {
    let mut event = Event {
        id: "abc123".into(),
        pubkey: "def456".into(),
        created_at: 1000,
        kind: 1,
        tags: vec![vec!["e".into(), "789".into()]],
        content: "test".into(),
        sig: "".into(),
        tagidx: None,
    };
    event.build_index();

    let filter = ReqFilter {
        ids: Some(vec!["abc".into()]),
        authors: Some(vec!["def".into()]),
        kinds: Some(vec![1]),
        since: Some(500),
        until: Some(1500),
        limit: None,
        tags: {
            let mut map = HashMap::new();
            map.insert("#e".into(), vec!["789".into()]);
            map
        },
    };

    assert!(filter.matches_event(&event));
}

#[test]
fn test_filter_non_matching() {
    let mut event = Event {
        id: "abc123".into(),
        pubkey: "def456".into(),
        created_at: 1000,
        kind: 1,
        tags: vec![vec!["e".into(), "789".into()]],
        content: "test".into(),
        sig: "".into(),
        tagidx: None,
    };
    event.build_index();

    let filter = ReqFilter {
        ids: Some(vec!["xyz".into()]), // Won't match
        authors: Some(vec!["def".into()]),
        kinds: Some(vec![1]),
        since: Some(500),
        until: Some(1500),
        limit: None,
        tags: HashMap::new(),
    };

    assert!(!filter.matches_event(&event));
}
