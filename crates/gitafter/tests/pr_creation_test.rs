//! Integration tests for GitAfter PR creation with trajectory linking

mod helpers;

use anyhow::Result;
use helpers::test_app::TestApp;
use nostr::EventTemplate;

#[tokio::test]
async fn test_create_pull_request() -> Result<()> {
    let app = TestApp::new().await?;

    // Create repository first
    let _repo = app
        .create_repository("test-repo", "Test Repo", "Test repository")
        .await?;

    // Create pull request event (kind:1618)
    let pr_template = EventTemplate {
        kind: 1618, // PULL_REQUEST
        tags: vec![
            vec!["a".to_string(), format!("30617:{}:test-repo", app.pubkey())],
            vec!["subject".to_string(), "Add new feature".to_string()],
            vec!["c".to_string(), "abc123def456".to_string()], // commit ID
            vec![
                "clone".to_string(),
                "https://github.com/test/repo.git".to_string(),
            ],
        ],
        content: "This PR adds a new feature to the codebase".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let pr = app.publish_event(pr_template).await?;

    // Verify PR structure
    assert_eq!(pr.kind, 1618);

    // Verify tags
    let subject_tag = pr
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "subject").unwrap_or(false))
        .expect("subject tag should exist");
    assert_eq!(subject_tag.get(1), Some(&"Add new feature".to_string()));

    let commit_tag = pr
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "c").unwrap_or(false))
        .expect("commit tag should exist");
    assert_eq!(commit_tag.get(1), Some(&"abc123def456".to_string()));

    let clone_tag = pr
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "clone").unwrap_or(false))
        .expect("clone tag should exist");
    assert_eq!(
        clone_tag.get(1),
        Some(&"https://github.com/test/repo.git".to_string())
    );

    // Verify PR was stored in relay
    let stored_prs = app.relay.get_events_by_kind(1618).await;
    assert_eq!(stored_prs.len(), 1);
    assert_eq!(stored_prs[0].id, pr.id);

    Ok(())
}

#[tokio::test]
async fn test_create_pr_with_trajectory_proof() -> Result<()> {
    let app = TestApp::new().await?;

    // Create repository
    let _repo = app
        .create_repository("test-repo", "Test Repo", "Test repository")
        .await?;

    // Create PR with trajectory proof
    let trajectory_session_id = "session-uuid-12345";
    let trajectory_hash = "a1b2c3d4e5f6";

    let pr_template = EventTemplate {
        kind: 1618,
        tags: vec![
            vec!["a".to_string(), format!("30617:{}:test-repo", app.pubkey())],
            vec!["subject".to_string(), "Agent-authored feature".to_string()],
            vec!["c".to_string(), "commit-hash-789".to_string()],
            vec![
                "clone".to_string(),
                "https://github.com/test/repo.git".to_string(),
            ],
            vec!["trajectory".to_string(), trajectory_session_id.to_string()],
            vec!["trajectory_hash".to_string(), trajectory_hash.to_string()],
        ],
        content: "This PR was created by an agent with full trajectory proof".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let pr = app.publish_event(pr_template).await?;

    // Verify trajectory tags
    let trajectory_tag = pr
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "trajectory").unwrap_or(false))
        .expect("trajectory tag should exist");
    assert_eq!(
        trajectory_tag.get(1),
        Some(&trajectory_session_id.to_string())
    );

    let hash_tag = pr
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "trajectory_hash").unwrap_or(false))
        .expect("trajectory_hash tag should exist");
    assert_eq!(hash_tag.get(1), Some(&trajectory_hash.to_string()));

    Ok(())
}

#[tokio::test]
async fn test_pr_status_transitions() -> Result<()> {
    let app = TestApp::new().await?;

    // Create repository
    let _repo = app
        .create_repository("test-repo", "Test Repo", "Test repository")
        .await?;

    // Create PR
    let pr_template = EventTemplate {
        kind: 1618,
        tags: vec![
            vec!["a".to_string(), format!("30617:{}:test-repo", app.pubkey())],
            vec!["subject".to_string(), "Feature PR".to_string()],
            vec!["c".to_string(), "commit123".to_string()],
        ],
        content: "Feature implementation".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let pr = app.publish_event(pr_template).await?;

    // Set status to Open (kind:1630)
    let open_status_template = EventTemplate {
        kind: 1630, // STATUS_OPEN
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

    let open_status = app.publish_event(open_status_template).await?;
    assert_eq!(open_status.kind, 1630);

    // Set status to Merged (kind:1631)
    let merged_status_template = EventTemplate {
        kind: 1631, // STATUS_MERGED
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

    let merged_status = app.publish_event(merged_status_template).await?;
    assert_eq!(merged_status.kind, 1631);

    // Verify status events stored
    let open_events = app.relay.get_events_by_kind(1630).await;
    assert_eq!(open_events.len(), 1);

    let merged_events = app.relay.get_events_by_kind(1631).await;
    assert_eq!(merged_events.len(), 1);

    Ok(())
}

#[tokio::test]
async fn test_pr_with_stacked_diff_tags() -> Result<()> {
    let app = TestApp::new().await?;

    // Create repository
    let _repo = app
        .create_repository("test-repo", "Test Repo", "Test repository")
        .await?;

    let stack_id = "stack-uuid-abc";

    // Create base layer PR (layer 1 of 3)
    let pr1_template = EventTemplate {
        kind: 1618,
        tags: vec![
            vec!["a".to_string(), format!("30617:{}:test-repo", app.pubkey())],
            vec!["subject".to_string(), "Layer 1: Base changes".to_string()],
            vec!["c".to_string(), "commit1".to_string()],
            vec!["stack".to_string(), stack_id.to_string()],
            vec!["layer".to_string(), "1".to_string(), "3".to_string()],
        ],
        content: "Base layer changes".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let pr1 = app.publish_event(pr1_template).await?;

    // Create dependent layer PR (layer 2 of 3)
    let pr2_template = EventTemplate {
        kind: 1618,
        tags: vec![
            vec!["a".to_string(), format!("30617:{}:test-repo", app.pubkey())],
            vec!["subject".to_string(), "Layer 2: Build on base".to_string()],
            vec!["c".to_string(), "commit2".to_string()],
            vec!["stack".to_string(), stack_id.to_string()],
            vec!["layer".to_string(), "2".to_string(), "3".to_string()],
            vec!["depends_on".to_string(), pr1.id.clone()],
        ],
        content: "Second layer changes".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let pr2 = app.publish_event(pr2_template).await?;

    // Verify stack tags
    let stack_tag = pr2
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "stack").unwrap_or(false))
        .expect("stack tag should exist");
    assert_eq!(stack_tag.get(1), Some(&stack_id.to_string()));

    let layer_tag = pr2
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "layer").unwrap_or(false))
        .expect("layer tag should exist");
    assert_eq!(layer_tag.get(1), Some(&"2".to_string()));
    assert_eq!(layer_tag.get(2), Some(&"3".to_string()));

    let depends_tag = pr2
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "depends_on").unwrap_or(false))
        .expect("depends_on tag should exist");
    assert_eq!(depends_tag.get(1), Some(&pr1.id));

    // Verify both PRs stored
    let all_prs = app.relay.get_events_by_kind(1618).await;
    assert_eq!(all_prs.len(), 2);

    Ok(())
}

#[tokio::test]
async fn test_pr_update_event() -> Result<()> {
    let app = TestApp::new().await?;

    // Create repository
    let _repo = app
        .create_repository("test-repo", "Test Repo", "Test repository")
        .await?;

    // Create initial PR
    let pr_template = EventTemplate {
        kind: 1618,
        tags: vec![
            vec!["a".to_string(), format!("30617:{}:test-repo", app.pubkey())],
            vec!["subject".to_string(), "Initial PR".to_string()],
            vec!["c".to_string(), "commit-v1".to_string()],
        ],
        content: "Initial version".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let pr = app.publish_event(pr_template).await?;

    // Create PR update event (kind:1619)
    let update_template = EventTemplate {
        kind: 1619, // PR_UPDATE
        tags: vec![
            vec![
                "e".to_string(),
                pr.id.clone(),
                "".to_string(),
                "root".to_string(),
            ],
            vec!["c".to_string(), "commit-v2".to_string()], // Updated commit
        ],
        content: "Updated commit after rebase".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let update = app.publish_event(update_template).await?;

    // Verify update event
    assert_eq!(update.kind, 1619);

    // Verify update references original PR
    let e_tag = update
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "e").unwrap_or(false))
        .expect("e tag should exist");
    assert_eq!(e_tag.get(1), Some(&pr.id));

    // Verify updated commit tag
    let commit_tag = update
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "c").unwrap_or(false))
        .expect("c tag should exist");
    assert_eq!(commit_tag.get(1), Some(&"commit-v2".to_string()));

    // Verify update stored
    let updates = app.relay.get_events_by_kind(1619).await;
    assert_eq!(updates.len(), 1);

    Ok(())
}
