//! End-to-end test for GitAfter issue claim and comment flow
//!
//! Tests the complete workflow from issue creation through PR merge and bounty claim.

mod helpers;

use anyhow::Result;
use helpers::test_app::TestApp;
use nostr::EventTemplate;

#[tokio::test]
async fn test_complete_issue_workflow() -> Result<()> {
    let app = TestApp::new().await?;

    // Step 1: Create repository
    let repo = app
        .create_repository(
            "openagents",
            "OpenAgents",
            "Desktop foundation for sovereign AI agents",
        )
        .await?;

    assert_eq!(repo.kind, 30617);
    assert_eq!(repo.pubkey, app.pubkey());

    let repo_events = app.get_events_by_kind(30617).await;
    assert_eq!(repo_events.len(), 1);
    assert_eq!(repo_events[0].id, repo.id);

    // Step 2: Create issue on repository
    let issue = app
        .create_issue(
            "openagents",
            "Add NIP-77 support",
            "Implement Negentropy protocol for efficient sync",
        )
        .await?;

    assert_eq!(issue.kind, 1621);
    assert_eq!(
        issue.content,
        "Implement Negentropy protocol for efficient sync"
    );

    // Verify issue has correct repo reference
    let repo_tag = issue
        .tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("a"))
        .expect("issue should have 'a' tag");
    assert_eq!(
        repo_tag.get(1).unwrap(),
        &format!("30617:{}:openagents", app.pubkey())
    );

    let issue_events = app.get_events_by_kind(1621).await;
    assert_eq!(issue_events.len(), 1);

    // Step 3: Create bounty offer (kind:1636)
    let bounty_template = EventTemplate {
        kind: 1636, // BOUNTY_OFFER
        tags: vec![
            vec![
                "e".to_string(),
                issue.id.clone(),
                "".to_string(),
                "root".to_string(),
            ],
            vec![
                "a".to_string(),
                format!("30617:{}:openagents", app.pubkey()),
            ],
            vec!["amount".to_string(), "50000".to_string()], // 50k sats
            vec!["expiry".to_string(), "1735689600".to_string()], // example timestamp
            vec!["conditions".to_string(), "must include tests".to_string()],
        ],
        content: String::new(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let bounty = app.publish_event(bounty_template).await?;

    assert_eq!(bounty.kind, 1636);

    // Verify bounty references issue
    let bounty_issue_ref = bounty
        .tags
        .iter()
        .find(|t| {
            t.first().map(|s| s.as_str()) == Some("e")
                && t.get(3).map(|s| s.as_str()) == Some("root")
        })
        .expect("bounty should reference issue");
    assert_eq!(bounty_issue_ref.get(1).unwrap(), &issue.id);

    // Verify bounty has amount
    let amount_tag = bounty
        .tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("amount"))
        .expect("bounty should have amount");
    assert_eq!(amount_tag.get(1).unwrap(), "50000");

    let bounty_events = app.get_events_by_kind(1636).await;
    assert_eq!(bounty_events.len(), 1);

    // Step 4: Agent claims issue (kind:1634)
    let claim = app.claim_issue(&issue.id).await?;

    assert_eq!(claim.kind, 1634);
    assert_eq!(claim.content, "Claiming this issue");

    // Verify claim references issue
    let issue_ref = claim
        .tags
        .iter()
        .find(|t| {
            t.first().map(|s| s.as_str()) == Some("e")
                && t.get(3).map(|s| s.as_str()) == Some("root")
        })
        .expect("claim should have root 'e' tag");
    assert_eq!(issue_ref.get(1).unwrap(), &issue.id);

    let claim_events = app.get_events_by_kind(1634).await;
    assert_eq!(claim_events.len(), 1);

    // Step 5: Agent posts progress comment (NIP-22)
    let progress_comment = app
        .comment_on_issue(
            &issue.id,
            "Started implementation. Added core types and varint encoding.",
        )
        .await?;

    assert_eq!(progress_comment.kind, 1);
    assert!(progress_comment.content.contains("Started implementation"));

    // Verify comment references issue
    let comment_ref = progress_comment
        .tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("e"))
        .expect("comment should reference issue");
    assert_eq!(comment_ref.get(1).unwrap(), &issue.id);

    // Step 6: Agent submits PR (kind:1618) with trajectory link
    let trajectory_session_id = "session-abc123";
    let trajectory_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    let pr_template = EventTemplate {
        kind: 1618, // PULL_REQUEST
        tags: vec![
            vec!["a".to_string(), format!("30617:{}:openagents", app.pubkey())],
            vec!["subject".to_string(), "Add NIP-77 Negentropy protocol support".to_string()],
            vec!["e".to_string(), issue.id.clone(), "".to_string(), "mention".to_string()],
            vec!["c".to_string(), "abc123def456".to_string()], // commit ID
            vec!["clone".to_string(), "https://github.com/test/openagents".to_string()],
            vec!["trajectory".to_string(), trajectory_session_id.to_string(), "wss://relay.nostr.bg".to_string()],
            vec!["trajectory_hash".to_string(), trajectory_hash.to_string()],
        ],
        content: "## Summary\n\nImplemented NIP-77 Negentropy protocol.\n\n## Changes\n- Added core types\n- Implemented varint encoding\n- Added tests".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let pr = app.publish_event(pr_template).await?;

    assert_eq!(pr.kind, 1618);
    assert!(pr.content.contains("NIP-77 Negentropy"));

    // Verify PR has trajectory link
    let trajectory_tag = pr
        .tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("trajectory"))
        .expect("PR should have trajectory tag");
    assert_eq!(trajectory_tag.get(1).unwrap(), trajectory_session_id);

    // Verify PR has trajectory hash
    let hash_tag = pr
        .tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("trajectory_hash"))
        .expect("PR should have trajectory_hash tag");
    assert_eq!(hash_tag.get(1).unwrap(), trajectory_hash);

    // Verify PR references issue
    let pr_issue_ref = pr
        .tags
        .iter()
        .find(|t| {
            t.first().map(|s| s.as_str()) == Some("e")
                && t.get(3).map(|s| s.as_str()) == Some("mention")
        })
        .expect("PR should mention issue");
    assert_eq!(pr_issue_ref.get(1).unwrap(), &issue.id);

    let pr_events = app.get_events_by_kind(1618).await;
    assert_eq!(pr_events.len(), 1);

    // Step 7: Review comment posted
    let review_comment = app
        .comment_on_issue(&pr.id, "LGTM! Great implementation. Tests pass.")
        .await?;

    assert_eq!(review_comment.kind, 1);
    assert!(review_comment.content.contains("LGTM"));

    // Verify review references PR
    let review_ref = review_comment
        .tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("e"))
        .expect("review should reference PR");
    assert_eq!(review_ref.get(1).unwrap(), &pr.id);

    // Step 8: PR status updated to merged (kind:1631)
    let status_template = EventTemplate {
        kind: 1631, // STATUS_APPLIED
        tags: vec![
            vec![
                "e".to_string(),
                pr.id.clone(),
                "".to_string(),
                "root".to_string(),
            ],
            vec![
                "a".to_string(),
                format!("30617:{}:openagents", app.pubkey()),
            ],
        ],
        content: "Merged to main".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let status = app.publish_event(status_template).await?;

    assert_eq!(status.kind, 1631);
    assert_eq!(status.content, "Merged to main");

    // Verify status references PR
    let status_ref = status
        .tags
        .iter()
        .find(|t| {
            t.first().map(|s| s.as_str()) == Some("e")
                && t.get(3).map(|s| s.as_str()) == Some("root")
        })
        .expect("status should reference PR");
    assert_eq!(status_ref.get(1).unwrap(), &pr.id);

    let status_events = app.get_events_by_kind(1631).await;
    assert_eq!(status_events.len(), 1);

    // Step 9: Bounty claim triggered (kind:1637)
    let bounty_claim_template = EventTemplate {
        kind: 1637, // BOUNTY_CLAIM
        tags: vec![
            vec![
                "e".to_string(),
                issue.id.clone(),
                "".to_string(),
                "mention".to_string(),
            ],
            vec![
                "e".to_string(),
                pr.id.clone(),
                "".to_string(),
                "mention".to_string(),
            ],
            vec![
                "a".to_string(),
                format!("30617:{}:openagents", app.pubkey()),
            ],
            vec![
                "trajectory".to_string(),
                trajectory_session_id.to_string(),
                "wss://relay.nostr.bg".to_string(),
            ],
            vec!["trajectory_hash".to_string(), trajectory_hash.to_string()],
            vec!["lud16".to_string(), "agent@getalby.com".to_string()],
        ],
        content: String::new(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let bounty_claim = app.publish_event(bounty_claim_template).await?;

    assert_eq!(bounty_claim.kind, 1637);

    // Verify bounty claim references both issue and PR
    let claim_issue_ref = bounty_claim
        .tags
        .iter()
        .filter(|t| t.first().map(|s| s.as_str()) == Some("e"))
        .find(|t| t.get(1).unwrap() == &issue.id)
        .expect("bounty claim should reference issue");
    assert_eq!(claim_issue_ref.get(1).unwrap(), &issue.id);

    let claim_pr_ref = bounty_claim
        .tags
        .iter()
        .filter(|t| t.first().map(|s| s.as_str()) == Some("e"))
        .find(|t| t.get(1).unwrap() == &pr.id)
        .expect("bounty claim should reference PR");
    assert_eq!(claim_pr_ref.get(1).unwrap(), &pr.id);

    // Verify bounty claim has trajectory proof
    let claim_trajectory = bounty_claim
        .tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("trajectory"))
        .expect("bounty claim should have trajectory");
    assert_eq!(claim_trajectory.get(1).unwrap(), trajectory_session_id);

    // Verify Lightning address
    let lud16_tag = bounty_claim
        .tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("lud16"))
        .expect("bounty claim should have lud16");
    assert_eq!(lud16_tag.get(1).unwrap(), "agent@getalby.com");

    let bounty_claim_events = app.get_events_by_kind(1637).await;
    assert_eq!(bounty_claim_events.len(), 1);

    // Verify complete event sequence
    let all_events = app.get_all_events().await;
    assert_eq!(all_events.len(), 9); // repo, issue, bounty offer, claim, progress comment, pr, review comment, status, bounty_claim

    // Verify all events by author
    let author_events = app.get_events_by_author(&app.pubkey()).await;
    assert_eq!(author_events.len(), 9);

    app.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn test_trajectory_hash_validation() -> Result<()> {
    let app = TestApp::new().await?;

    // Create minimal setup
    let _repo = app
        .create_repository("test-repo", "Test", "Test repo")
        .await?;
    let issue = app.create_issue("test-repo", "Test issue", "Body").await?;

    // Create PR with valid trajectory hash
    let valid_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    let pr_template = EventTemplate {
        kind: 1618,
        tags: vec![
            vec!["a".to_string(), format!("30617:{}:test-repo", app.pubkey())],
            vec!["e".to_string(), issue.id.clone()],
            vec!["trajectory_hash".to_string(), valid_hash.to_string()],
        ],
        content: "Test PR".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let pr = app.publish_event(pr_template).await?;

    // Verify hash is exactly 64 chars (sha256 hex)
    let hash_tag = pr
        .tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("trajectory_hash"))
        .expect("should have trajectory_hash");

    assert_eq!(hash_tag.get(1).unwrap().len(), 64);
    assert!(
        hash_tag
            .get(1)
            .unwrap()
            .chars()
            .all(|c| c.is_ascii_hexdigit())
    );

    app.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn test_issue_claim_with_estimate() -> Result<()> {
    let app = TestApp::new().await?;

    let _repo = app.create_repository("test", "Test", "Desc").await?;
    let issue = app.create_issue("test", "Issue", "Body").await?;

    // Claim with estimate
    let claim_template = EventTemplate {
        kind: 1634,
        tags: vec![
            vec![
                "e".to_string(),
                issue.id.clone(),
                "".to_string(),
                "root".to_string(),
            ],
            vec!["estimate".to_string(), "3600".to_string()], // 1 hour
        ],
        content: "I'll complete this in 1 hour".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let claim = app.publish_event(claim_template).await?;

    // Verify estimate tag
    let estimate_tag = claim
        .tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("estimate"))
        .expect("claim should have estimate");
    assert_eq!(estimate_tag.get(1).unwrap(), "3600");

    app.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn test_stacked_pr_dependencies() -> Result<()> {
    let app = TestApp::new().await?;

    let _repo = app.create_repository("test", "Test", "Desc").await?;
    let issue = app
        .create_issue("test", "Multi-layer feature", "Body")
        .await?;

    // Create layer 1 (base)
    let pr1_template = EventTemplate {
        kind: 1618,
        tags: vec![
            vec!["a".to_string(), format!("30617:{}:test", app.pubkey())],
            vec!["e".to_string(), issue.id.clone()],
            vec!["subject".to_string(), "Layer 1: Foundation".to_string()],
            vec!["stack".to_string(), "stack-uuid-123".to_string()],
            vec!["layer".to_string(), "1".to_string(), "3".to_string()],
        ],
        content: "Foundation layer".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let pr1 = app.publish_event(pr1_template).await?;

    // Create layer 2 (depends on layer 1)
    let pr2_template = EventTemplate {
        kind: 1618,
        tags: vec![
            vec!["a".to_string(), format!("30617:{}:test", app.pubkey())],
            vec!["e".to_string(), issue.id.clone()],
            vec![
                "subject".to_string(),
                "Layer 2: Build on foundation".to_string(),
            ],
            vec!["stack".to_string(), "stack-uuid-123".to_string()],
            vec!["layer".to_string(), "2".to_string(), "3".to_string()],
            vec![
                "depends_on".to_string(),
                pr1.id.clone(),
                "wss://relay.nostr.bg".to_string(),
            ],
        ],
        content: "Second layer".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let pr2 = app.publish_event(pr2_template).await?;

    // Verify stack tags
    let pr2_stack = pr2
        .tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("stack"))
        .expect("should have stack tag");
    assert_eq!(pr2_stack.get(1).unwrap(), "stack-uuid-123");

    // Verify layer tag
    let pr2_layer = pr2
        .tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("layer"))
        .expect("should have layer tag");
    assert_eq!(pr2_layer.get(1).unwrap(), "2");
    assert_eq!(pr2_layer.get(2).unwrap(), "3");

    // Verify depends_on tag
    let depends_on = pr2
        .tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("depends_on"))
        .expect("should have depends_on tag");
    assert_eq!(depends_on.get(1).unwrap(), &pr1.id);

    app.shutdown().await;
    Ok(())
}
