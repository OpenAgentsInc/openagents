//! Tests for TestApp helper

mod helpers;

use helpers::TestApp;

#[tokio::test]
async fn test_full_issue_flow() {
    let app = TestApp::new().await.unwrap();

    // Create repository
    let repo = app
        .create_repository("test-repo", "Test Repository", "A test repo for agents")
        .await
        .unwrap();

    assert_eq!(repo.kind, 30617);

    // Create issue
    let issue = app
        .create_issue("test-repo", "Implement feature X", "We need feature X")
        .await
        .unwrap();

    assert_eq!(issue.kind, 1621);
    assert_eq!(issue.content, "We need feature X");

    // Claim issue
    let claim = app.claim_issue(&issue.id).await.unwrap();

    assert_eq!(claim.kind, 1634);

    // Post progress comment
    let comment = app
        .comment_on_issue(&issue.id, "Working on this now")
        .await
        .unwrap();

    assert_eq!(comment.kind, 1);

    // Verify all events stored
    let all_events = app.get_all_events().await;
    assert_eq!(all_events.len(), 4); // repo, issue, claim, comment

    // Verify events by kind
    let repos = app.get_events_by_kind(30617).await;
    assert_eq!(repos.len(), 1);

    let issues = app.get_events_by_kind(1621).await;
    assert_eq!(issues.len(), 1);

    let claims = app.get_events_by_kind(1634).await;
    assert_eq!(claims.len(), 1);

    let comments = app.get_events_by_kind(1).await;
    assert_eq!(comments.len(), 1);

    app.shutdown().await;
}

#[tokio::test]
async fn test_multiple_issues() {
    let app = TestApp::new().await.unwrap();

    let _repo = app.create_repository("repo", "Repo", "Desc").await.unwrap();

    // Create multiple issues
    let issue1 = app.create_issue("repo", "Issue 1", "First").await.unwrap();
    let issue2 = app.create_issue("repo", "Issue 2", "Second").await.unwrap();
    let issue3 = app.create_issue("repo", "Issue 3", "Third").await.unwrap();

    let issues = app.get_events_by_kind(1621).await;
    assert_eq!(issues.len(), 3);

    // Claim one issue
    let _claim = app.claim_issue(&issue2.id).await.unwrap();

    let claims = app.get_events_by_kind(1634).await;
    assert_eq!(claims.len(), 1);

    // Comment on different issues
    let _c1 = app.comment_on_issue(&issue1.id, "Comment 1").await.unwrap();
    let _c2 = app.comment_on_issue(&issue3.id, "Comment 2").await.unwrap();

    let comments = app.get_events_by_kind(1).await;
    assert_eq!(comments.len(), 2);

    app.shutdown().await;
}
