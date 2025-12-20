//! Tests for the issue export functionality

use issues::{db, issue, IssueType, Priority, Status};
use rusqlite::Connection;
use serde_json::Value;
use tempfile::TempDir;

/// Helper to create a test database
fn test_db() -> (TempDir, Connection) {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = dir.path().join("test.db");
    let conn = db::init_db(&db_path).expect("Failed to initialize DB");
    (dir, conn)
}

#[test]
fn test_export_issues_to_json() {
    let (_dir, conn) = test_db();

    // Create test issues
    let _issue1 = issue::create_issue(
        &conn,
        "Test issue 1",
        Some("Description 1"),
        Priority::High,
        IssueType::Task,
        Some("claude"),
    )
    .expect("Failed to create issue");

    let issue2 = issue::create_issue(
        &conn,
        "Test issue 2",
        Some("Description 2"),
        Priority::Medium,
        IssueType::Bug,
        Some("claude"),
    )
    .expect("Failed to create issue");

    // Complete one issue
    issue::complete_issue(&conn, &issue2.id).expect("Failed to complete issue");

    // Get all issues
    let issues = issue::list_issues(&conn, None).expect("Failed to list issues");
    assert_eq!(issues.len(), 2);

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&issues).expect("Failed to serialize");

    // Verify JSON is valid
    let parsed: Value = serde_json::from_str(&json).expect("Failed to parse JSON");
    assert!(parsed.is_array());

    let array = parsed.as_array().expect("Should be array");
    assert_eq!(array.len(), 2);

    // Verify first issue
    let first = &array[0];
    assert_eq!(first["title"], "Test issue 1");
    assert_eq!(first["description"], "Description 1");
    assert_eq!(first["priority"], "high");
    assert_eq!(first["issue_type"], "task");
    assert_eq!(first["status"], "open");

    // Verify second issue
    let second = &array[1];
    assert_eq!(second["title"], "Test issue 2");
    assert_eq!(second["status"], "done");
}

#[test]
fn test_export_excludes_completed() {
    let (_dir, conn) = test_db();

    // Create test issues
    let issue1 = issue::create_issue(
        &conn,
        "Open issue",
        None,
        Priority::High,
        IssueType::Task,
        Some("claude"),
    )
    .expect("Failed to create issue");

    let issue2 = issue::create_issue(
        &conn,
        "Completed issue",
        None,
        Priority::Medium,
        IssueType::Bug,
        Some("claude"),
    )
    .expect("Failed to create issue");

    // Complete one issue
    issue::complete_issue(&conn, &issue2.id).expect("Failed to complete issue");

    // Get all issues
    let all_issues = issue::list_issues(&conn, None).expect("Failed to list issues");
    assert_eq!(all_issues.len(), 2);

    // Filter out completed
    let open_issues: Vec<_> = all_issues
        .into_iter()
        .filter(|i| i.status != Status::Done)
        .collect();
    assert_eq!(open_issues.len(), 1);
    assert_eq!(open_issues[0].id, issue1.id);

    // Verify JSON only contains open issue
    let json = serde_json::to_string_pretty(&open_issues).expect("Failed to serialize");
    let parsed: Value = serde_json::from_str(&json).expect("Failed to parse JSON");
    let array = parsed.as_array().expect("Should be array");
    assert_eq!(array.len(), 1);
    assert_eq!(array[0]["title"], "Open issue");
}

#[test]
fn test_export_json_roundtrip() {
    let (_dir, conn) = test_db();

    // Create an issue with all fields populated
    let original = issue::create_issue(
        &conn,
        "Complete issue",
        Some("Full description with details"),
        Priority::Urgent,
        IssueType::Feature,
        Some("codex"),
    )
    .expect("Failed to create issue");

    // Claim the issue
    issue::claim_issue(&conn, &original.id, "test-run-123").expect("Failed to claim");

    // Get the issue
    let issues = issue::list_issues(&conn, None).expect("Failed to list issues");
    assert_eq!(issues.len(), 1);

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&issues).expect("Failed to serialize");

    // Deserialize back
    let deserialized: Vec<issue::Issue> =
        serde_json::from_str(&json).expect("Failed to deserialize");
    assert_eq!(deserialized.len(), 1);

    let restored = &deserialized[0];
    assert_eq!(restored.id, original.id);
    assert_eq!(restored.title, original.title);
    assert_eq!(restored.description, original.description);
    assert_eq!(restored.priority, original.priority);
    assert_eq!(restored.issue_type, original.issue_type);
    assert_eq!(restored.status, Status::InProgress);
    assert_eq!(restored.agent, original.agent);
    assert_eq!(restored.number, original.number);
}

#[test]
fn test_export_preserves_number() {
    let (_dir, conn) = test_db();

    // Create multiple issues to test number preservation
    let issue1 = issue::create_issue(
        &conn,
        "Issue 1",
        None,
        Priority::High,
        IssueType::Task,
        Some("claude"),
    )
    .expect("Failed to create issue");

    let issue2 = issue::create_issue(
        &conn,
        "Issue 2",
        None,
        Priority::Medium,
        IssueType::Bug,
        Some("claude"),
    )
    .expect("Failed to create issue");

    // Get issues
    let issues = issue::list_issues(&conn, None).expect("Failed to list issues");

    // Serialize
    let json = serde_json::to_string_pretty(&issues).expect("Failed to serialize");
    let parsed: Value = serde_json::from_str(&json).expect("Failed to parse JSON");
    let array = parsed.as_array().expect("Should be array");

    // Verify numbers are preserved
    assert_eq!(array[0]["number"], issue1.number);
    assert_eq!(array[1]["number"], issue2.number);

    // Verify numbers are sequential
    assert_eq!(issue2.number, issue1.number + 1);
}
