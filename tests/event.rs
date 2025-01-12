use openagents::event::Event;
use std::collections::HashSet;

#[test]
fn test_event_validation() {
    let event = Event {
        id: "a6b6c6d6e6f6".into(),
        pubkey: "0123456789abcdef".into(),
        created_at: 1234567890,
        kind: 1,
        tags: vec![],
        content: "test".into(),
        sig: "0123456789abcdef".into(),
        tagidx: None,
    };

    // This will fail since we're using dummy values
    assert!(event.validate().is_err());
}

#[test]
fn test_canonical_serialization() {
    let event = Event {
        id: "a6b6c6d6e6f6".into(),
        pubkey: "0123456789abcdef".into(),
        created_at: 1234567890,
        kind: 1,
        tags: vec![vec!["e".into(), "123".into()]],
        content: "test".into(),
        sig: "0123456789abcdef".into(),
        tagidx: None,
    };

    let canonical = event.to_canonical().unwrap();
    assert!(canonical.starts_with("[0,"));
    assert!(canonical.contains("test"));
}

#[test]
fn test_tag_indexing() {
    let mut event = Event {
        id: "a6b6c6d6e6f6".into(),
        pubkey: "0123456789abcdef".into(),
        created_at: 1234567890,
        kind: 1,
        tags: vec![
            vec!["e".into(), "123".into()],
            vec!["p".into(), "456".into()],
        ],
        content: "test".into(),
        sig: "0123456789abcdef".into(),
        tagidx: None,
    };

    event.build_index();

    let mut check = HashSet::new();
    check.insert("123".into());

    assert!(event.generic_tag_val_intersect('e', &check));

    check.clear();
    check.insert("789".into());
    assert!(!event.generic_tag_val_intersect('e', &check));
}
