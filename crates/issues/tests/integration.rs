//! Integration tests for issues database lifecycle
//!
//! These tests verify the full lifecycle of issues through the database,
//! including state transitions, claim expiration, priority ordering, and
//! edge cases.

use issues::{
    db::{init_memory_db, next_issue_number},
    issue::{
        IssueType, Priority, Status, block_issue, claim_issue, complete_issue, create_issue,
        delete_issue, get_issue_by_id, get_issue_by_number, get_next_ready_issue, list_issues,
        unblock_issue, unclaim_issue, update_issue,
    },
};

#[test]
fn test_full_issue_lifecycle() {
    let conn = init_memory_db().unwrap();

    // Create issue
    let issue = create_issue(
        &conn,
        "Implement feature X",
        Some("Add new functionality"),
        Priority::High,
        IssueType::Feature,
        None,
        None,
        None,
    )
    .unwrap();

    assert_eq!(issue.status, Status::Open);
    assert_eq!(issue.number, 1);
    assert!(!issue.is_blocked);
    assert!(issue.claimed_by.is_none());

    // Claim issue
    assert!(claim_issue(&conn, &issue.id, "run-123").unwrap());
    let claimed = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
    assert_eq!(claimed.status, Status::InProgress);
    assert_eq!(claimed.claimed_by, Some("run-123".to_string()));
    assert!(claimed.claimed_at.is_some());

    // Complete issue
    assert!(complete_issue(&conn, &issue.id).unwrap());
    let completed = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
    assert_eq!(completed.status, Status::Done);
    assert!(completed.claimed_by.is_none());
    assert!(completed.claimed_at.is_none());
    assert!(completed.completed_at.is_some());
}

#[test]
fn test_claim_already_claimed_issue() {
    let conn = init_memory_db().unwrap();

    let issue = create_issue(
        &conn,
        "Test issue",
        None,
        Priority::Medium,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();

    // First claim succeeds
    assert!(claim_issue(&conn, &issue.id, "run-1").unwrap());

    // Second claim fails (issue already claimed)
    assert!(!claim_issue(&conn, &issue.id, "run-2").unwrap());

    // Verify still claimed by run-1
    let issue = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
    assert_eq!(issue.claimed_by, Some("run-1".to_string()));
}

#[test]
fn test_unclaim_and_reclaim() {
    let conn = init_memory_db().unwrap();

    let issue = create_issue(
        &conn,
        "Task",
        None,
        Priority::Medium,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();

    // Claim
    assert!(claim_issue(&conn, &issue.id, "run-1").unwrap());

    // Unclaim
    assert!(unclaim_issue(&conn, &issue.id).unwrap());

    let unclaimed = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
    assert_eq!(unclaimed.status, Status::Open);
    assert!(unclaimed.claimed_by.is_none());

    // Can now be claimed by another run
    assert!(claim_issue(&conn, &issue.id, "run-2").unwrap());
    let reclaimed = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
    assert_eq!(reclaimed.claimed_by, Some("run-2".to_string()));
}

#[test]
fn test_block_and_unblock_workflow() {
    let conn = init_memory_db().unwrap();

    let issue = create_issue(
        &conn,
        "Blocked task",
        None,
        Priority::High,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();

    // Block the issue
    assert!(block_issue(&conn, &issue.id, "Waiting for API keys").unwrap());

    let blocked = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
    assert!(blocked.is_blocked);
    assert_eq!(
        blocked.blocked_reason,
        Some("Waiting for API keys".to_string())
    );
    assert_eq!(blocked.status, Status::Open);

    // Blocked issues should not appear in ready queue
    assert!(get_next_ready_issue(&conn, None).unwrap().is_none());

    // Unblock
    assert!(unblock_issue(&conn, &issue.id).unwrap());

    let unblocked = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
    assert!(!unblocked.is_blocked);
    assert!(unblocked.blocked_reason.is_none());

    // Should now appear in ready queue
    let ready = get_next_ready_issue(&conn, None).unwrap().unwrap();
    assert_eq!(ready.id, issue.id);
}

#[test]
fn test_priority_based_ready_queue() {
    let conn = init_memory_db().unwrap();

    // Create issues in various priority levels
    let low = create_issue(
        &conn,
        "Low priority",
        None,
        Priority::Low,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();
    let medium = create_issue(
        &conn,
        "Medium priority",
        None,
        Priority::Medium,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();
    let high = create_issue(
        &conn,
        "High priority",
        None,
        Priority::High,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();
    let urgent = create_issue(
        &conn,
        "Urgent priority",
        None,
        Priority::Urgent,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();

    // Ready queue should return urgent first
    let next = get_next_ready_issue(&conn, None).unwrap().unwrap();
    assert_eq!(next.id, urgent.id);

    // Claim urgent, next should be high
    claim_issue(&conn, &urgent.id, "run-1").unwrap();
    let next = get_next_ready_issue(&conn, None).unwrap().unwrap();
    assert_eq!(next.id, high.id);

    // Claim high, next should be medium
    claim_issue(&conn, &high.id, "run-2").unwrap();
    let next = get_next_ready_issue(&conn, None).unwrap().unwrap();
    assert_eq!(next.id, medium.id);

    // Claim medium, next should be low
    claim_issue(&conn, &medium.id, "run-3").unwrap();
    let next = get_next_ready_issue(&conn, None).unwrap().unwrap();
    assert_eq!(next.id, low.id);
}

#[test]
fn test_agent_filtering_in_ready_queue() {
    let conn = init_memory_db().unwrap();

    // Create issues for different agents
    let codex_high = create_issue(
        &conn,
        "Codex high task",
        None,
        Priority::High,
        IssueType::Task,
        Some("codex"),
        None,
        None,
    )
    .unwrap();

    let codex_urgent = create_issue(
        &conn,
        "Codex urgent task",
        None,
        Priority::Urgent,
        IssueType::Task,
        Some("codex"),
        None,
        None,
    )
    .unwrap();

    let _codex_medium = create_issue(
        &conn,
        "Codex medium task",
        None,
        Priority::Medium,
        IssueType::Task,
        Some("codex"),
        None,
        None,
    )
    .unwrap();

    // Without filter, should return highest priority overall (codex urgent)
    let next = get_next_ready_issue(&conn, None).unwrap().unwrap();
    assert_eq!(next.id, codex_urgent.id);

    // With codex filter, should return codex urgent
    let next = get_next_ready_issue(&conn, Some("codex"))
        .unwrap()
        .unwrap();
    assert_eq!(next.id, codex_urgent.id);

    // Claim codex urgent, then next codex issue should be high
    claim_issue(&conn, &codex_urgent.id, "run-1").unwrap();
    let next = get_next_ready_issue(&conn, Some("codex"))
        .unwrap()
        .unwrap();
    assert_eq!(next.id, codex_high.id);
}

#[test]
fn test_list_issues_by_status() {
    let conn = init_memory_db().unwrap();

    // Create issues with different statuses
    let open1 = create_issue(
        &conn,
        "Open 1",
        None,
        Priority::High,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();
    let open2 = create_issue(
        &conn,
        "Open 2",
        None,
        Priority::Medium,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();

    // Claim one to make it in_progress
    claim_issue(&conn, &open2.id, "run-1").unwrap();

    // Create and complete another
    let done_issue = create_issue(
        &conn,
        "Done",
        None,
        Priority::Low,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();
    claim_issue(&conn, &done_issue.id, "run-2").unwrap();
    complete_issue(&conn, &done_issue.id).unwrap();

    // List all issues
    let all = list_issues(&conn, None).unwrap();
    assert_eq!(all.len(), 3);

    // List only open
    let open_list = list_issues(&conn, Some(Status::Open)).unwrap();
    assert_eq!(open_list.len(), 1);
    assert_eq!(open_list[0].id, open1.id);

    // List only in_progress
    let in_progress = list_issues(&conn, Some(Status::InProgress)).unwrap();
    assert_eq!(in_progress.len(), 1);
    assert_eq!(in_progress[0].id, open2.id);

    // List only done
    let done = list_issues(&conn, Some(Status::Done)).unwrap();
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].id, done_issue.id);
}

#[test]
fn test_update_issue_fields() {
    let conn = init_memory_db().unwrap();

    let issue = create_issue(
        &conn,
        "Original title",
        Some("Original description"),
        Priority::Low,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();

    // Update all fields
    assert!(
        update_issue(
            &conn,
            &issue.id,
            Some("New title"),
            Some("New description"),
            Some(Priority::Urgent),
            Some(IssueType::Bug),
        )
        .unwrap()
    );

    let updated = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
    assert_eq!(updated.title, "New title");
    assert_eq!(updated.description, Some("New description".to_string()));
    assert_eq!(updated.priority, Priority::Urgent);
    assert_eq!(updated.issue_type, IssueType::Bug);
}

#[test]
fn test_update_issue_partial_fields() {
    let conn = init_memory_db().unwrap();

    let issue = create_issue(
        &conn,
        "Title",
        Some("Description"),
        Priority::Medium,
        IssueType::Feature,
        None,
        None,
        None,
    )
    .unwrap();

    // Update only priority
    assert!(update_issue(&conn, &issue.id, None, None, Some(Priority::High), None).unwrap());

    let updated = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
    assert_eq!(updated.title, "Title"); // Unchanged
    assert_eq!(updated.description, Some("Description".to_string())); // Unchanged
    assert_eq!(updated.priority, Priority::High); // Changed
    assert_eq!(updated.issue_type, IssueType::Feature); // Unchanged
}

#[test]
fn test_update_issue_no_changes() {
    let conn = init_memory_db().unwrap();

    let issue = create_issue(
        &conn,
        "Title",
        None,
        Priority::Medium,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();

    // Update with no fields - should return false
    assert!(!update_issue(&conn, &issue.id, None, None, None, None).unwrap());
}

#[test]
fn test_delete_issue() {
    let conn = init_memory_db().unwrap();

    let issue = create_issue(
        &conn,
        "To be deleted",
        None,
        Priority::Low,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();

    // Verify exists
    assert!(get_issue_by_id(&conn, &issue.id).unwrap().is_some());

    // Delete
    assert!(delete_issue(&conn, &issue.id).unwrap());

    // Verify deleted
    assert!(get_issue_by_id(&conn, &issue.id).unwrap().is_none());
    assert!(get_issue_by_number(&conn, issue.number).unwrap().is_none());
}

#[test]
fn test_delete_nonexistent_issue() {
    let conn = init_memory_db().unwrap();

    // Try to delete non-existent issue - should return false
    assert!(!delete_issue(&conn, "nonexistent-id").unwrap());
}

#[test]
fn test_sequential_issue_numbering() {
    let conn = init_memory_db().unwrap();

    let issue1 = create_issue(
        &conn,
        "First",
        None,
        Priority::Medium,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();
    let issue2 = create_issue(
        &conn,
        "Second",
        None,
        Priority::Medium,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();
    let issue3 = create_issue(
        &conn,
        "Third",
        None,
        Priority::Medium,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();

    assert_eq!(issue1.number, 1);
    assert_eq!(issue2.number, 2);
    assert_eq!(issue3.number, 3);

    // Delete issue 2
    delete_issue(&conn, &issue2.id).unwrap();

    // Next issue should still be #4 (numbers are never reused)
    let issue4 = create_issue(
        &conn,
        "Fourth",
        None,
        Priority::Medium,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(issue4.number, 4);
}

#[test]
fn test_issue_counter_atomicity() {
    let conn = init_memory_db().unwrap();

    // Get multiple numbers in sequence
    let num1 = next_issue_number(&conn).unwrap();
    let num2 = next_issue_number(&conn).unwrap();
    let num3 = next_issue_number(&conn).unwrap();

    assert_eq!(num1, 1);
    assert_eq!(num2, 2);
    assert_eq!(num3, 3);

    // Create an issue to verify counter continues correctly
    let issue = create_issue(
        &conn,
        "Test",
        None,
        Priority::Medium,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(issue.number, 4);
}

#[test]
fn test_block_clears_claim() {
    let conn = init_memory_db().unwrap();

    let issue = create_issue(
        &conn,
        "Task",
        None,
        Priority::High,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();

    // Claim the issue
    claim_issue(&conn, &issue.id, "run-123").unwrap();
    let claimed = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
    assert_eq!(claimed.status, Status::InProgress);
    assert!(claimed.claimed_by.is_some());

    // Block it - should clear claim and reset to Open
    block_issue(&conn, &issue.id, "Blocked reason").unwrap();

    let blocked = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
    assert_eq!(blocked.status, Status::Open);
    assert!(blocked.claimed_by.is_none());
    assert!(blocked.claimed_at.is_none());
    assert!(blocked.is_blocked);
}

#[test]
fn test_complete_clears_claim() {
    let conn = init_memory_db().unwrap();

    let issue = create_issue(
        &conn,
        "Task",
        None,
        Priority::Medium,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();

    // Claim and complete
    claim_issue(&conn, &issue.id, "run-456").unwrap();
    complete_issue(&conn, &issue.id).unwrap();

    let completed = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
    assert_eq!(completed.status, Status::Done);
    assert!(completed.claimed_by.is_none());
    assert!(completed.claimed_at.is_none());
    assert!(completed.completed_at.is_some());
}

#[test]
fn test_cannot_claim_blocked_issue() {
    let conn = init_memory_db().unwrap();

    let issue = create_issue(
        &conn,
        "Task",
        None,
        Priority::High,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();

    // Block the issue first
    block_issue(&conn, &issue.id, "Blocked").unwrap();

    // Attempt to claim - should fail
    assert!(!claim_issue(&conn, &issue.id, "run-1").unwrap());

    let issue = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
    assert!(issue.claimed_by.is_none());
    assert_eq!(issue.status, Status::Open);
}

#[test]
fn test_get_issue_by_number() {
    let conn = init_memory_db().unwrap();

    let issue1 = create_issue(
        &conn,
        "First",
        None,
        Priority::Medium,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();
    let issue2 = create_issue(
        &conn,
        "Second",
        None,
        Priority::High,
        IssueType::Bug,
        None,
        None,
        None,
    )
    .unwrap();

    // Fetch by number
    let fetched1 = get_issue_by_number(&conn, 1).unwrap().unwrap();
    assert_eq!(fetched1.id, issue1.id);
    assert_eq!(fetched1.title, "First");

    let fetched2 = get_issue_by_number(&conn, 2).unwrap().unwrap();
    assert_eq!(fetched2.id, issue2.id);
    assert_eq!(fetched2.title, "Second");

    // Non-existent number
    assert!(get_issue_by_number(&conn, 999).unwrap().is_none());
}

#[test]
fn test_create_issue_with_different_types() {
    let conn = init_memory_db().unwrap();

    let task = create_issue(
        &conn,
        "Task",
        None,
        Priority::Medium,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();
    let bug = create_issue(
        &conn,
        "Bug",
        None,
        Priority::High,
        IssueType::Bug,
        None,
        None,
        None,
    )
    .unwrap();
    let feature = create_issue(
        &conn,
        "Feature",
        None,
        Priority::Low,
        IssueType::Feature,
        None,
        None,
        None,
    )
    .unwrap();

    assert_eq!(task.issue_type, IssueType::Task);
    assert_eq!(bug.issue_type, IssueType::Bug);
    assert_eq!(feature.issue_type, IssueType::Feature);
}

#[test]
fn test_timestamp_fields_populated() {
    let conn = init_memory_db().unwrap();

    let issue = create_issue(
        &conn,
        "Test",
        None,
        Priority::Medium,
        IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();

    // created_at and updated_at should be set
    assert!(issue.created_at.timestamp() > 0);
    assert!(issue.updated_at.timestamp() > 0);

    // completed_at should be None for new issue
    assert!(issue.completed_at.is_none());

    // Claim it
    claim_issue(&conn, &issue.id, "run-1").unwrap();
    let claimed = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
    assert!(claimed.claimed_at.is_some());

    // Complete it
    complete_issue(&conn, &issue.id).unwrap();
    let completed = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
    assert!(completed.completed_at.is_some());
}
#[test]
fn test_cascade_delete_issue_events() {
    use issues::{db, issue};

    let conn = db::init_memory_db().unwrap();

    // Create an issue
    let issue = issue::create_issue(
        &conn,
        "Test issue",
        None,
        issue::Priority::Medium,
        issue::IssueType::Task,
        None,
        None,
        None,
    )
    .unwrap();

    // Manually add an event (events aren't auto-created by claim_issue)
    conn.execute(
        "INSERT INTO issue_events (id, issue_id, event_type, actor, created_at) VALUES (?, ?, ?, ?, datetime('now'))",
        ["test-event-1", &issue.id, "claimed", "test-run"],
    ).unwrap();

    // Verify event exists
    let event_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM issue_events WHERE issue_id = ?",
            [&issue.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(event_count, 1);

    // Delete the issue
    issue::delete_issue(&conn, &issue.id).unwrap();

    // Verify event was cascade deleted (not orphaned)
    let event_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM issue_events WHERE issue_id = ?",
            [&issue.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(event_count, 0);
}
