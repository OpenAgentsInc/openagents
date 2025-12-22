//! Integration tests for autopilot database operations
//!
//! These tests verify that database operations work correctly with the
//! autopilot trajectory system and issue management.

use issues::{db, issue, IssueType, Priority, Status};
use rusqlite::Connection;
use tempfile::TempDir;

/// Helper to create a test database
fn test_db() -> (TempDir, Connection) {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = dir.path().join("test.db");
    let conn = db::init_db(&db_path).expect("Failed to initialize DB");
    (dir, conn)
}

#[test]
fn test_create_and_retrieve_issue() {
    let (_dir, conn) = test_db();

    // Create an issue
    let issue = issue::create_issue(
        &conn,
        "Test task",
        Some("This is a test description"),
        Priority::High,
        IssueType::Task,
        Some("claude"),
        None,
    )
    .expect("Failed to create issue");

    assert_eq!(issue.title, "Test task");
    assert_eq!(issue.description, Some("This is a test description".to_string()));
    assert_eq!(issue.priority, Priority::High);
    assert_eq!(issue.issue_type, IssueType::Task);
    assert_eq!(issue.status, Status::Open);
    assert_eq!(issue.agent, "claude");

    // Retrieve the issue
    let retrieved = issue::get_issue_by_id(&conn, &issue.id).expect("Failed to get issue").expect("Issue should exist");
    assert_eq!(retrieved.id, issue.id);
    assert_eq!(retrieved.title, "Test task");
}

#[test]
fn test_issue_lifecycle() {
    let (_dir, conn) = test_db();

    // Create issue
    let issue = issue::create_issue(
        &conn,
        "Lifecycle test",
        None,
        Priority::Medium,
        IssueType::Bug,
        Some("claude"),
        None,
    )
    .expect("Failed to create issue");

    assert_eq!(issue.status, Status::Open);

    // Claim issue
    issue::claim_issue(&conn, &issue.id, "run_123").expect("Failed to claim issue");
    let claimed = issue::get_issue_by_id(&conn, &issue.id).expect("Failed to get issue").expect("Issue should exist");
    assert_eq!(claimed.status, Status::InProgress);
    assert_eq!(claimed.claimed_by, Some("run_123".to_string()));

    // Complete issue
    issue::complete_issue(&conn, &issue.id).expect("Failed to complete issue");
    let completed = issue::get_issue_by_id(&conn, &issue.id).expect("Failed to get issue").expect("Issue should exist");
    assert_eq!(completed.status, Status::Done);
}

#[test]
fn test_block_issue() {
    let (_dir, conn) = test_db();

    let issue = issue::create_issue(
        &conn,
        "Will be blocked",
        None,
        Priority::Low,
        IssueType::Feature,
        Some("claude"),
        None,
    )
    .expect("Failed to create issue");

    // Block the issue
    issue::block_issue(&conn, &issue.id, "Missing dependencies")
        .expect("Failed to block issue");

    let blocked = issue::get_issue_by_id(&conn, &issue.id).expect("Failed to get issue").expect("Issue should exist");
    assert_eq!(blocked.is_blocked, true);
    assert_eq!(
        blocked.blocked_reason,
        Some("Missing dependencies".to_string())
    );
}

#[test]
fn test_list_issues_by_status() {
    let (_dir, conn) = test_db();

    // Create multiple issues in different states
    let issue1 = issue::create_issue(
        &conn,
        "Open issue 1",
        None,
        Priority::High,
        IssueType::Task,
        Some("claude"),
        None,
    )
    .expect("Failed to create issue");

    let issue2 = issue::create_issue(
        &conn,
        "Open issue 2",
        None,
        Priority::Medium,
        IssueType::Task,
        Some("claude"),
        None,
    )
    .expect("Failed to create issue");

    let issue3 = issue::create_issue(
        &conn,
        "Will complete",
        None,
        Priority::Low,
        IssueType::Task,
        Some("claude"),
        None,
    )
    .expect("Failed to create issue");

    // Complete one issue
    issue::complete_issue(&conn, &issue3.id).expect("Failed to complete issue");

    // List open issues
    let open_issues = issue::list_issues(&conn, Some(Status::Open))
        .expect("Failed to list issues");
    assert_eq!(open_issues.len(), 2);
    assert!(open_issues.iter().any(|i| i.id == issue1.id));
    assert!(open_issues.iter().any(|i| i.id == issue2.id));

    // List done issues
    let done_issues = issue::list_issues(&conn, Some(Status::Done))
        .expect("Failed to list issues");
    assert_eq!(done_issues.len(), 1);
    assert_eq!(done_issues[0].id, issue3.id);

    // List all issues
    let all_issues = issue::list_issues(&conn, None).expect("Failed to list all issues");
    assert_eq!(all_issues.len(), 3);
}

#[test]
fn test_get_next_ready_issue() {
    let (_dir, conn) = test_db();

    // Create issues with different priorities
    let _low = issue::create_issue(
        &conn,
        "Low priority",
        None,
        Priority::Low,
        IssueType::Task,
        Some("claude"),
        None,
    )
    .expect("Failed to create issue");

    let high = issue::create_issue(
        &conn,
        "High priority",
        None,
        Priority::High,
        IssueType::Task,
        Some("claude"),
        None,
    )
    .expect("Failed to create issue");

    let urgent = issue::create_issue(
        &conn,
        "Urgent priority",
        None,
        Priority::Urgent,
        IssueType::Bug,
        Some("claude"),
        None,
    )
    .expect("Failed to create issue");

    // Block the high priority issue
    issue::block_issue(&conn, &high.id, "Blocked").expect("Failed to block issue");

    // Get next ready issue - should be urgent (highest priority, not blocked)
    let next = issue::get_next_ready_issue(&conn, Some("claude"))
        .expect("Failed to get next issue")
        .expect("Should have a ready issue");

    assert_eq!(next.id, urgent.id);
    assert_eq!(next.priority, Priority::Urgent);
}

#[test]
fn test_issue_agent_filter() {
    let (_dir, conn) = test_db();

    // Create issues for different agents
    let claude_issue = issue::create_issue(
        &conn,
        "Claude task",
        None,
        Priority::High,
        IssueType::Task,
        Some("claude"),
        None,
    )
    .expect("Failed to create issue");

    let codex_issue = issue::create_issue(
        &conn,
        "Codex task",
        None,
        Priority::High,
        IssueType::Task,
        Some("codex"),
        None,
    )
    .expect("Failed to create issue");

    // Get next for claude
    let next_claude = issue::get_next_ready_issue(&conn, Some("claude"))
        .expect("Failed to get next issue")
        .expect("Should have a ready issue");
    assert_eq!(next_claude.id, claude_issue.id);

    // Get next for codex
    let next_codex = issue::get_next_ready_issue(&conn, Some("codex"))
        .expect("Failed to get next issue")
        .expect("Should have a ready issue");
    assert_eq!(next_codex.id, codex_issue.id);
}

#[test]
fn test_delete_issue() {
    let (_dir, conn) = test_db();

    let issue = issue::create_issue(
        &conn,
        "Will be deleted",
        None,
        Priority::Medium,
        IssueType::Task,
        Some("claude"),
        None,
    )
    .expect("Failed to create issue");

    let issue_id = issue.id;

    // Verify issue exists
    assert!(issue::get_issue_by_id(&conn, &issue_id).expect("Should query").is_some());

    // Delete issue
    issue::delete_issue(&conn, &issue_id).expect("Failed to delete issue");

    // Verify issue is gone
    assert!(issue::get_issue_by_id(&conn, &issue_id).expect("Should query").is_none());
}

#[test]
fn test_concurrent_issue_claims() {
    let (_dir, conn) = test_db();

    let issue = issue::create_issue(
        &conn,
        "Concurrent test",
        None,
        Priority::High,
        IssueType::Task,
        Some("claude"),
        None,
    )
    .expect("Failed to create issue");

    // First claim should succeed
    let result1 = issue::claim_issue(&conn, &issue.id, "run_1");
    assert_eq!(result1.expect("First claim should succeed"), true);

    // Second claim should return false (already claimed)
    let result2 = issue::claim_issue(&conn, &issue.id, "run_2");
    assert_eq!(result2.expect("Should not error"), false);

    // Verify first claim persisted
    let claimed = issue::get_issue_by_id(&conn, &issue.id).expect("Failed to get issue").expect("Issue should exist");
    assert_eq!(claimed.claimed_by, Some("run_1".to_string()));
}

#[test]
fn test_update_issue() {
    let (_dir, conn) = test_db();

    let issue = issue::create_issue(
        &conn,
        "Original title",
        Some("Original description"),
        Priority::Low,
        IssueType::Task,
        Some("claude"),
        None,
    )
    .expect("Failed to create issue");

    // Update the issue
    issue::update_issue(
        &conn,
        &issue.id,
        Some("Updated title"),
        Some("Updated description"),
        Some(Priority::Urgent),
        Some(IssueType::Bug),
    )
    .expect("Failed to update issue");

    let updated = issue::get_issue_by_id(&conn, &issue.id).expect("Failed to get issue").expect("Issue should exist");
    assert_eq!(updated.title, "Updated title");
    assert_eq!(
        updated.description,
        Some("Updated description".to_string())
    );
    assert_eq!(updated.priority, Priority::Urgent);
    assert_eq!(updated.issue_type, IssueType::Bug);
}

#[test]
fn test_database_persistence() {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = dir.path().join("persist.db");

    // Create issue in first connection
    {
        let conn = db::init_db(&db_path).expect("Failed to initialize DB");
        issue::create_issue(
            &conn,
            "Persistent issue",
            None,
            Priority::Medium,
            IssueType::Task,
            Some("claude"),
        None,
        )
        .expect("Failed to create issue");
    }

    // Reopen database and verify issue persists
    {
        let conn = Connection::open(&db_path).expect("Failed to open DB");
        let issues = issue::list_issues(&conn, None).expect("Failed to list issues");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].title, "Persistent issue");
    }
}

#[test]
fn test_issue_priority_ordering() {
    let (_dir, conn) = test_db();

    // Create issues with all priority levels
    issue::create_issue(
        &conn,
        "Low",
        None,
        Priority::Low,
        IssueType::Task,
        Some("claude"),
        None,
    )
    .expect("Failed to create issue");

    issue::create_issue(
        &conn,
        "Medium",
        None,
        Priority::Medium,
        IssueType::Task,
        Some("claude"),
        None,
    )
    .expect("Failed to create issue");

    let high_issue = issue::create_issue(
        &conn,
        "High",
        None,
        Priority::High,
        IssueType::Task,
        Some("claude"),
        None,
    )
    .expect("Failed to create issue");

    let urgent_issue = issue::create_issue(
        &conn,
        "Urgent",
        None,
        Priority::Urgent,
        IssueType::Task,
        Some("claude"),
        None,
    )
    .expect("Failed to create issue");

    // Get next ready - should be urgent
    let next1 = issue::get_next_ready_issue(&conn, Some("claude"))
        .expect("Failed to get next")
        .expect("Should have issue");
    assert_eq!(next1.id, urgent_issue.id);

    // Claim urgent
    issue::claim_issue(&conn, &urgent_issue.id, "run_1").expect("Failed to claim");

    // Get next ready - should be high
    let next2 = issue::get_next_ready_issue(&conn, Some("claude"))
        .expect("Failed to get next")
        .expect("Should have issue");
    assert_eq!(next2.id, high_issue.id);
}
