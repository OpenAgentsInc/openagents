//! Integration tests for GitAfter bounty attachment and claim workflow

mod helpers;

use anyhow::Result;
use helpers::test_app::TestApp;
use nostr::EventTemplate;

#[tokio::test]
async fn test_attach_bounty_to_issue() -> Result<()> {
    let app = TestApp::new().await?;

    // Create repository
    let _repo = app
        .create_repository("test-repo", "Test Repo", "Test repository")
        .await?;

    // Create issue
    let issue = app
        .create_issue("test-repo", "Fix authentication bug", "Auth is broken")
        .await?;

    // Attach bounty to issue (kind:1636)
    let bounty_template = EventTemplate {
        kind: 1636, // BOUNTY_OFFER
        tags: vec![
            vec![
                "e".to_string(),
                issue.id.clone(),
                "".to_string(),
                "root".to_string(),
            ],
            vec!["a".to_string(), format!("30617:{}:test-repo", app.pubkey())],
            vec!["amount".to_string(), "50000".to_string()], // 50k sats
            vec![
                "expiry".to_string(),
                "1735689600".to_string(), // Unix timestamp
            ],
            vec!["conditions".to_string(), "must include tests".to_string()],
            vec!["conditions".to_string(), "must pass CI".to_string()],
        ],
        content: String::new(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let bounty = app.publish_event(bounty_template).await?;

    // Verify bounty structure
    assert_eq!(bounty.kind, 1636);

    // Verify tags
    let amount_tag = bounty
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "amount").unwrap_or(false))
        .expect("amount tag should exist");
    assert_eq!(amount_tag.get(1), Some(&"50000".to_string()));

    let expiry_tag = bounty
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "expiry").unwrap_or(false))
        .expect("expiry tag should exist");
    assert_eq!(expiry_tag.get(1), Some(&"1735689600".to_string()));

    // Verify conditions tags (there should be 2)
    let conditions: Vec<_> = bounty
        .tags
        .iter()
        .filter(|t| t.first().map(|s| s == "conditions").unwrap_or(false))
        .collect();
    assert_eq!(conditions.len(), 2);

    // Verify bounty stored in relay
    let bounties = app.relay.get_events_by_kind(1636).await;
    assert_eq!(bounties.len(), 1);

    Ok(())
}

#[tokio::test]
async fn test_claim_bounty_after_pr_merge() -> Result<()> {
    let app = TestApp::new().await?;

    // Create repository
    let _repo = app
        .create_repository("test-repo", "Test Repo", "Test repository")
        .await?;

    // Create issue
    let issue = app
        .create_issue("test-repo", "Add feature X", "Need feature X")
        .await?;

    // Attach bounty
    let bounty_template = EventTemplate {
        kind: 1636,
        tags: vec![
            vec![
                "e".to_string(),
                issue.id.clone(),
                "".to_string(),
                "root".to_string(),
            ],
            vec!["a".to_string(), format!("30617:{}:test-repo", app.pubkey())],
            vec!["amount".to_string(), "100000".to_string()],
        ],
        content: String::new(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let bounty = app.publish_event(bounty_template).await?;

    // Claim issue
    let claim = app.claim_issue(&issue.id).await?;
    assert_eq!(claim.kind, 1634);

    // Create PR that fixes the issue
    let pr_template = EventTemplate {
        kind: 1618,
        tags: vec![
            vec!["a".to_string(), format!("30617:{}:test-repo", app.pubkey())],
            vec![
                "subject".to_string(),
                "Fix issue: Add feature X".to_string(),
            ],
            vec!["c".to_string(), "commit-fix-123".to_string()],
            vec![
                "e".to_string(),
                issue.id.clone(),
                "".to_string(),
                "mention".to_string(),
            ], // References issue
        ],
        content: "Implements feature X as requested".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let pr = app.publish_event(pr_template).await?;

    // Merge PR (kind:1631 STATUS_MERGED)
    let merge_template = EventTemplate {
        kind: 1631,
        tags: vec![vec![
            "e".to_string(),
            pr.id.clone(),
            "".to_string(),
            "root".to_string(),
        ]],
        content: String::new(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let _merge = app.publish_event(merge_template).await?;

    // Claim bounty (kind:1637)
    let bounty_claim_template = EventTemplate {
        kind: 1637, // BOUNTY_CLAIM
        tags: vec![
            vec![
                "e".to_string(),
                bounty.id.clone(),
                "".to_string(),
                "root".to_string(),
            ],
            vec![
                "e".to_string(),
                pr.id.clone(),
                "".to_string(),
                "mention".to_string(),
            ],
            vec!["a".to_string(), format!("30617:{}:test-repo", app.pubkey())],
            vec!["lud16".to_string(), "agent@getalby.com".to_string()],
        ],
        content: String::new(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let bounty_claim = app.publish_event(bounty_claim_template).await?;

    // Verify bounty claim
    assert_eq!(bounty_claim.kind, 1637);

    // Verify references bounty and PR
    let bounty_ref = bounty_claim
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "e").unwrap_or(false) && t.get(1) == Some(&bounty.id))
        .expect("should reference bounty");
    assert_eq!(bounty_ref.get(3), Some(&"root".to_string()));

    let pr_ref = bounty_claim
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "e").unwrap_or(false) && t.get(1) == Some(&pr.id))
        .expect("should reference PR");
    assert_eq!(pr_ref.get(3), Some(&"mention".to_string()));

    // Verify Lightning address
    let lud16_tag = bounty_claim
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "lud16").unwrap_or(false))
        .expect("lud16 tag should exist");
    assert_eq!(lud16_tag.get(1), Some(&"agent@getalby.com".to_string()));

    // Verify bounty claim stored
    let claims = app.relay.get_events_by_kind(1637).await;
    assert_eq!(claims.len(), 1);

    Ok(())
}

#[tokio::test]
async fn test_bounty_claim_with_trajectory_proof() -> Result<()> {
    let app = TestApp::new().await?;

    // Create repository
    let _repo = app
        .create_repository("test-repo", "Test Repo", "Test repository")
        .await?;

    // Create issue with bounty
    let issue = app
        .create_issue("test-repo", "Agent task", "Task for agent")
        .await?;

    let bounty_template = EventTemplate {
        kind: 1636,
        tags: vec![
            vec![
                "e".to_string(),
                issue.id.clone(),
                "".to_string(),
                "root".to_string(),
            ],
            vec!["a".to_string(), format!("30617:{}:test-repo", app.pubkey())],
            vec!["amount".to_string(), "25000".to_string()],
        ],
        content: String::new(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let bounty = app.publish_event(bounty_template).await?;

    // Create PR with trajectory proof
    let trajectory_session = "session-abc-123";
    let trajectory_hash = "hash-xyz-789";

    let pr_template = EventTemplate {
        kind: 1618,
        tags: vec![
            vec!["a".to_string(), format!("30617:{}:test-repo", app.pubkey())],
            vec!["subject".to_string(), "Agent fix".to_string()],
            vec!["c".to_string(), "commit-agent-123".to_string()],
            vec!["trajectory".to_string(), trajectory_session.to_string()],
            vec!["trajectory_hash".to_string(), trajectory_hash.to_string()],
        ],
        content: "Agent-authored fix".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let pr = app.publish_event(pr_template).await?;

    // Merge PR
    let merge_template = EventTemplate {
        kind: 1631,
        tags: vec![vec![
            "e".to_string(),
            pr.id.clone(),
            "".to_string(),
            "root".to_string(),
        ]],
        content: String::new(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let _merge = app.publish_event(merge_template).await?;

    // Claim bounty with trajectory proof
    let bounty_claim_template = EventTemplate {
        kind: 1637,
        tags: vec![
            vec![
                "e".to_string(),
                bounty.id.clone(),
                "".to_string(),
                "root".to_string(),
            ],
            vec![
                "e".to_string(),
                pr.id.clone(),
                "".to_string(),
                "mention".to_string(),
            ],
            vec!["a".to_string(), format!("30617:{}:test-repo", app.pubkey())],
            vec!["trajectory".to_string(), trajectory_session.to_string()],
            vec!["trajectory_hash".to_string(), trajectory_hash.to_string()],
            vec![
                "lud16".to_string(),
                "sovereign-agent@getalby.com".to_string(),
            ],
        ],
        content: String::new(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let claim = app.publish_event(bounty_claim_template).await?;

    // Verify trajectory tags in claim
    let trajectory_tag = claim
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "trajectory").unwrap_or(false))
        .expect("trajectory tag should exist");
    assert_eq!(trajectory_tag.get(1), Some(&trajectory_session.to_string()));

    let hash_tag = claim
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "trajectory_hash").unwrap_or(false))
        .expect("trajectory_hash tag should exist");
    assert_eq!(hash_tag.get(1), Some(&trajectory_hash.to_string()));

    Ok(())
}

#[tokio::test]
async fn test_multiple_bounties_on_same_issue() -> Result<()> {
    let app = TestApp::new().await?;

    // Create repository and issue
    let _repo = app
        .create_repository("test-repo", "Test Repo", "Test repository")
        .await?;

    let issue = app
        .create_issue("test-repo", "Complex feature", "This is a complex feature")
        .await?;

    // First bounty offer
    let bounty1_template = EventTemplate {
        kind: 1636,
        tags: vec![
            vec![
                "e".to_string(),
                issue.id.clone(),
                "".to_string(),
                "root".to_string(),
            ],
            vec!["a".to_string(), format!("30617:{}:test-repo", app.pubkey())],
            vec!["amount".to_string(), "30000".to_string()],
        ],
        content: String::new(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let bounty1 = app.publish_event(bounty1_template).await?;

    // Second bounty offer (from same or different user)
    let bounty2_template = EventTemplate {
        kind: 1636,
        tags: vec![
            vec![
                "e".to_string(),
                issue.id.clone(),
                "".to_string(),
                "root".to_string(),
            ],
            vec!["a".to_string(), format!("30617:{}:test-repo", app.pubkey())],
            vec!["amount".to_string(), "20000".to_string()],
        ],
        content: String::new(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let bounty2 = app.publish_event(bounty2_template).await?;

    // Verify both bounties stored
    let bounties = app.relay.get_events_by_kind(1636).await;
    assert_eq!(bounties.len(), 2);

    // Verify both reference the same issue
    for bounty in &bounties {
        let issue_ref = bounty
            .tags
            .iter()
            .find(|t| t.first().map(|s| s == "e").unwrap_or(false) && t.get(1) == Some(&issue.id))
            .expect("should reference issue");
        assert_eq!(issue_ref.get(3), Some(&"root".to_string()));
    }

    Ok(())
}

#[tokio::test]
async fn test_issue_claim_workflow() -> Result<()> {
    let app = TestApp::new().await?;

    // Create repository and issue
    let _repo = app
        .create_repository("test-repo", "Test Repo", "Test repository")
        .await?;

    let issue = app
        .create_issue("test-repo", "Bug fix", "Fix this bug")
        .await?;

    // Claim issue (kind:1634)
    let claim_template = EventTemplate {
        kind: 1634, // ISSUE_CLAIM
        tags: vec![
            vec![
                "e".to_string(),
                issue.id.clone(),
                "".to_string(),
                "root".to_string(),
            ],
            vec![
                "estimate".to_string(),
                "7200".to_string(), // 2 hours in seconds
            ],
        ],
        content: "I'll work on this issue. Should be done in 2 hours.".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let claim = app.publish_event(claim_template).await?;

    // Verify claim structure
    assert_eq!(claim.kind, 1634);

    // Verify references issue
    let issue_ref = claim
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "e").unwrap_or(false) && t.get(1) == Some(&issue.id))
        .expect("should reference issue");
    assert_eq!(issue_ref.get(3), Some(&"root".to_string()));

    // Verify estimate tag
    let estimate_tag = claim
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "estimate").unwrap_or(false))
        .expect("estimate tag should exist");
    assert_eq!(estimate_tag.get(1), Some(&"7200".to_string()));

    // Verify claim stored
    let claims = app.relay.get_events_by_kind(1634).await;
    assert_eq!(claims.len(), 1);

    Ok(())
}
