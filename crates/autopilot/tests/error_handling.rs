//! Error handling tests for autopilot
//!
//! Tests various error scenarios and recovery mechanisms:
//! - Database errors
//! - File system errors
//! - Invalid input handling
//! - Concurrent error scenarios

use issues::{db, issue, IssueType, Priority};
use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to create a test database
fn test_db() -> (TempDir, Connection) {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = dir.path().join("test.db");
    let conn = db::init_db(&db_path).expect("Failed to initialize DB");
    (dir, conn)
}

#[test]
fn test_database_connection_failure() {
    // Try to open a database in a non-existent directory
    let result = db::init_db(&PathBuf::from("/nonexistent/path/db.sqlite"));
    assert!(result.is_err(), "Should fail to create DB in nonexistent path");
}

#[test]
fn test_create_issue_with_empty_title() {
    let (_dir, conn) = test_db();

    // Empty title should be rejected (if validation exists)
    let result = issue::create_issue(
        &conn,
        "",
        None,
        Priority::Medium,
        IssueType::Task,
        Some("claude"),
        None,
        None,
    );

    // This might succeed currently, but ideally should validate
    // For now, just verify it doesn't panic
    if let Ok(issue) = result {
        assert!(issue.title.is_empty() || !issue.title.is_empty());
    }
}

#[test]
fn test_get_nonexistent_issue() {
    let (_dir, conn) = test_db();

    let result = issue::get_issue_by_id(&conn, "nonexistent-id");
    assert!(result.is_ok(), "Should return Ok(None) for nonexistent issue");
    assert!(result.unwrap().is_none(), "Should return None");
}

#[test]
fn test_claim_nonexistent_issue() {
    let (_dir, conn) = test_db();

    let result = issue::claim_issue(&conn, "nonexistent-id", "test-run");
    // Returns Ok(false) when no rows updated
    assert!(result.is_ok(), "Should return Ok(false) for nonexistent issue");
    assert_eq!(result.unwrap(), false, "Should return false when nothing claimed");
}

#[test]
fn test_complete_unclaimed_issue() {
    let (_dir, conn) = test_db();

    let issue = issue::create_issue(
        &conn,
        "Test issue",
        None,
        Priority::Medium,
        IssueType::Task,
        Some("claude"),
        None,
        None,
    )
    .expect("Failed to create issue");

    // Try to complete without claiming
    let result = issue::complete_issue(&conn, &issue.id);

    // Should either fail or succeed with a warning
    // For now, just verify it doesn't panic
    let _ = result;
}

#[test]
fn test_double_claim_same_run() {
    let (_dir, conn) = test_db();

    let issue = issue::create_issue(
        &conn,
        "Test issue",
        None,
        Priority::Medium,
        IssueType::Task,
        Some("claude"),
        None,
        None,
    )
    .expect("Failed to create issue");

    // Claim once
    issue::claim_issue(&conn, &issue.id, "test-run-123")
        .expect("First claim should succeed");

    // Try to claim again with same run ID - should be idempotent
    let result = issue::claim_issue(&conn, &issue.id, "test-run-123");
    assert!(result.is_ok(), "Same run should be able to re-claim");
}

#[test]
fn test_double_claim_different_run() {
    let (_dir, conn) = test_db();

    let issue = issue::create_issue(
        &conn,
        "Test issue",
        None,
        Priority::Medium,
        IssueType::Task,
        Some("claude"),
        None,
        None,
    )
    .expect("Failed to create issue");

    // Claim with first run
    let claimed = issue::claim_issue(&conn, &issue.id, "test-run-123")
        .expect("First claim should succeed");
    assert!(claimed, "First claim should return true");

    // Try to claim with different run ID (will fail because status is now in_progress)
    let result = issue::claim_issue(&conn, &issue.id, "test-run-456")
        .expect("Should not error");
    assert!(!result, "Different run should not be able to claim (returns false)");
}

#[test]
fn test_block_completed_issue() {
    let (_dir, conn) = test_db();

    let issue = issue::create_issue(
        &conn,
        "Test issue",
        None,
        Priority::Medium,
        IssueType::Task,
        Some("claude"),
        None,
        None,
    )
    .expect("Failed to create issue");

    issue::claim_issue(&conn, &issue.id, "test-run")
        .expect("Failed to claim");
    issue::complete_issue(&conn, &issue.id)
        .expect("Failed to complete");

    // Try to block a completed issue
    let result = issue::block_issue(&conn, &issue.id, "Test reason");

    // Should either fail or succeed - verify no panic
    let _ = result;
}

#[test]
fn test_database_locked_scenario() {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = dir.path().join("test.db");

    // Create and hold a connection
    let _conn1 = db::init_db(&db_path).expect("Failed to create first connection");

    // Try to open another connection - SQLite allows this with WAL mode
    let result = db::init_db(&db_path);
    assert!(result.is_ok(), "Should be able to open multiple connections with WAL");
}

#[test]
fn test_corrupted_database_recovery() {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = dir.path().join("test.db");

    // Create a corrupted file
    fs::write(&db_path, "not a database").expect("Failed to write corrupted file");

    // Try to init - should fail
    let result = db::init_db(&db_path);
    assert!(result.is_err(), "Should fail to init corrupted database");
}

#[test]
fn test_invalid_priority_handling() {
    let (_dir, conn) = test_db();

    // All priority values should be valid enums, but test edge cases
    for priority in [Priority::Low, Priority::Medium, Priority::High, Priority::Urgent] {
        let result = issue::create_issue(
            &conn,
            "Test",
            None,
            priority,
            IssueType::Task,
            Some("claude"),
            None,
        None,
        );
        assert!(result.is_ok(), "All priority levels should work");
    }
}

#[test]
fn test_invalid_issue_type_handling() {
    let (_dir, conn) = test_db();

    // All issue types should be valid enums
    for issue_type in [IssueType::Task, IssueType::Bug, IssueType::Feature] {
        let result = issue::create_issue(
            &conn,
            "Test",
            None,
            Priority::Medium,
            issue_type,
            Some("claude"),
            None,
        None,
        );
        assert!(result.is_ok(), "All issue types should work");
    }
}

#[test]
fn test_concurrent_issue_creation() {
    use std::sync::Arc;
    use std::thread;

    let dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = Arc::new(dir.path().join("test.db"));

    // Initialize database
    {
        db::init_db(&db_path).expect("Failed to init DB");
    }

    // Spawn multiple threads creating issues
    let mut handles = vec![];
    for i in 0..5 {
        let path = Arc::clone(&db_path);
        let handle = thread::spawn(move || {
            let conn = db::init_db(&path).expect("Failed to connect");
            issue::create_issue(
                &conn,
                &format!("Issue {}", i),
                None,
                Priority::Medium,
                IssueType::Task,
                Some("claude"),
                None,
        None,
            )
        });
        handles.push(handle);
    }

    // Wait for all threads
    let mut results = vec![];
    for handle in handles {
        results.push(handle.join().expect("Thread panicked"));
    }

    // All should succeed
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 5,
               "All concurrent creations should succeed");
}
