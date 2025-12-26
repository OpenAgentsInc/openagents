//! Integration tests for repository watch and follow behavior

use gitafter::nostr::cache::EventCache;
use tempfile::TempDir;

#[test]
fn test_watch_and_unwatch_repository() {
    let temp_dir = TempDir::new().unwrap();
    let cache = EventCache::new(temp_dir.path().join("watch.db")).unwrap();

    cache
        .watch_repository("openagents", "30617:pubkey:openagents")
        .unwrap();

    assert!(cache.is_repository_watched("openagents").unwrap());

    let watched = cache.get_watched_repositories().unwrap();
    assert_eq!(watched, vec!["openagents".to_string()]);

    cache.unwatch_repository("openagents").unwrap();

    assert!(!cache.is_repository_watched("openagents").unwrap());
    let watched_after = cache.get_watched_repositories().unwrap();
    assert!(watched_after.is_empty());
}
