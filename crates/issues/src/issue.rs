//! Issue struct and CRUD operations

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Result, Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::next_issue_number;

/// Issue status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Open,
    InProgress,
    Done,
}

impl Status {
    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Open => "open",
            Status::InProgress => "in_progress",
            Status::Done => "done",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "in_progress" => Status::InProgress,
            "done" => Status::Done,
            _ => Status::Open,
        }
    }
}

/// Issue priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Urgent,
    High,
    #[default]
    Medium,
    Low,
}

impl Priority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::Urgent => "urgent",
            Priority::High => "high",
            Priority::Medium => "medium",
            Priority::Low => "low",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "urgent" => Priority::Urgent,
            "high" => Priority::High,
            "low" => Priority::Low,
            _ => Priority::Medium,
        }
    }
}

/// Issue type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum IssueType {
    #[default]
    Task,
    Bug,
    Feature,
}

impl IssueType {
    pub fn as_str(&self) -> &'static str {
        match self {
            IssueType::Task => "task",
            IssueType::Bug => "bug",
            IssueType::Feature => "feature",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "bug" => IssueType::Bug,
            "feature" => IssueType::Feature,
            _ => IssueType::Task,
        }
    }
}

/// An issue in the tracking system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,
    pub number: i32,
    pub title: String,
    pub description: Option<String>,
    pub status: Status,
    pub priority: Priority,
    pub issue_type: IssueType,
    pub agent: String,
    pub directive_id: Option<String>,
    pub project_id: Option<String>,
    pub is_blocked: bool,
    pub blocked_reason: Option<String>,
    pub claimed_by: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    /// Whether this issue was automatically created by anomaly detection
    pub auto_created: bool,
}

impl Issue {
    /// Parse datetime from either RFC3339 or simple "YYYY-MM-DD HH:MM:SS" format
    fn parse_datetime(s: &str) -> std::result::Result<DateTime<Utc>, chrono::ParseError> {
        // Try RFC3339 first
        if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
            return Ok(dt.with_timezone(&Utc));
        }
        // Fall back to simple format "YYYY-MM-DD HH:MM:SS"
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
            .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
    }

    fn from_row(row: &Row) -> Result<Self> {
        Ok(Issue {
            id: row.get("id")?,
            number: row.get("number")?,
            title: row.get("title")?,
            description: row.get("description")?,
            status: Status::from_str(&row.get::<_, String>("status")?),
            priority: Priority::from_str(&row.get::<_, String>("priority")?),
            issue_type: IssueType::from_str(&row.get::<_, String>("issue_type")?),
            agent: row
                .get::<_, Option<String>>("agent")?
                .unwrap_or_else(|| "claude".to_string()),
            directive_id: row.get("directive_id")?,
            project_id: row.get("project_id")?,
            is_blocked: row.get::<_, i32>("is_blocked")? != 0,
            blocked_reason: row.get("blocked_reason")?,
            claimed_by: row.get("claimed_by")?,
            claimed_at: row
                .get::<_, Option<String>>("claimed_at")?
                .and_then(|s| Self::parse_datetime(&s).ok()),
            created_at: Self::parse_datetime(&row.get::<_, String>("created_at")?).map_err(
                |e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                },
            )?,
            updated_at: Self::parse_datetime(&row.get::<_, String>("updated_at")?).map_err(
                |e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                },
            )?,
            completed_at: row
                .get::<_, Option<String>>("completed_at")?
                .and_then(|s| Self::parse_datetime(&s).ok()),
            auto_created: row.get::<_, Option<i32>>("auto_created")?.unwrap_or(0) != 0,
        })
    }
}

/// Create a new issue
pub fn create_issue(
    conn: &Connection,
    title: &str,
    description: Option<&str>,
    priority: Priority,
    issue_type: IssueType,
    agent: Option<&str>,
    directive_id: Option<&str>,
    project_id: Option<&str>,
) -> Result<Issue> {
    create_issue_with_auto(
        conn,
        title,
        description,
        priority,
        issue_type,
        agent,
        directive_id,
        project_id,
        false,
    )
}

/// Create a new issue with auto_created flag
pub fn create_issue_with_auto(
    conn: &Connection,
    title: &str,
    description: Option<&str>,
    priority: Priority,
    issue_type: IssueType,
    agent: Option<&str>,
    directive_id: Option<&str>,
    project_id: Option<&str>,
    auto_created: bool,
) -> Result<Issue> {
    let id = Uuid::new_v4().to_string();
    let number = next_issue_number(conn)?;
    let now = Utc::now().to_rfc3339();
    let agent = agent.unwrap_or("claude");

    conn.execute(
        r#"
        INSERT INTO issues (id, number, title, description, priority, issue_type, agent, directive_id, project_id, auto_created, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        params![
            id,
            number,
            title,
            description,
            priority.as_str(),
            issue_type.as_str(),
            agent,
            directive_id,
            project_id,
            if auto_created { 1 } else { 0 },
            now,
            now,
        ],
    )?;

    get_issue_by_id(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

/// Get an issue by ID
pub fn get_issue_by_id(conn: &Connection, id: &str) -> Result<Option<Issue>> {
    conn.query_row("SELECT * FROM issues WHERE id = ?", [id], Issue::from_row)
        .optional()
}

/// Get an issue by number
pub fn get_issue_by_number(conn: &Connection, number: i32) -> Result<Option<Issue>> {
    conn.query_row(
        "SELECT * FROM issues WHERE number = ?",
        [number],
        Issue::from_row,
    )
    .optional()
}

/// List all issues, optionally filtered by status
pub fn list_issues(conn: &Connection, status: Option<Status>) -> Result<Vec<Issue>> {
    let mut issues = Vec::new();

    match status {
        Some(s) => {
            let mut stmt = conn.prepare("SELECT * FROM issues WHERE status = ? ORDER BY number")?;
            let rows = stmt.query_map([s.as_str()], Issue::from_row)?;
            for row in rows {
                issues.push(row?);
            }
        }
        None => {
            let mut stmt = conn.prepare("SELECT * FROM issues ORDER BY number")?;
            let rows = stmt.query_map([], Issue::from_row)?;
            for row in rows {
                issues.push(row?);
            }
        }
    }

    Ok(issues)
}

/// List auto-created issues (created by automated anomaly detection)
pub fn list_auto_created_issues(conn: &Connection, status: Option<Status>) -> Result<Vec<Issue>> {
    let mut issues = Vec::new();

    match status {
        Some(s) => {
            let mut stmt = conn.prepare(
                "SELECT * FROM issues WHERE auto_created = 1 AND status = ? ORDER BY number DESC",
            )?;
            let rows = stmt.query_map([s.as_str()], Issue::from_row)?;
            for row in rows {
                issues.push(row?);
            }
        }
        None => {
            let mut stmt =
                conn.prepare("SELECT * FROM issues WHERE auto_created = 1 ORDER BY number DESC")?;
            let rows = stmt.query_map([], Issue::from_row)?;
            for row in rows {
                issues.push(row?);
            }
        }
    }

    Ok(issues)
}

/// Get the next ready issue (open, not blocked, not claimed or claim expired)
/// Optionally filter by agent (e.g., "claude" or "codex")
pub fn get_next_ready_issue(conn: &Connection, agent: Option<&str>) -> Result<Option<Issue>> {
    match agent {
        Some(agent_filter) => conn
            .query_row(
                r#"
                SELECT * FROM issues
                WHERE status = 'open'
                  AND is_blocked = 0
                  AND (claimed_by IS NULL OR claimed_at < datetime('now', '-15 minutes'))
                  AND id IS NOT NULL
                  AND id != ''
                  AND agent = ?
                ORDER BY
                  CASE priority
                    WHEN 'urgent' THEN 0
                    WHEN 'high' THEN 1
                    WHEN 'medium' THEN 2
                    WHEN 'low' THEN 3
                  END,
                  created_at ASC
                LIMIT 1
                "#,
                [agent_filter],
                Issue::from_row,
            )
            .optional(),
        None => conn
            .query_row(
                r#"
                SELECT * FROM issues
                WHERE status = 'open'
                  AND is_blocked = 0
                  AND (claimed_by IS NULL OR claimed_at < datetime('now', '-15 minutes'))
                  AND id IS NOT NULL
                  AND id != ''
                ORDER BY
                  CASE priority
                    WHEN 'urgent' THEN 0
                    WHEN 'high' THEN 1
                    WHEN 'medium' THEN 2
                    WHEN 'low' THEN 3
                  END,
                  created_at ASC
                LIMIT 1
                "#,
                [],
                Issue::from_row,
            )
            .optional(),
    }
}

/// Claim an issue for a run
pub fn claim_issue(conn: &Connection, issue_id: &str, run_id: &str) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let updated = conn.execute(
        r#"
        UPDATE issues SET
          status = 'in_progress',
          claimed_by = ?,
          claimed_at = ?,
          updated_at = ?
        WHERE id = ?
          AND status = 'open'
          AND is_blocked = 0
          AND (claimed_by IS NULL OR claimed_at < datetime('now', '-15 minutes'))
        "#,
        params![run_id, now, now, issue_id],
    )?;
    Ok(updated > 0)
}

/// Release a claim on an issue (without completing it)
pub fn unclaim_issue(conn: &Connection, issue_id: &str) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let updated = conn.execute(
        r#"
        UPDATE issues SET
          status = 'open',
          claimed_by = NULL,
          claimed_at = NULL,
          updated_at = ?
        WHERE id = ?
        "#,
        params![now, issue_id],
    )?;
    Ok(updated > 0)
}

/// Mark an issue as complete
pub fn complete_issue(conn: &Connection, issue_id: &str) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let updated = conn.execute(
        r#"
        UPDATE issues SET
          status = 'done',
          claimed_by = NULL,
          claimed_at = NULL,
          completed_at = ?,
          updated_at = ?
        WHERE id = ?
        "#,
        params![now, now, issue_id],
    )?;
    Ok(updated > 0)
}

/// Block an issue with a reason
pub fn block_issue(conn: &Connection, issue_id: &str, reason: &str) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let updated = conn.execute(
        r#"
        UPDATE issues SET
          is_blocked = 1,
          blocked_reason = ?,
          status = 'open',
          claimed_by = NULL,
          claimed_at = NULL,
          updated_at = ?
        WHERE id = ?
        "#,
        params![reason, now, issue_id],
    )?;
    Ok(updated > 0)
}

/// Unblock an issue
pub fn unblock_issue(conn: &Connection, issue_id: &str) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let updated = conn.execute(
        r#"
        UPDATE issues SET
          is_blocked = 0,
          blocked_reason = NULL,
          updated_at = ?
        WHERE id = ?
        "#,
        params![now, issue_id],
    )?;
    Ok(updated > 0)
}

/// Release all stale in_progress issues back to open
///
/// An issue is considered stale if it's been claimed for more than the specified
/// duration (in minutes). This is useful for cleaning up issues that were claimed
/// but abandoned.
///
/// Returns the number of issues released.
pub fn release_stale_issues(conn: &Connection, stale_minutes: i32) -> Result<usize> {
    let now = Utc::now().to_rfc3339();
    let updated = conn.execute(
        r#"
        UPDATE issues SET
          status = 'open',
          claimed_by = NULL,
          claimed_at = NULL,
          updated_at = ?
        WHERE status = 'in_progress'
          AND claimed_at < datetime('now', '-' || ? || ' minutes')
        "#,
        params![now, stale_minutes],
    )?;
    Ok(updated)
}

/// Update issue fields
pub fn update_issue(
    conn: &Connection,
    issue_id: &str,
    title: Option<&str>,
    description: Option<&str>,
    priority: Option<Priority>,
    issue_type: Option<IssueType>,
) -> Result<bool> {
    let now = Utc::now().to_rfc3339();

    // Build dynamic update query
    let mut updates = vec!["updated_at = ?"];
    let mut has_changes = false;

    if title.is_some() {
        updates.push("title = ?");
        has_changes = true;
    }
    if description.is_some() {
        updates.push("description = ?");
        has_changes = true;
    }
    if priority.is_some() {
        updates.push("priority = ?");
        has_changes = true;
    }
    if issue_type.is_some() {
        updates.push("issue_type = ?");
        has_changes = true;
    }

    if !has_changes {
        return Ok(false);
    }

    let sql = format!("UPDATE issues SET {} WHERE id = ?", updates.join(", "));

    // Build params dynamically
    let mut param_values: Vec<String> = vec![now];
    if let Some(t) = title {
        param_values.push(t.to_string());
    }
    if let Some(d) = description {
        param_values.push(d.to_string());
    }
    if let Some(p) = priority {
        param_values.push(p.as_str().to_string());
    }
    if let Some(it) = issue_type {
        param_values.push(it.as_str().to_string());
    }
    param_values.push(issue_id.to_string());

    let params: Vec<&dyn rusqlite::ToSql> = param_values
        .iter()
        .map(|s| s as &dyn rusqlite::ToSql)
        .collect();

    let updated = conn.execute(&sql, params.as_slice())?;
    Ok(updated > 0)
}

/// Delete an issue (hard delete)
pub fn delete_issue(conn: &Connection, issue_id: &str) -> Result<bool> {
    // First delete related events
    conn.execute("DELETE FROM issue_events WHERE issue_id = ?", [issue_id])?;
    // Then delete the issue
    let deleted = conn.execute("DELETE FROM issues WHERE id = ?", [issue_id])?;
    Ok(deleted > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_memory_db;

    #[test]
    fn test_create_and_get_issue() {
        let conn = init_memory_db().unwrap();
        let issue = create_issue(
            &conn,
            "Test issue",
            Some("Description"),
            Priority::High,
            IssueType::Bug,
            None,
            None,
            None,
        )
        .unwrap();

        assert_eq!(issue.number, 1);
        assert_eq!(issue.title, "Test issue");
        assert_eq!(issue.description, Some("Description".to_string()));
        assert_eq!(issue.priority, Priority::High);
        assert_eq!(issue.issue_type, IssueType::Bug);
        assert_eq!(issue.status, Status::Open);
        assert_eq!(issue.agent, "claude");

        let fetched = get_issue_by_number(&conn, 1).unwrap().unwrap();
        assert_eq!(fetched.id, issue.id);
    }

    #[test]
    fn test_create_issue_with_agent() {
        let conn = init_memory_db().unwrap();
        let issue = create_issue(
            &conn,
            "Codex task",
            None,
            Priority::Medium,
            IssueType::Task,
            Some("codex"),
            None,
            None,
        )
        .unwrap();

        assert_eq!(issue.agent, "codex");
    }

    #[test]
    fn test_claim_and_complete() {
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

        // Claim it
        assert!(claim_issue(&conn, &issue.id, "run-123").unwrap());

        let claimed = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
        assert_eq!(claimed.status, Status::InProgress);
        assert_eq!(claimed.claimed_by, Some("run-123".to_string()));

        // Complete it
        assert!(complete_issue(&conn, &issue.id).unwrap());

        let completed = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
        assert_eq!(completed.status, Status::Done);
        assert!(completed.claimed_by.is_none());
        assert!(completed.completed_at.is_some());
    }

    #[test]
    fn test_block_unblock() {
        let conn = init_memory_db().unwrap();
        let issue = create_issue(
            &conn,
            "Blocked task",
            None,
            Priority::Medium,
            IssueType::Task,
            None,
            None,
            None,
        )
        .unwrap();

        // Block it
        assert!(block_issue(&conn, &issue.id, "Waiting on dependency").unwrap());

        let blocked = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
        assert!(blocked.is_blocked);
        assert_eq!(
            blocked.blocked_reason,
            Some("Waiting on dependency".to_string())
        );

        // Should not appear in ready queue
        assert!(get_next_ready_issue(&conn, None).unwrap().is_none());

        // Unblock it
        assert!(unblock_issue(&conn, &issue.id).unwrap());

        let unblocked = get_issue_by_id(&conn, &issue.id).unwrap().unwrap();
        assert!(!unblocked.is_blocked);
        assert!(unblocked.blocked_reason.is_none());

        // Should now appear in ready queue
        let ready = get_next_ready_issue(&conn, None).unwrap().unwrap();
        assert_eq!(ready.id, issue.id);
    }

    #[test]
    fn test_agent_filtering() {
        let conn = init_memory_db().unwrap();

        // Create claude and codex issues
        create_issue(
            &conn,
            "Claude task",
            None,
            Priority::High,
            IssueType::Task,
            Some("claude"),
            None,
            None,
        )
        .unwrap();
        create_issue(
            &conn,
            "Codex task",
            None,
            Priority::Urgent,
            IssueType::Task,
            Some("codex"),
            None,
            None,
        )
        .unwrap();

        // Without filter, should return highest priority (codex)
        let next = get_next_ready_issue(&conn, None).unwrap().unwrap();
        assert_eq!(next.title, "Codex task");

        // With claude filter, should return claude task
        let claude_next = get_next_ready_issue(&conn, Some("claude"))
            .unwrap()
            .unwrap();
        assert_eq!(claude_next.title, "Claude task");

        // With codex filter, should return codex task
        let codex_next = get_next_ready_issue(&conn, Some("codex")).unwrap().unwrap();
        assert_eq!(codex_next.title, "Codex task");
    }

    #[test]
    fn test_priority_ordering() {
        let conn = init_memory_db().unwrap();

        // Create issues in reverse priority order
        create_issue(
            &conn,
            "Low",
            None,
            Priority::Low,
            IssueType::Task,
            None,
            None,
            None,
        )
        .unwrap();
        create_issue(
            &conn,
            "High",
            None,
            Priority::High,
            IssueType::Task,
            None,
            None,
            None,
        )
        .unwrap();
        create_issue(
            &conn,
            "Urgent",
            None,
            Priority::Urgent,
            IssueType::Task,
            None,
            None,
            None,
        )
        .unwrap();
        create_issue(
            &conn,
            "Medium",
            None,
            Priority::Medium,
            IssueType::Task,
            None,
            None,
            None,
        )
        .unwrap();

        // Next ready should be urgent
        let next = get_next_ready_issue(&conn, None).unwrap().unwrap();
        assert_eq!(next.title, "Urgent");
    }

    #[test]
    fn test_delete_issue() {
        let conn = init_memory_db().unwrap();

        // Create an issue
        let issue = create_issue(
            &conn,
            "Test delete",
            None,
            Priority::Medium,
            IssueType::Task,
            None,
            None,
            None,
        )
        .unwrap();

        // Verify it exists
        assert!(get_issue_by_id(&conn, &issue.id).unwrap().is_some());
        assert!(get_issue_by_number(&conn, issue.number).unwrap().is_some());

        // Delete it
        assert!(delete_issue(&conn, &issue.id).unwrap());

        // Verify it's gone
        assert!(get_issue_by_id(&conn, &issue.id).unwrap().is_none());
        assert!(get_issue_by_number(&conn, issue.number).unwrap().is_none());
    }

    #[test]
    fn test_delete_nonexistent_issue() {
        let conn = init_memory_db().unwrap();

        // Try to delete an issue that doesn't exist
        assert!(!delete_issue(&conn, "nonexistent-id").unwrap());
    }

    #[test]
    fn test_delete_issue_with_events() {
        let conn = init_memory_db().unwrap();

        // Create an issue
        let issue = create_issue(
            &conn,
            "Issue with events",
            None,
            Priority::High,
            IssueType::Bug,
            None,
            None,
            None,
        )
        .unwrap();

        // Add an event manually (simulating the event log)
        conn.execute(
            "INSERT INTO issue_events (id, issue_id, event_type, created_at) VALUES (?, ?, ?, datetime('now'))",
            ["event-1", &issue.id, "created"]
        ).unwrap();

        // Delete the issue - should cascade delete events
        assert!(delete_issue(&conn, &issue.id).unwrap());

        // Verify issue and events are gone
        assert!(get_issue_by_id(&conn, &issue.id).unwrap().is_none());

        let event_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM issue_events WHERE issue_id = ?",
                [&issue.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(event_count, 0);
    }
}
