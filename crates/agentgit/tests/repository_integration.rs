//! Integration tests for AgentGit repository announcement and viewing

mod helpers;

use anyhow::Result;
use helpers::test_app::TestApp;

#[tokio::test]
async fn test_create_repository_announcement() -> Result<()> {
    let app = TestApp::new().await?;

    // Create repository announcement
    let event = app
        .create_repository(
            "test-repo",
            "Test Repository",
            "A test repository for integration tests",
        )
        .await?;

    // Verify event structure
    assert_eq!(event.kind, 30617); // REPOSITORY_ANNOUNCEMENT

    // Verify tags
    let d_tag = event
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "d").unwrap_or(false))
        .expect("d tag should exist");
    assert_eq!(d_tag.get(1), Some(&"test-repo".to_string()));

    let name_tag = event
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "name").unwrap_or(false))
        .expect("name tag should exist");
    assert_eq!(name_tag.get(1), Some(&"Test Repository".to_string()));

    let desc_tag = event
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "description").unwrap_or(false))
        .expect("description tag should exist");
    assert_eq!(
        desc_tag.get(1),
        Some(&"A test repository for integration tests".to_string())
    );

    // Verify event was stored in relay
    let stored_events = app.relay.get_events_by_kind(30617).await;
    assert_eq!(stored_events.len(), 1);
    assert_eq!(stored_events[0].id, event.id);

    Ok(())
}

#[tokio::test]
async fn test_fetch_repository_list() -> Result<()> {
    let app = TestApp::new().await?;

    // Create multiple repositories
    let repo1 = app
        .create_repository("repo1", "Repository One", "First test repo")
        .await?;
    let repo2 = app
        .create_repository("repo2", "Repository Two", "Second test repo")
        .await?;
    let repo3 = app
        .create_repository("repo3", "Repository Three", "Third test repo")
        .await?;

    // Fetch all repository announcements
    let repos = app.relay.get_events_by_kind(30617).await;

    assert_eq!(repos.len(), 3);

    // Verify all repositories are present
    let repo_ids: Vec<String> = repos.iter().map(|e| e.id.clone()).collect();
    assert!(repo_ids.contains(&repo1.id));
    assert!(repo_ids.contains(&repo2.id));
    assert!(repo_ids.contains(&repo3.id));

    Ok(())
}

#[tokio::test]
async fn test_view_repository_details() -> Result<()> {
    let app = TestApp::new().await?;

    // Create repository with detailed information
    let event = app
        .create_repository(
            "detailed-repo",
            "Detailed Repository",
            "A repository with complete information",
        )
        .await?;

    // Fetch repository by ID
    let stored_event = app
        .relay
        .get_event(&event.id)
        .await
        .expect("event should exist");

    // Verify all details match
    assert_eq!(stored_event.id, event.id);
    assert_eq!(stored_event.kind, 30617);
    assert_eq!(stored_event.pubkey, app.pubkey());

    // Extract and verify tags
    let get_tag_value = |tag_name: &str| -> Option<String> {
        stored_event
            .tags
            .iter()
            .find(|t| t.first().map(|s| s == tag_name).unwrap_or(false))
            .and_then(|t| t.get(1))
            .cloned()
    };

    assert_eq!(get_tag_value("d"), Some("detailed-repo".to_string()));
    assert_eq!(
        get_tag_value("name"),
        Some("Detailed Repository".to_string())
    );
    assert_eq!(
        get_tag_value("description"),
        Some("A repository with complete information".to_string())
    );

    Ok(())
}

#[tokio::test]
async fn test_repository_event_signing() -> Result<()> {
    let app = TestApp::new().await?;

    // Create repository
    let event = app
        .create_repository("signed-repo", "Signed Repo", "Test signing")
        .await?;

    // Verify event signature is valid
    assert!(!event.sig.is_empty(), "signature should not be empty");
    assert_eq!(event.pubkey, app.pubkey(), "pubkey should match");

    // Event ID should be deterministic based on content
    assert!(!event.id.is_empty(), "event ID should not be empty");

    Ok(())
}

#[tokio::test]
async fn test_repository_update_via_replacement() -> Result<()> {
    let app = TestApp::new().await?;

    // Create initial repository
    let initial = app
        .create_repository("updateable-repo", "Initial Name", "Initial description")
        .await?;

    // Create updated version (same 'd' tag)
    let updated = app
        .create_repository(
            "updateable-repo", // Same identifier
            "Updated Name",
            "Updated description",
        )
        .await?;

    // Fetch all events with this kind
    let repos = app.relay.get_events_by_kind(30617).await;

    // Both events should be stored (relay doesn't auto-replace)
    assert_eq!(repos.len(), 2);

    // Verify both events have the same 'd' tag
    for event in &repos {
        let d_tag = event
            .tags
            .iter()
            .find(|t| t.first().map(|s| s == "d").unwrap_or(false))
            .expect("d tag should exist");
        assert_eq!(d_tag.get(1), Some(&"updateable-repo".to_string()));
    }

    // In a real implementation, the client would only show the newest event
    // (determined by created_at timestamp)
    assert!(updated.created_at >= initial.created_at);

    Ok(())
}
