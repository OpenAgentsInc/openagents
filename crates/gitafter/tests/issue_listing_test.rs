//! Integration tests for GitAfter issue listing

use anyhow::Result;
use gitafter::nostr::cache::EventCache;
use nostr::Event;
use std::time::{SystemTime, UNIX_EPOCH};

fn issue_event(id: &str, repo_address: &str, title: &str, created_at: u64) -> Event {
    Event {
        id: id.to_string(),
        kind: 1621,
        pubkey: "test_pubkey".to_string(),
        created_at,
        content: format!("Issue body for {}", title),
        tags: vec![
            vec!["a".to_string(), repo_address.to_string()],
            vec!["subject".to_string(), title.to_string()],
            vec!["status".to_string(), "open".to_string()],
        ],
        sig: "test_signature".to_string(),
    }
}

#[test]
fn test_get_issues_by_repo_filters_and_sorts() -> Result<()> {
    let cache = EventCache::new_in_memory()?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let repo_address = "30617:test_pubkey:test-repo";
    let other_repo_address = "30617:other_pubkey:other-repo";

    let issue_old = issue_event("issue-old", repo_address, "Old Issue", now - 60);
    let issue_new = issue_event("issue-new", repo_address, "New Issue", now);
    let issue_other = issue_event("issue-other", other_repo_address, "Other Issue", now - 30);

    cache.insert_event(&issue_old)?;
    cache.insert_event(&issue_new)?;
    cache.insert_event(&issue_other)?;

    let issues = cache.get_issues_by_repo(repo_address, 10)?;

    assert_eq!(issues.len(), 2, "Should only return issues for the repo");
    assert_eq!(issues[0].id, "issue-new", "Newest issue should come first");
    assert_eq!(issues[1].id, "issue-old", "Oldest issue should come last");

    Ok(())
}

#[test]
fn test_search_issues_matches_labels() -> Result<()> {
    let cache = EventCache::new_in_memory()?;

    let issue = Event {
        id: "issue-labeled".to_string(),
        kind: 1621,
        pubkey: "test_pubkey".to_string(),
        created_at: 123,
        content: "Fix the rendering bug".to_string(),
        tags: vec![
            vec!["a".to_string(), "30617:test_pubkey:test-repo".to_string()],
            vec!["subject".to_string(), "Rendering glitch".to_string()],
            vec!["t".to_string(), "bug".to_string()],
        ],
        sig: "test_signature".to_string(),
    };

    cache.insert_event(&issue)?;

    let results = cache.search_issues("bug", 10)?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "issue-labeled");

    Ok(())
}
