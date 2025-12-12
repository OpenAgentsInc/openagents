//! SQLite implementation of IssueRepository
//!
//! This is the main storage backend for taskmaster, ported from Beads.

use std::path::Path;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use uuid::Uuid;

use crate::repository::{IssueRepository, Result, TaskmasterError};
use crate::storage::schema::*;
use crate::types::*;

/// SQLite-backed repository implementation
pub struct SqliteRepository {
    conn: Mutex<Connection>,
}

impl SqliteRepository {
    /// Create a new repository with in-memory database (for testing)
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let repo = Self {
            conn: Mutex::new(conn),
        };
        repo.init()?;
        Ok(repo)
    }

    /// Create a new repository with file-based database
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        let repo = Self {
            conn: Mutex::new(conn),
        };
        repo.init()?;
        Ok(repo)
    }

    /// Generate a new issue ID
    fn generate_id(&self, prefix: &str, method: IdMethod, create: &IssueCreate) -> String {
        match method {
            IdMethod::Random => {
                let uuid = Uuid::new_v4();
                let short = &uuid.to_string()[..8];
                format!("{}-{}", prefix, short)
            }
            IdMethod::Hash => {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(create.title.as_bytes());
                if let Some(desc) = &create.description {
                    hasher.update(desc.as_bytes());
                }
                let hash = hex::encode(hasher.finalize());
                format!("{}-{}", prefix, &hash[..8])
            }
        }
    }

    /// Parse an Issue from a database row
    fn parse_issue_row(row: &Row<'_>) -> rusqlite::Result<Issue> {
        let status_str: String = row.get("status")?;
        let priority_val: i32 = row.get("priority")?;
        let type_str: String = row.get("issue_type")?;

        Ok(Issue {
            id: row.get("id")?,
            title: row.get("title")?,
            description: row.get("description")?,
            design: row.get("design")?,
            acceptance_criteria: row.get("acceptance_criteria")?,
            notes: row.get("notes")?,
            status: status_str.parse().unwrap_or_default(),
            priority: Priority::from_u8(priority_val as u8).unwrap_or_default(),
            issue_type: type_str.parse().unwrap_or_default(),
            assignee: row.get("assignee")?,
            estimated_minutes: row.get("estimated_minutes")?,
            compaction_level: row.get::<_, i32>("compaction_level")? as u32,
            close_reason: row.get("close_reason")?,
            external_ref: row.get("external_ref")?,
            source_repo: row.get("source_repo")?,
            discovered_from: row.get("discovered_from")?,
            content_hash: row.get("content_hash")?,
            created_at: parse_datetime(row.get::<_, String>("created_at")?),
            updated_at: parse_datetime(row.get::<_, String>("updated_at")?),
            closed_at: row.get::<_, Option<String>>("closed_at")?.map(parse_datetime),
            tombstoned_at: row.get::<_, Option<String>>("tombstoned_at")?.map(parse_datetime),
            tombstone_ttl_days: row.get::<_, Option<i32>>("tombstone_ttl_days")?.map(|v| v as u32),
            tombstone_reason: row.get("tombstone_reason")?,
            // Execution context fields (with defaults for V1 schema compatibility)
            execution_mode: row
                .get::<_, Option<String>>("execution_mode")
                .ok()
                .flatten()
                .and_then(|s| s.parse().ok())
                .unwrap_or_default(),
            execution_state: row
                .get::<_, Option<String>>("execution_state")
                .ok()
                .flatten()
                .and_then(|s| s.parse().ok())
                .unwrap_or_default(),
            container_id: row.get("container_id").ok().flatten(),
            agent_id: row.get("agent_id").ok().flatten(),
            execution_branch: row.get("execution_branch").ok().flatten(),
            execution_started_at: row
                .get::<_, Option<String>>("execution_started_at")
                .ok()
                .flatten()
                .map(parse_datetime),
            execution_finished_at: row
                .get::<_, Option<String>>("execution_finished_at")
                .ok()
                .flatten()
                .map(parse_datetime),
            execution_exit_code: row.get("execution_exit_code").ok().flatten(),
            labels: Vec::new(), // Loaded separately
            deps: Vec::new(),   // Loaded separately
        })
    }

    /// Load labels for an issue
    fn load_labels(&self, conn: &Connection, issue_id: &str) -> Result<Vec<String>> {
        let mut stmt = conn.prepare(GET_LABELS)?;
        let labels = stmt
            .query_map([issue_id], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(labels)
    }

    /// Load dependencies for an issue
    fn load_deps(&self, conn: &Connection, issue_id: &str) -> Result<Vec<DependencyRef>> {
        let mut stmt = conn.prepare(GET_DEPENDENCIES)?;
        let deps = stmt
            .query_map([issue_id], |row| {
                let dep_type_str: String = row.get(1)?;
                Ok(DependencyRef {
                    id: row.get(0)?,
                    dep_type: dep_type_str.parse().unwrap_or_default(),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(deps)
    }

    /// Load full issue with labels and deps
    fn load_full_issue(&self, conn: &Connection, mut issue: Issue) -> Result<Issue> {
        issue.labels = self.load_labels(conn, &issue.id)?;
        issue.deps = self.load_deps(conn, &issue.id)?;
        Ok(issue)
    }

    /// Record an event
    fn record_event(
        &self,
        conn: &Connection,
        issue_id: &str,
        event_type: EventType,
        actor: Option<&str>,
        field: Option<&str>,
        old_value: Option<&str>,
        new_value: Option<&str>,
    ) -> Result<()> {
        conn.execute(
            INSERT_EVENT,
            params![
                issue_id,
                event_type.as_str(),
                actor,
                field,
                old_value,
                new_value,
                Option::<String>::None, // metadata
            ],
        )?;
        Ok(())
    }
}

impl IssueRepository for SqliteRepository {
    fn init(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(SCHEMA_V1)?;
        Ok(())
    }

    fn create(&self, issue: IssueCreate, prefix: &str) -> Result<Issue> {
        self.create_with_id_method(issue, IdMethod::Random, prefix)
    }

    fn create_with_id_method(
        &self,
        issue: IssueCreate,
        method: IdMethod,
        prefix: &str,
    ) -> Result<Issue> {
        let conn = self.conn.lock().unwrap();

        let id = self.generate_id(prefix, method, &issue);

        // Check if already exists
        let exists: bool = conn
            .query_row(EXISTS_ISSUE, [&id], |row| row.get(0))
            .optional()?
            .unwrap_or(false);

        if exists {
            return Err(TaskmasterError::already_exists(&id));
        }

        let now = Utc::now();
        let now_str = format_datetime(now);

        // Compute content hash
        let mut hasher = sha2::Sha256::new();
        use sha2::Digest;
        hasher.update(issue.title.as_bytes());
        if let Some(desc) = &issue.description {
            hasher.update(desc.as_bytes());
        }
        let content_hash = hex::encode(hasher.finalize());

        // Insert issue
        conn.execute(
            INSERT_ISSUE,
            params![
                id,
                issue.title,
                issue.description.as_deref().unwrap_or(""),
                issue.design,
                issue.acceptance_criteria,
                issue.notes,
                IssueStatus::Open.as_str(),
                issue.priority.as_u8() as i32,
                issue.issue_type.as_str(),
                issue.assignee,
                issue.estimated_minutes,
                0i32, // compaction_level
                Option::<String>::None, // close_reason
                issue.external_ref,
                issue.source_repo,
                issue.discovered_from,
                content_hash,
                &now_str,
                &now_str,
                Option::<String>::None, // closed_at
                Option::<String>::None, // tombstoned_at
                Option::<i32>::None,    // tombstone_ttl_days
                Option::<String>::None, // tombstone_reason
                // Execution context fields (defaults)
                "none",                 // execution_mode
                "unscheduled",          // execution_state
                Option::<String>::None, // container_id
                Option::<String>::None, // agent_id
                Option::<String>::None, // execution_branch
                Option::<String>::None, // execution_started_at
                Option::<String>::None, // execution_finished_at
                Option::<i32>::None,    // execution_exit_code
            ],
        )?;

        // Insert labels
        for label in &issue.labels {
            conn.execute(INSERT_LABEL, params![&id, label])?;
        }

        // Insert dependencies
        for dep in &issue.deps {
            conn.execute(
                INSERT_DEPENDENCY,
                params![&id, &dep.id, dep.dep_type.as_str()],
            )?;
        }

        // Record event
        self.record_event(&conn, &id, EventType::Created, None, None, None, None)?;

        // Return the created issue
        drop(conn);
        self.get(&id)
    }

    fn get(&self, id: &str) -> Result<Issue> {
        let conn = self.conn.lock().unwrap();
        let issue = conn
            .query_row(GET_ISSUE, [id], Self::parse_issue_row)
            .optional()?
            .ok_or_else(|| TaskmasterError::not_found(id))?;

        self.load_full_issue(&conn, issue)
    }

    fn get_with_tombstones(&self, id: &str) -> Result<Issue> {
        let conn = self.conn.lock().unwrap();
        let issue = conn
            .query_row(GET_ISSUE_WITH_TOMBSTONES, [id], Self::parse_issue_row)
            .optional()?
            .ok_or_else(|| TaskmasterError::not_found(id))?;

        self.load_full_issue(&conn, issue)
    }

    fn exists(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let exists: bool = conn
            .query_row(EXISTS_ISSUE, [id], |row| row.get(0))
            .optional()?
            .unwrap_or(false);
        Ok(exists)
    }

    fn update(&self, id: &str, update: IssueUpdate, actor: Option<&str>) -> Result<Issue> {
        let conn = self.conn.lock().unwrap();

        // Get current issue
        let current = conn
            .query_row(GET_ISSUE_WITH_TOMBSTONES, [id], Self::parse_issue_row)
            .optional()?
            .ok_or_else(|| TaskmasterError::not_found(id))?;

        let now_str = format_datetime(Utc::now());

        // Build update query dynamically
        let mut updates = vec!["updated_at = ?1".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now_str.clone())];
        let mut param_idx = 2;

        macro_rules! add_update {
            ($field:expr, $val:expr, $col:expr) => {
                if let Some(v) = $val {
                    updates.push(format!("{} = ?{}", $col, param_idx));
                    params.push(Box::new(v));
                    param_idx += 1;

                    // Record field change event
                    let old = format!("{:?}", $field);
                    let new = format!("{:?}", params.last().unwrap());
                    self.record_event(&conn, id, EventType::Updated, actor, Some($col), Some(&old), Some(&new))?;
                }
            };
        }

        if let Some(title) = &update.title {
            updates.push(format!("title = ?{}", param_idx));
            params.push(Box::new(title.clone()));
            param_idx += 1;
        }

        if let Some(desc) = &update.description {
            updates.push(format!("description = ?{}", param_idx));
            params.push(Box::new(desc.clone()));
            param_idx += 1;
        }

        if let Some(status) = update.status {
            // Validate transition
            if !current.status.can_transition_to(status) {
                return Err(TaskmasterError::invalid_transition(current.status, status));
            }

            updates.push(format!("status = ?{}", param_idx));
            params.push(Box::new(status.as_str().to_string()));
            param_idx += 1;

            // Handle closed_at
            if status == IssueStatus::Closed {
                updates.push(format!("closed_at = ?{}", param_idx));
                params.push(Box::new(now_str.clone()));
                param_idx += 1;
            } else if current.status == IssueStatus::Closed {
                updates.push(format!("closed_at = ?{}", param_idx));
                params.push(Box::new(Option::<String>::None));
                param_idx += 1;
            }

            self.record_event(
                &conn,
                id,
                EventType::StatusChanged,
                actor,
                Some("status"),
                Some(current.status.as_str()),
                Some(status.as_str()),
            )?;
        }

        if let Some(priority) = update.priority {
            updates.push(format!("priority = ?{}", param_idx));
            params.push(Box::new(priority.as_u8() as i32));
            param_idx += 1;
        }

        if let Some(issue_type) = update.issue_type {
            updates.push(format!("issue_type = ?{}", param_idx));
            params.push(Box::new(issue_type.as_str().to_string()));
            param_idx += 1;
        }

        if let Some(assignee) = &update.assignee {
            updates.push(format!("assignee = ?{}", param_idx));
            params.push(Box::new(assignee.clone()));
            param_idx += 1;
        }

        if let Some(close_reason) = &update.close_reason {
            updates.push(format!("close_reason = ?{}", param_idx));
            params.push(Box::new(close_reason.clone()));
            param_idx += 1;
        }

        // Execution context fields
        if let Some(mode) = update.execution_mode {
            updates.push(format!("execution_mode = ?{}", param_idx));
            params.push(Box::new(mode.to_string()));
            param_idx += 1;
        }

        if let Some(state) = update.execution_state {
            updates.push(format!("execution_state = ?{}", param_idx));
            params.push(Box::new(state.to_string()));
            param_idx += 1;
        }

        if let Some(container_id) = &update.container_id {
            updates.push(format!("container_id = ?{}", param_idx));
            params.push(Box::new(container_id.clone()));
            param_idx += 1;
        }

        if let Some(agent_id) = &update.agent_id {
            updates.push(format!("agent_id = ?{}", param_idx));
            params.push(Box::new(agent_id.clone()));
            param_idx += 1;
        }

        if let Some(branch) = &update.execution_branch {
            updates.push(format!("execution_branch = ?{}", param_idx));
            params.push(Box::new(branch.clone()));
            param_idx += 1;
        }

        if let Some(started) = &update.execution_started_at {
            updates.push(format!("execution_started_at = ?{}", param_idx));
            params.push(Box::new(started.map(format_datetime)));
            param_idx += 1;
        }

        if let Some(finished) = &update.execution_finished_at {
            updates.push(format!("execution_finished_at = ?{}", param_idx));
            params.push(Box::new(finished.map(format_datetime)));
            param_idx += 1;
        }

        if let Some(exit_code) = &update.execution_exit_code {
            updates.push(format!("execution_exit_code = ?{}", param_idx));
            params.push(Box::new(*exit_code));
            let _ = param_idx;
        }

        // Execute update
        let sql = format!("UPDATE issues SET {} WHERE id = ?", updates.join(", "));
        params.push(Box::new(id.to_string()));

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, params_refs.as_slice())?;

        // Handle labels update
        if let Some(labels) = update.labels {
            conn.execute("DELETE FROM issue_labels WHERE issue_id = ?1", [id])?;
            for label in labels {
                conn.execute(INSERT_LABEL, params![id, label])?;
            }
        }

        // Handle deps update
        if let Some(deps) = update.deps {
            conn.execute("DELETE FROM issue_dependencies WHERE issue_id = ?1", [id])?;
            for dep in deps {
                conn.execute(INSERT_DEPENDENCY, params![id, &dep.id, dep.dep_type.as_str()])?;
            }
        }

        drop(conn);
        self.get_with_tombstones(id)
    }

    fn tombstone(&self, id: &str, reason: Option<&str>, actor: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        let now_str = format_datetime(Utc::now());

        conn.execute(
            "UPDATE issues SET status = 'tombstone', tombstoned_at = ?1, tombstone_reason = ?2, updated_at = ?1 WHERE id = ?3",
            params![&now_str, reason, id],
        )?;

        self.record_event(&conn, id, EventType::Tombstoned, actor, None, None, reason)?;

        Ok(())
    }

    fn purge(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM issues WHERE id = ?1", [id])?;
        Ok(())
    }

    fn restore(&self, id: &str, actor: Option<&str>) -> Result<Issue> {
        let conn = self.conn.lock().unwrap();

        let now_str = format_datetime(Utc::now());

        conn.execute(
            "UPDATE issues SET status = 'open', tombstoned_at = NULL, tombstone_reason = NULL, updated_at = ?1 WHERE id = ?2 AND status = 'tombstone'",
            params![&now_str, id],
        )?;

        self.record_event(&conn, id, EventType::Restored, actor, None, None, None)?;

        drop(conn);
        self.get(id)
    }

    fn list(&self, filter: IssueFilter) -> Result<Vec<Issue>> {
        let conn = self.conn.lock().unwrap();

        let mut sql = String::from(
            "SELECT id, title, description, design, acceptance_criteria, notes,
             status, priority, issue_type, assignee, estimated_minutes,
             compaction_level, close_reason, external_ref, source_repo,
             discovered_from, content_hash, created_at, updated_at, closed_at,
             tombstoned_at, tombstone_ttl_days, tombstone_reason
             FROM issues WHERE 1=1",
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        let mut param_idx = 1;

        // Exclude tombstones by default
        if !filter.include_tombstones {
            sql.push_str(" AND status != 'tombstone'");
        }

        // Status filter
        if let Some(statuses) = &filter.status {
            let placeholders: Vec<String> = statuses
                .iter()
                .map(|_| {
                    let p = format!("?{}", param_idx);
                    param_idx += 1;
                    p
                })
                .collect();
            sql.push_str(&format!(" AND status IN ({})", placeholders.join(", ")));
            for s in statuses {
                params.push(Box::new(s.as_str().to_string()));
            }
        }

        // Priority filter
        if let Some(priorities) = &filter.priority {
            let placeholders: Vec<String> = priorities
                .iter()
                .map(|_| {
                    let p = format!("?{}", param_idx);
                    param_idx += 1;
                    p
                })
                .collect();
            sql.push_str(&format!(" AND priority IN ({})", placeholders.join(", ")));
            for p in priorities {
                params.push(Box::new(p.as_u8() as i32));
            }
        }

        // Assignee filter
        if let Some(assignee) = &filter.assignee {
            match assignee {
                AssigneeFilter::Is(name) => {
                    sql.push_str(&format!(" AND assignee = ?{}", param_idx));
                    params.push(Box::new(name.clone()));
                    param_idx += 1;
                }
                AssigneeFilter::IsNot(name) => {
                    sql.push_str(&format!(" AND (assignee IS NULL OR assignee != ?{})", param_idx));
                    params.push(Box::new(name.clone()));
                    param_idx += 1;
                }
                AssigneeFilter::Unassigned => {
                    sql.push_str(" AND assignee IS NULL");
                }
                AssigneeFilter::Assigned => {
                    sql.push_str(" AND assignee IS NOT NULL");
                }
            }
        }

        // Date filters
        if let Some(date) = filter.created_after {
            sql.push_str(&format!(" AND created_at >= ?{}", param_idx));
            params.push(Box::new(format_datetime(date)));
            param_idx += 1;
        }

        if let Some(date) = filter.created_before {
            sql.push_str(&format!(" AND created_at <= ?{}", param_idx));
            params.push(Box::new(format_datetime(date)));
            param_idx += 1;
        }

        if let Some(date) = filter.updated_after {
            sql.push_str(&format!(" AND updated_at >= ?{}", param_idx));
            params.push(Box::new(format_datetime(date)));
            param_idx += 1;
        }

        if let Some(date) = filter.updated_before {
            sql.push_str(&format!(" AND updated_at <= ?{}", param_idx));
            params.push(Box::new(format_datetime(date)));
            param_idx += 1;
        }

        // Title contains
        if let Some(text) = &filter.title_contains {
            sql.push_str(&format!(" AND title LIKE ?{}", param_idx));
            params.push(Box::new(format!("%{}%", text)));
            param_idx += 1;
        }

        // Sort
        sql.push_str(match filter.sort {
            SortPolicy::Hybrid => " ORDER BY priority ASC, created_at ASC",
            SortPolicy::Priority => " ORDER BY priority ASC, created_at DESC",
            SortPolicy::Oldest => " ORDER BY created_at ASC",
            SortPolicy::Newest => " ORDER BY created_at DESC",
            SortPolicy::RecentlyUpdated => " ORDER BY updated_at DESC",
        });

        // Limit & offset
        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT ?{}", param_idx));
            params.push(Box::new(limit as i64));
            param_idx += 1;
        }

        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET ?{}", param_idx));
            params.push(Box::new(offset as i64));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let issues = stmt
            .query_map(params_refs.as_slice(), Self::parse_issue_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        // Load labels and deps for each issue
        let mut full_issues = Vec::with_capacity(issues.len());
        for issue in issues {
            full_issues.push(self.load_full_issue(&conn, issue)?);
        }

        // Apply label filter (post-query for complex AND/OR logic)
        if let Some(label_filter) = &filter.labels {
            let filtered: Vec<Issue> = full_issues
                .into_iter()
                .filter(|issue| match label_filter {
                    LabelFilter::All(labels) => labels.iter().all(|l| issue.labels.contains(l)),
                    LabelFilter::Any(labels) => labels.iter().any(|l| issue.labels.contains(l)),
                    LabelFilter::None => issue.labels.is_empty(),
                    LabelFilter::Expr(_) => true, // TODO: Implement complex expressions
                })
                .collect();
            return Ok(filtered);
        }

        Ok(full_issues)
    }

    fn count(&self, filter: IssueFilter) -> Result<usize> {
        Ok(self.list(filter)?.len())
    }

    fn search(&self, query: &str, filter: IssueFilter) -> Result<Vec<Issue>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT i.id, i.title, i.description, i.design, i.acceptance_criteria, i.notes,
             i.status, i.priority, i.issue_type, i.assignee, i.estimated_minutes,
             i.compaction_level, i.close_reason, i.external_ref, i.source_repo,
             i.discovered_from, i.content_hash, i.created_at, i.updated_at, i.closed_at,
             i.tombstoned_at, i.tombstone_ttl_days, i.tombstone_reason
             FROM issues i
             JOIN issues_fts fts ON i.id = fts.id
             WHERE issues_fts MATCH ?1
             AND i.status != 'tombstone'
             ORDER BY rank",
        )?;

        let issues = stmt
            .query_map([query], Self::parse_issue_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut full_issues = Vec::with_capacity(issues.len());
        for issue in issues {
            full_issues.push(self.load_full_issue(&conn, issue)?);
        }

        Ok(full_issues)
    }

    fn ready(&self, mut filter: IssueFilter) -> Result<Vec<Issue>> {
        filter.status = Some(vec![IssueStatus::Open]);
        filter.include_tombstones = false;

        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(READY_ISSUES)?;
        let issues = stmt
            .query_map([], Self::parse_issue_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut full_issues = Vec::with_capacity(issues.len());
        for issue in issues {
            full_issues.push(self.load_full_issue(&conn, issue)?);
        }

        // Apply additional filters
        if let Some(limit) = filter.limit {
            full_issues.truncate(limit);
        }

        Ok(full_issues)
    }

    fn stale(&self, filter: StaleFilter) -> Result<Vec<Issue>> {
        let conn = self.conn.lock().unwrap();

        let days_modifier = format!("-{} days", filter.days);
        let limit = filter.limit.unwrap_or(100) as i64;

        let mut stmt = conn.prepare(STALE_ISSUES)?;
        let issues = stmt
            .query_map(params![days_modifier, limit], Self::parse_issue_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut full_issues = Vec::with_capacity(issues.len());
        for issue in issues {
            full_issues.push(self.load_full_issue(&conn, issue)?);
        }

        Ok(full_issues)
    }

    fn duplicates(&self) -> Result<Vec<DuplicateGroup>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(FIND_DUPLICATES)?;
        let groups = stmt
            .query_map([], |row| {
                let ids_str: String = row.get(1)?;
                Ok(DuplicateGroup {
                    content_hash: row.get(0)?,
                    issue_ids: ids_str.split(',').map(|s| s.to_string()).collect(),
                    title: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(groups)
    }

    fn start(&self, id: &str, actor: Option<&str>) -> Result<Issue> {
        self.update(id, IssueUpdate::new().status(IssueStatus::InProgress), actor)
    }

    fn close(
        &self,
        id: &str,
        reason: Option<&str>,
        _commits: Vec<String>,
        actor: Option<&str>,
    ) -> Result<Issue> {
        let mut update = IssueUpdate::new().status(IssueStatus::Closed);
        if let Some(r) = reason {
            update = update.close_reason(r);
        }
        self.update(id, update, actor)
    }

    fn reopen(&self, id: &str, actor: Option<&str>) -> Result<Issue> {
        self.update(id, IssueUpdate::new().status(IssueStatus::Open), actor)
    }

    fn block(&self, id: &str, reason: Option<&str>, actor: Option<&str>) -> Result<Issue> {
        let mut update = IssueUpdate::new().status(IssueStatus::Blocked);
        if let Some(r) = reason {
            update.notes = Some(Some(r.to_string()));
        }
        self.update(id, update, actor)
    }

    fn unblock(&self, id: &str, actor: Option<&str>) -> Result<Issue> {
        self.update(id, IssueUpdate::new().status(IssueStatus::Open), actor)
    }

    fn is_ready(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let blocked: bool = conn.query_row(IS_BLOCKED, [id], |row| row.get(0))?;
        Ok(!blocked)
    }

    fn add_dependency(&self, issue_id: &str, dep: Dependency) -> Result<()> {
        // Check for cycle
        if self.has_cycle(issue_id, &dep.depends_on_id)? {
            return Err(TaskmasterError::cycle_detected(format!(
                "{} -> {}",
                issue_id, dep.depends_on_id
            )));
        }

        let conn = self.conn.lock().unwrap();
        conn.execute(
            INSERT_DEPENDENCY,
            params![issue_id, &dep.depends_on_id, dep.dep_type.as_str()],
        )?;

        self.record_event(
            &conn,
            issue_id,
            EventType::DependencyAdded,
            None,
            None,
            None,
            Some(&dep.depends_on_id),
        )?;

        Ok(())
    }

    fn remove_dependency(&self, issue_id: &str, dep_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(DELETE_DEPENDENCY, params![issue_id, dep_id])?;

        if rows == 0 {
            return Err(TaskmasterError::DependencyNotFound {
                issue_id: issue_id.to_string(),
                dep_id: dep_id.to_string(),
            });
        }

        self.record_event(
            &conn,
            issue_id,
            EventType::DependencyRemoved,
            None,
            None,
            Some(dep_id),
            None,
        )?;

        Ok(())
    }

    fn blockers(&self, issue_id: &str) -> Result<Vec<Issue>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT i.* FROM issues i
             JOIN issue_dependencies d ON i.id = d.depends_on_id
             WHERE d.issue_id = ?1
             AND d.dependency_type IN ('blocks', 'parent-child')",
        )?;

        // This is a simplified query - full implementation would need the proper column selection
        let ids: Vec<String> = stmt
            .query_map([issue_id], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        drop(stmt);
        drop(conn);

        ids.iter().map(|id| self.get(id)).collect()
    }

    fn blocked_by(&self, issue_id: &str) -> Result<Vec<Issue>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT issue_id FROM issue_dependencies
             WHERE depends_on_id = ?1
             AND dependency_type IN ('blocks', 'parent-child')",
        )?;

        let ids: Vec<String> = stmt
            .query_map([issue_id], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        drop(stmt);
        drop(conn);

        ids.iter().map(|id| self.get(id)).collect()
    }

    fn dependency_tree(&self, issue_id: &str, _max_depth: u32) -> Result<DependencyTree> {
        let issue = self.get(issue_id)?;

        // Simplified implementation - just return the root node
        Ok(DependencyTree {
            root: DependencyTreeNode {
                id: issue.id.clone(),
                title: issue.title.clone(),
                status: issue.status.as_str().to_string(),
                dep_type: None,
                depth: 0,
                truncated: false,
                children: Vec::new(),
            },
            total_nodes: 1,
            max_depth: 0,
            has_truncated: false,
        })
    }

    fn has_cycle(&self, issue_id: &str, dep_id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let has_cycle: bool = conn.query_row(DETECT_CYCLE, params![issue_id, dep_id], |row| {
            row.get(0)
        })?;
        Ok(has_cycle)
    }

    fn add_label(&self, issue_id: &str, label: &str, actor: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(INSERT_LABEL, params![issue_id, label])?;

        self.record_event(
            &conn,
            issue_id,
            EventType::LabelAdded,
            actor,
            None,
            None,
            Some(label),
        )?;

        Ok(())
    }

    fn remove_label(&self, issue_id: &str, label: &str, actor: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(DELETE_LABEL, params![issue_id, label])?;

        self.record_event(
            &conn,
            issue_id,
            EventType::LabelRemoved,
            actor,
            None,
            Some(label),
            None,
        )?;

        Ok(())
    }

    fn all_labels(&self) -> Result<Vec<LabelCount>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(GET_ALL_LABELS)?;
        let labels = stmt
            .query_map([], |row| {
                Ok(LabelCount {
                    label: row.get(0)?,
                    count: row.get::<_, i64>(1)? as usize,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(labels)
    }

    fn add_comment(&self, issue_id: &str, comment: CommentCreate) -> Result<Comment> {
        let conn = self.conn.lock().unwrap();

        let id = Uuid::new_v4().to_string();
        let now = Utc::now();

        conn.execute(
            INSERT_COMMENT,
            params![&id, issue_id, &comment.author, &comment.body],
        )?;

        self.record_event(&conn, issue_id, EventType::Commented, Some(&comment.author), None, None, None)?;

        Ok(Comment {
            id,
            issue_id: issue_id.to_string(),
            author: comment.author,
            body: comment.body,
            created_at: now,
            updated_at: None,
        })
    }

    fn comments(&self, issue_id: &str) -> Result<Vec<Comment>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(GET_COMMENTS)?;
        let comments = stmt
            .query_map([issue_id], |row| {
                Ok(Comment {
                    id: row.get("id")?,
                    issue_id: row.get("issue_id")?,
                    author: row.get("author")?,
                    body: row.get("body")?,
                    created_at: parse_datetime(row.get::<_, String>("created_at")?),
                    updated_at: row.get::<_, Option<String>>("updated_at")?.map(parse_datetime),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(comments)
    }

    fn events(&self, issue_id: &str, limit: Option<usize>) -> Result<Vec<IssueEvent>> {
        let conn = self.conn.lock().unwrap();
        let limit = limit.unwrap_or(100) as i64;
        let mut stmt = conn.prepare(GET_EVENTS)?;
        let events = stmt
            .query_map(params![issue_id, limit], |row| {
                let event_type_str: String = row.get("event_type")?;
                Ok(IssueEvent {
                    id: row.get("id")?,
                    issue_id: row.get("issue_id")?,
                    event_type: event_type_str.parse().unwrap_or(EventType::Updated),
                    actor: row.get("actor")?,
                    field_name: row.get("field_name")?,
                    old_value: row.get("old_value")?,
                    new_value: row.get("new_value")?,
                    metadata: row.get("metadata")?,
                    created_at: parse_datetime(row.get::<_, String>("created_at")?),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(events)
    }

    fn recent_events(&self, limit: usize) -> Result<Vec<IssueEvent>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, issue_id, event_type, actor, field_name, old_value, new_value, metadata, created_at
             FROM issue_events
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let events = stmt
            .query_map([limit as i64], |row| {
                let event_type_str: String = row.get("event_type")?;
                Ok(IssueEvent {
                    id: row.get("id")?,
                    issue_id: row.get("issue_id")?,
                    event_type: event_type_str.parse().unwrap_or(EventType::Updated),
                    actor: row.get("actor")?,
                    field_name: row.get("field_name")?,
                    old_value: row.get("old_value")?,
                    new_value: row.get("new_value")?,
                    metadata: row.get("metadata")?,
                    created_at: parse_datetime(row.get::<_, String>("created_at")?),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(events)
    }

    fn compact(&self, _older_than_days: u32) -> Result<CompactionResult> {
        // TODO: Implement compaction
        Ok(CompactionResult::default())
    }

    fn stats(&self) -> Result<IssueStats> {
        let mut stats = IssueStats::new();

        // Collect counts in a block to release the lock
        {
            let conn = self.conn.lock().unwrap();

            // Count by status
            {
                let mut stmt = conn.prepare(COUNT_BY_STATUS)?;
                let status_counts: Vec<(String, usize)> = stmt
                    .query_map([], |row| {
                        let status: String = row.get(0)?;
                        let count: i64 = row.get(1)?;
                        Ok((status, count as usize))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;

                for (status_str, count) in status_counts {
                    if let Ok(status) = status_str.parse::<IssueStatus>() {
                        stats.by_status.set(status, count);
                    }
                }
            }

            // Count by priority
            {
                let mut stmt = conn.prepare(COUNT_BY_PRIORITY)?;
                let priority_counts: Vec<(i32, usize)> = stmt
                    .query_map([], |row| {
                        let priority: i32 = row.get(0)?;
                        let count: i64 = row.get(1)?;
                        Ok((priority, count as usize))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;

                for (priority_val, count) in priority_counts {
                    if let Some(priority) = Priority::from_u8(priority_val as u8) {
                        stats.by_priority.set(priority, count);
                    }
                }
            }

            // Count by type
            {
                let mut stmt = conn.prepare(COUNT_BY_TYPE)?;
                let type_counts: Vec<(String, usize)> = stmt
                    .query_map([], |row| {
                        let issue_type: String = row.get(0)?;
                        let count: i64 = row.get(1)?;
                        Ok((issue_type, count as usize))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;

                for (type_str, count) in type_counts {
                    if let Ok(issue_type) = type_str.parse::<IssueType>() {
                        stats.by_type.set(issue_type, count);
                    }
                }
            }
        } // conn lock released here

        // Calculate totals
        stats.total_issues = stats.by_status.open
            + stats.by_status.in_progress
            + stats.by_status.blocked
            + stats.by_status.closed;
        stats.tombstone_issues = stats.by_status.tombstone;

        // Count ready issues (this acquires its own lock)
        stats.ready_issues = self.ready(IssueFilter::default())?.len();

        Ok(stats)
    }

    fn stats_history(&self, _days: u32) -> Result<Vec<StatsSnapshot>> {
        // TODO: Implement stats history
        Ok(Vec::new())
    }

    fn save_stats_snapshot(&self) -> Result<StatsSnapshot> {
        let stats = self.stats()?;
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO stats_snapshots (snapshot_date, total_issues, open_count, in_progress_count, blocked_count, closed_count, tombstone_count)
             VALUES (datetime('now'), ?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                stats.total_issues as i64,
                stats.by_status.open as i64,
                stats.by_status.in_progress as i64,
                stats.by_status.blocked as i64,
                stats.by_status.closed as i64,
                stats.tombstone_issues as i64,
            ],
        )?;

        let id = conn.last_insert_rowid();

        Ok(StatsSnapshot {
            id,
            snapshot_date: Utc::now(),
            total_issues: stats.total_issues,
            open_count: stats.by_status.open,
            in_progress_count: stats.by_status.in_progress,
            blocked_count: stats.by_status.blocked,
            closed_count: stats.by_status.closed,
            tombstone_count: stats.tombstone_issues,
            avg_time_to_close_hours: stats.avg_time_to_close_hours,
            labels_json: None,
            priority_json: None,
            created_at: Utc::now(),
        })
    }

    fn doctor(&self) -> Result<DoctorReport> {
        let mut report = DoctorReport::new();
        let conn = self.conn.lock().unwrap();

        // Check for orphan dependencies
        let mut stmt = conn.prepare(
            "SELECT d.issue_id, d.depends_on_id
             FROM issue_dependencies d
             LEFT JOIN issues i ON d.depends_on_id = i.id
             WHERE i.id IS NULL",
        )?;

        let orphans: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        for (issue_id, dep_id) in orphans {
            report.add_problem(DoctorProblem {
                id: format!("orphan-dep-{}-{}", issue_id, dep_id),
                issue_id: Some(issue_id),
                category: DoctorCategory::OrphanDependency,
                description: format!("Dependency references non-existent issue: {}", dep_id),
                severity: DoctorSeverity::Warning,
                repairable: true,
            });
        }

        // Count issues checked
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM issues", [], |row| row.get(0))?;
        report.issues_checked = count as usize;

        let dep_count: i64 = conn.query_row("SELECT COUNT(*) FROM issue_dependencies", [], |row| row.get(0))?;
        report.dependencies_checked = dep_count as usize;

        Ok(report)
    }

    fn repair(&self, problems: &[DoctorProblem]) -> Result<RepairReport> {
        let mut report = RepairReport::new();
        let conn = self.conn.lock().unwrap();

        for problem in problems {
            if !problem.repairable {
                continue;
            }

            match problem.category {
                DoctorCategory::OrphanDependency => {
                    if let Some(issue_id) = &problem.issue_id {
                        // Delete orphan dependency
                        conn.execute(
                            "DELETE FROM issue_dependencies WHERE issue_id = ?1 AND depends_on_id NOT IN (SELECT id FROM issues)",
                            [issue_id],
                        )?;
                        report.repaired.push(problem.id.clone());
                    }
                }
                _ => {}
            }
        }

        Ok(report)
    }

    fn migrate(&self) -> Result<MigrationResult> {
        let conn = self.conn.lock().unwrap();

        // Get current schema version
        let current_version: Option<u32> = conn
            .query_row(CHECK_VERSION, [], |row| row.get(0))
            .optional()?
            .flatten();

        let mut applied = Vec::new();

        match current_version {
            None | Some(0) => {
                // Fresh database, apply V1
                conn.execute_batch(SCHEMA_V1)?;
                applied.push("v1_initial".to_string());
                // Fall through to apply V2
                conn.execute_batch(SCHEMA_V2)?;
                applied.push("v2_execution_context".to_string());
            }
            Some(1) => {
                // V1 exists, apply V2 migration
                conn.execute_batch(SCHEMA_V2)?;
                applied.push("v2_execution_context".to_string());
            }
            Some(v) if v >= SCHEMA_VERSION => {
                // Already up to date
            }
            Some(v) => {
                // Unknown version
                return Err(TaskmasterError::migration(format!(
                    "Unknown schema version: {}",
                    v
                )));
            }
        }

        Ok(MigrationResult {
            applied,
            current_version: SCHEMA_VERSION,
            success: true,
        })
    }

    fn vacuum(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("VACUUM")?;
        Ok(())
    }

    fn cleanup_tombstones(&self) -> Result<CleanupResult> {
        let conn = self.conn.lock().unwrap();

        // Get expired tombstones
        let mut stmt = conn.prepare(EXPIRED_TOMBSTONES)?;
        let expired_ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let purged_count = expired_ids.len();

        // Delete expired tombstones
        for id in &expired_ids {
            conn.execute("DELETE FROM issues WHERE id = ?1", [id])?;
        }

        // Count remaining tombstones
        let retained_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM issues WHERE status = 'tombstone'",
            [],
            |row| row.get(0),
        )?;

        Ok(CleanupResult {
            purged_count,
            retained_count: retained_count as usize,
            errors: Vec::new(),
        })
    }
}

// Helper functions

fn format_datetime(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn parse_datetime(s: String) -> DateTime<Utc> {
    chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S")
        .map(|ndt| DateTime::from_naive_utc_and_offset(ndt, Utc))
        .unwrap_or_else(|_| Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get() {
        let repo = SqliteRepository::in_memory().unwrap();

        let create = IssueCreate::new("Test Issue")
            .description("A test description")
            .priority(Priority::High)
            .label("test");

        let issue = repo.create(create, "tm").unwrap();
        assert!(issue.id.starts_with("tm-"));
        assert_eq!(issue.title, "Test Issue");
        assert_eq!(issue.priority, Priority::High);
        assert_eq!(issue.labels, vec!["test"]);

        let fetched = repo.get(&issue.id).unwrap();
        assert_eq!(fetched.id, issue.id);
        assert_eq!(fetched.title, issue.title);
    }

    #[test]
    fn test_update() {
        let repo = SqliteRepository::in_memory().unwrap();

        let create = IssueCreate::new("Original Title");
        let issue = repo.create(create, "tm").unwrap();

        let update = IssueUpdate::new()
            .title("Updated Title")
            .priority(Priority::Critical);

        let updated = repo.update(&issue.id, update, Some("alice")).unwrap();
        assert_eq!(updated.title, "Updated Title");
        assert_eq!(updated.priority, Priority::Critical);
    }

    #[test]
    fn test_status_lifecycle() {
        let repo = SqliteRepository::in_memory().unwrap();

        let create = IssueCreate::new("Lifecycle Test");
        let issue = repo.create(create, "tm").unwrap();
        assert_eq!(issue.status, IssueStatus::Open);

        let started = repo.start(&issue.id, None).unwrap();
        assert_eq!(started.status, IssueStatus::InProgress);

        let closed = repo.close(&issue.id, Some("Done"), vec![], None).unwrap();
        assert_eq!(closed.status, IssueStatus::Closed);
        assert!(closed.closed_at.is_some());

        let reopened = repo.reopen(&issue.id, None).unwrap();
        assert_eq!(reopened.status, IssueStatus::Open);
        assert!(reopened.closed_at.is_none());
    }

    #[test]
    fn test_tombstone() {
        let repo = SqliteRepository::in_memory().unwrap();

        let create = IssueCreate::new("To Delete");
        let issue = repo.create(create, "tm").unwrap();

        repo.tombstone(&issue.id, Some("Test deletion"), None).unwrap();

        // Should not find with regular get
        assert!(repo.get(&issue.id).is_err());

        // Should find with tombstones
        let tombstoned = repo.get_with_tombstones(&issue.id).unwrap();
        assert_eq!(tombstoned.status, IssueStatus::Tombstone);

        // Restore
        let restored = repo.restore(&issue.id, None).unwrap();
        assert_eq!(restored.status, IssueStatus::Open);
    }

    #[test]
    fn test_dependencies() {
        let repo = SqliteRepository::in_memory().unwrap();

        let issue1 = repo.create(IssueCreate::new("Issue 1"), "tm").unwrap();
        let issue2 = repo.create(IssueCreate::new("Issue 2"), "tm").unwrap();

        // Add dependency: issue2 blocks issue1
        let dep = Dependency::new(&issue1.id, &issue2.id, DependencyType::Blocks);
        repo.add_dependency(&issue1.id, dep).unwrap();

        // Issue1 should not be ready (blocked by issue2)
        assert!(!repo.is_ready(&issue1.id).unwrap());

        // Issue2 should be ready
        assert!(repo.is_ready(&issue2.id).unwrap());

        // Close issue2
        repo.close(&issue2.id, None, vec![], None).unwrap();

        // Now issue1 should be ready
        assert!(repo.is_ready(&issue1.id).unwrap());
    }

    #[test]
    fn test_labels() {
        let repo = SqliteRepository::in_memory().unwrap();

        let issue = repo.create(IssueCreate::new("Label Test"), "tm").unwrap();

        repo.add_label(&issue.id, "urgent", None).unwrap();
        repo.add_label(&issue.id, "backend", None).unwrap();

        let fetched = repo.get(&issue.id).unwrap();
        assert!(fetched.labels.contains(&"urgent".to_string()));
        assert!(fetched.labels.contains(&"backend".to_string()));

        let all_labels = repo.all_labels().unwrap();
        assert!(all_labels.iter().any(|l| l.label == "urgent"));
    }

    #[test]
    fn test_comments() {
        let repo = SqliteRepository::in_memory().unwrap();

        let issue = repo.create(IssueCreate::new("Comment Test"), "tm").unwrap();

        let comment = repo
            .add_comment(&issue.id, CommentCreate::new("alice", "Test comment"))
            .unwrap();

        assert_eq!(comment.author, "alice");
        assert_eq!(comment.body, "Test comment");

        let comments = repo.comments(&issue.id).unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].body, "Test comment");
    }

    #[test]
    fn test_stats() {
        let repo = SqliteRepository::in_memory().unwrap();

        repo.create(IssueCreate::new("Issue 1").priority(Priority::High), "tm")
            .unwrap();
        repo.create(IssueCreate::new("Issue 2").priority(Priority::Medium), "tm")
            .unwrap();

        let stats = repo.stats().unwrap();
        assert_eq!(stats.total_issues, 2);
        assert_eq!(stats.by_status.open, 2);
        assert_eq!(stats.by_priority.high, 1);
        assert_eq!(stats.by_priority.medium, 1);
    }
}
