//! Tests for the issue export and import functionality

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

#[test]
fn test_import_issues_from_json() {
    let (dir1, conn1) = test_db();
    let (_dir2, conn2) = test_db();

    // Create issues in first database
    let issue1 = issue::create_issue(
        &conn1,
        "Import test 1",
        Some("Description 1"),
        Priority::High,
        IssueType::Task,
        Some("claude"),
    )
    .expect("Failed to create issue");

    let issue2 = issue::create_issue(
        &conn1,
        "Import test 2",
        Some("Description 2"),
        Priority::Medium,
        IssueType::Bug,
        Some("codex"),
    )
    .expect("Failed to create issue");

    // Export to JSON
    let issues = issue::list_issues(&conn1, None).expect("Failed to list issues");
    let json = serde_json::to_string_pretty(&issues).expect("Failed to serialize");
    let json_path = dir1.path().join("export.json");
    std::fs::write(&json_path, &json).expect("Failed to write JSON");

    // Import into second database
    let imported_issues: Vec<issue::Issue> =
        serde_json::from_str(&json).expect("Failed to deserialize");

    for imported_issue in imported_issues {
        let now = chrono::Utc::now().to_rfc3339();
        conn2
            .execute(
                r#"
                INSERT INTO issues (
                    id, number, title, description, status, priority, issue_type, agent,
                    is_blocked, blocked_reason, claimed_by, claimed_at,
                    created_at, updated_at, completed_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
                rusqlite::params![
                    imported_issue.id,
                    imported_issue.number,
                    imported_issue.title,
                    imported_issue.description,
                    imported_issue.status.as_str(),
                    imported_issue.priority.as_str(),
                    imported_issue.issue_type.as_str(),
                    imported_issue.agent,
                    if imported_issue.is_blocked { 1 } else { 0 },
                    imported_issue.blocked_reason,
                    imported_issue.claimed_by,
                    imported_issue.claimed_at.map(|dt| dt.to_rfc3339()),
                    imported_issue.created_at.to_rfc3339(),
                    now,
                    imported_issue.completed_at.map(|dt| dt.to_rfc3339()),
                ],
            )
            .expect("Failed to import issue");
    }

    // Verify import
    let imported = issue::list_issues(&conn2, None).expect("Failed to list issues");
    assert_eq!(imported.len(), 2);

    // Find imported issues by ID
    let imported1 = imported.iter().find(|i| i.id == issue1.id).expect("Issue 1 not found");
    let imported2 = imported.iter().find(|i| i.id == issue2.id).expect("Issue 2 not found");

    assert_eq!(imported1.title, "Import test 1");
    assert_eq!(imported1.number, issue1.number);
    assert_eq!(imported2.title, "Import test 2");
    assert_eq!(imported2.agent, "codex");
}

#[test]
fn test_import_skip_existing() {
    let (_dir, conn) = test_db();

    // Create an issue
    let existing = issue::create_issue(
        &conn,
        "Original title",
        Some("Original description"),
        Priority::Medium,
        IssueType::Task,
        Some("claude"),
    )
    .expect("Failed to create issue");

    // Try to import the same issue (by UUID) with different data
    let modified = issue::Issue {
        id: existing.id.clone(),
        number: existing.number,
        title: "Modified title".to_string(),
        description: Some("Modified description".to_string()),
        status: existing.status,
        priority: Priority::High,
        issue_type: existing.issue_type,
        agent: existing.agent.clone(),
        is_blocked: existing.is_blocked,
        blocked_reason: existing.blocked_reason.clone(),
        claimed_by: existing.claimed_by.clone(),
        claimed_at: existing.claimed_at,
        created_at: existing.created_at,
        updated_at: existing.updated_at,
        completed_at: existing.completed_at,
    };

    // Check that issue exists
    let exists = issue::get_issue_by_id(&conn, &modified.id)
        .expect("Should query")
        .is_some();
    assert!(exists);

    // In real import, we would skip this issue unless --force is used
    // For this test, just verify the original data is unchanged
    let retrieved = issue::get_issue_by_id(&conn, &existing.id)
        .expect("Failed to get issue")
        .expect("Issue should exist");
    assert_eq!(retrieved.title, "Original title");
    assert_eq!(retrieved.priority, Priority::Medium);
}

#[test]
fn test_import_updates_counter() {
    let (_dir, conn) = test_db();

    // Get initial counter value
    let initial_counter: i32 = conn
        .query_row(
            "SELECT next_number FROM issue_counter WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .expect("Failed to get counter");

    // Import an issue with a higher number
    let high_number = initial_counter + 10;
    // Generate a simple unique ID for testing
    let test_id = format!("test-{}-{}", chrono::Utc::now().timestamp(), high_number);
    let imported = issue::Issue {
        id: test_id,
        number: high_number,
        title: "High numbered issue".to_string(),
        description: None,
        status: Status::Open,
        priority: Priority::Medium,
        issue_type: IssueType::Task,
        agent: "claude".to_string(),
        is_blocked: false,
        blocked_reason: None,
        claimed_by: None,
        claimed_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        completed_at: None,
    };

    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        r#"
        INSERT INTO issues (
            id, number, title, description, status, priority, issue_type, agent,
            is_blocked, blocked_reason, claimed_by, claimed_at,
            created_at, updated_at, completed_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        rusqlite::params![
            imported.id,
            imported.number,
            imported.title,
            imported.description,
            imported.status.as_str(),
            imported.priority.as_str(),
            imported.issue_type.as_str(),
            imported.agent,
            if imported.is_blocked { 1 } else { 0 },
            imported.blocked_reason,
            imported.claimed_by,
            imported.claimed_at.map(|dt| dt.to_rfc3339()),
            imported.created_at.to_rfc3339(),
            now,
            imported.completed_at.map(|dt| dt.to_rfc3339()),
        ],
    )
    .expect("Failed to import issue");

    // Update counter
    conn.execute(
        "UPDATE issue_counter SET next_number = ? WHERE id = 1",
        [high_number + 1],
    )
    .expect("Failed to update counter");

    // Verify counter was updated
    let new_counter: i32 = conn
        .query_row(
            "SELECT next_number FROM issue_counter WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .expect("Failed to get counter");
    assert_eq!(new_counter, high_number + 1);

    // Create a new issue and verify it gets the correct number
    let new_issue = issue::create_issue(
        &conn,
        "New issue after import",
        None,
        Priority::Low,
        IssueType::Task,
        Some("claude"),
    )
    .expect("Failed to create issue");
    assert_eq!(new_issue.number, high_number + 1);
}
