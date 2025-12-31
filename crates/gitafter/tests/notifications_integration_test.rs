//! Integration tests for GitAfter notification system

use bip39::Mnemonic;
use gitafter::nostr::cache::EventCache;
use nostr::EventTemplate;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::TempDir;
use wallet::core::identity::UnifiedIdentity;

/// Helper to create a test cache
/// Returns (EventCache, TempDir) - caller must keep TempDir alive
fn create_test_cache() -> (EventCache, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test-notifications.db");
    let cache = EventCache::new(db_path).unwrap();
    (cache, temp_dir)
}

/// Test that notifications are created when a PR receives a review comment
#[test]
fn test_notification_on_pr_review() {
    // Create two identities: PR author and reviewer
    let author_mnemonic = Mnemonic::parse("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about").unwrap();
    let author_identity = UnifiedIdentity::from_mnemonic(author_mnemonic).unwrap();
    let author_pubkey = author_identity.nostr_public_key();

    let reviewer_mnemonic =
        Mnemonic::parse("zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong").unwrap();
    let reviewer_identity = UnifiedIdentity::from_mnemonic(reviewer_mnemonic).unwrap();

    let (cache, _temp_dir) = create_test_cache();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Step 1: Author creates a PR (kind:1618)
    let pr_template = EventTemplate {
        kind: 1618,
        content: "Fix authentication bug".to_string(),
        tags: vec![
            vec!["a".to_string(), "30617:test-owner:test-repo".to_string()],
            vec!["subject".to_string(), "Fix auth bug".to_string()],
            vec!["c".to_string(), "abc123".to_string()],
        ],
        created_at: now,
    };

    let pr_event = author_identity.sign_event(pr_template).unwrap();
    let pr_id = pr_event.id.clone();

    // Insert PR event
    cache.insert_event(&pr_event).unwrap();

    // Step 2: Reviewer posts a review comment (kind:1) referencing the PR
    let review_template = EventTemplate {
        kind: 1,
        content: "Looks good! Just one suggestion on line 42.".to_string(),
        tags: vec![
            vec!["e".to_string(), pr_id.clone()],
            vec!["p".to_string(), author_pubkey.to_string()],
        ],
        created_at: now + 10,
    };

    let review_event = reviewer_identity.sign_event(review_template).unwrap();
    cache.insert_event(&review_event).unwrap();

    // Step 3: Verify PR event exists in cache
    let cached_pr = cache.get_event(&pr_id).unwrap();
    assert!(cached_pr.is_some());

    // Step 4: Create notification for the review
    let notification_id = cache
        .create_notification(
            &author_pubkey.to_string(),
            &review_event.id,
            1,
            "pr_review",
            "New review on your PR",
            Some("Looks good! Just one suggestion on line 42."),
        )
        .unwrap();

    // Step 5: Verify notification was created
    let notifications = cache
        .get_notifications(&author_pubkey.to_string(), 10)
        .unwrap();
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].id, notification_id);
    assert_eq!(notifications[0].user_pubkey, author_pubkey.to_string());
    assert_eq!(notifications[0].event_id, review_event.id);
    assert_eq!(notifications[0].notification_type, "pr_review");
    assert!(!notifications[0].read);

    // Step 6: Verify unread count
    let unread_count = cache.get_unread_count(&author_pubkey.to_string()).unwrap();
    assert_eq!(unread_count, 1);

    // Step 7: Mark notification as read
    cache.mark_notification_read(&notification_id).unwrap();

    // Step 8: Verify notification is marked as read
    let notifications_after = cache
        .get_notifications(&author_pubkey.to_string(), 10)
        .unwrap();
    assert_eq!(notifications_after.len(), 1);
    assert!(notifications_after[0].read);

    // Step 9: Verify unread count is now 0
    let unread_count_after = cache.get_unread_count(&author_pubkey.to_string()).unwrap();
    assert_eq!(unread_count_after, 0);
}

/// Test that notifications are created for PR status changes
#[test]
fn test_notification_on_pr_status_change() {
    let author_mnemonic = Mnemonic::parse("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about").unwrap();
    let author_identity = UnifiedIdentity::from_mnemonic(author_mnemonic).unwrap();
    let author_pubkey = author_identity.nostr_public_key();

    let maintainer_mnemonic =
        Mnemonic::parse("zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong").unwrap();
    let maintainer_identity = UnifiedIdentity::from_mnemonic(maintainer_mnemonic).unwrap();

    let (cache, _temp_dir) = create_test_cache();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Create PR
    let pr_template = EventTemplate {
        kind: 1618,
        content: "Feature implementation".to_string(),
        tags: vec![
            vec!["a".to_string(), "30617:test-owner:test-repo".to_string()],
            vec!["subject".to_string(), "Add feature X".to_string()],
        ],
        created_at: now,
    };

    let pr_event = author_identity.sign_event(pr_template).unwrap();
    let pr_id = pr_event.id.clone();

    cache.insert_event(&pr_event).unwrap();

    // Maintainer merges the PR (kind:1631 - STATUS_APPLIED)
    let merge_template = EventTemplate {
        kind: 1631,
        content: "".to_string(),
        tags: vec![
            vec!["e".to_string(), pr_id.clone()],
            vec!["p".to_string(), author_pubkey.to_string()],
        ],
        created_at: now + 20,
    };

    let merge_event = maintainer_identity.sign_event(merge_template).unwrap();
    cache.insert_event(&merge_event).unwrap();

    // Create notification for the merge
    cache
        .create_notification(
            &author_pubkey.to_string(),
            &merge_event.id,
            1631,
            "pr_status",
            "Your PR was merged",
            None,
        )
        .unwrap();

    // Verify notification
    let notifications = cache
        .get_notifications(&author_pubkey.to_string(), 10)
        .unwrap();
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].notification_type, "pr_status");
    assert_eq!(notifications[0].title, "Your PR was merged");
}

/// Test that notifications are created for issue claims
#[test]
fn test_notification_on_issue_claim() {
    let issue_author_mnemonic = Mnemonic::parse("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about").unwrap();
    let issue_author_identity = UnifiedIdentity::from_mnemonic(issue_author_mnemonic).unwrap();
    let issue_author_pubkey = issue_author_identity.nostr_public_key();

    let agent_mnemonic =
        Mnemonic::parse("zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong").unwrap();
    let agent_identity = UnifiedIdentity::from_mnemonic(agent_mnemonic).unwrap();

    let (cache, _temp_dir) = create_test_cache();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Create issue (kind:1621)
    let issue_template = EventTemplate {
        kind: 1621,
        content: "Bug description".to_string(),
        tags: vec![
            vec!["a".to_string(), "30617:test-owner:test-repo".to_string()],
            vec!["subject".to_string(), "Fix broken feature".to_string()],
        ],
        created_at: now,
    };

    let issue_event = issue_author_identity.sign_event(issue_template).unwrap();
    let issue_id = issue_event.id.clone();

    cache.insert_event(&issue_event).unwrap();

    // Agent claims the issue (kind:1634)
    let claim_template = EventTemplate {
        kind: 1634,
        content: "I'll work on this. ETA: 2 hours".to_string(),
        tags: vec![
            vec!["e".to_string(), issue_id.clone()],
            vec!["p".to_string(), issue_author_pubkey.to_string()],
            vec!["estimate".to_string(), "7200".to_string()],
        ],
        created_at: now + 5,
    };

    let claim_event = agent_identity.sign_event(claim_template).unwrap();
    cache.insert_event(&claim_event).unwrap();

    // Create notification for the claim
    cache
        .create_notification(
            &issue_author_pubkey.to_string(),
            &claim_event.id,
            1634,
            "issue_claim",
            "Someone claimed your issue",
            None,
        )
        .unwrap();

    // Verify notification
    let notifications = cache
        .get_notifications(&issue_author_pubkey.to_string(), 10)
        .unwrap();
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].notification_type, "issue_claim");
    assert_eq!(notifications[0].title, "Someone claimed your issue");
}

/// Test mark all notifications as read
#[test]
fn test_mark_all_notifications_read() {
    let user_mnemonic = Mnemonic::parse("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about").unwrap();
    let user_identity = UnifiedIdentity::from_mnemonic(user_mnemonic).unwrap();
    let user_pubkey = user_identity.nostr_public_key();

    let (cache, _temp_dir) = create_test_cache();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Create multiple notifications - need to create events first for FK constraint
    for i in 0..3 {
        // Create a dummy event
        let event_template = EventTemplate {
            kind: 1,
            content: format!("Review comment {}", i),
            tags: vec![vec!["p".to_string(), user_pubkey.to_string()]],
            created_at: now + i,
        };
        let event = user_identity.sign_event(event_template).unwrap();
        let event_id = event.id.clone();

        // Insert event first (for FK constraint)
        cache.insert_event(&event).unwrap();

        // Create notification
        cache
            .create_notification(
                &user_pubkey.to_string(),
                &event_id,
                1,
                "pr_review",
                &format!("Review {}", i),
                None,
            )
            .unwrap();
    }

    // Verify 3 unread notifications
    let unread_count = cache.get_unread_count(&user_pubkey.to_string()).unwrap();
    assert_eq!(unread_count, 3);

    // Mark all as read
    let marked = cache
        .mark_all_notifications_read(&user_pubkey.to_string())
        .unwrap();
    assert_eq!(marked, 3);

    // Verify all are read
    let unread_count_after = cache.get_unread_count(&user_pubkey.to_string()).unwrap();
    assert_eq!(unread_count_after, 0);
}
