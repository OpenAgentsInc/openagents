//! SQLite implementation of TaskRepository

use crate::{
    repository::{TaskError, TaskRepository, TaskResult},
    types::*,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::sync::Mutex;
use uuid::Uuid;

/// SQLite-based task repository
pub struct SqliteRepository {
    conn: Mutex<Connection>,
}

impl SqliteRepository {
    /// Open a SQLite database
    pub fn open(path: &str) -> TaskResult<Self> {
        let conn = Connection::open(path).map_err(|e| TaskError::DatabaseError(e.to_string()))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open an in-memory database (for testing)
    pub fn in_memory() -> TaskResult<Self> {
        Self::open(":memory:")
    }

    /// Generate a task ID
    fn generate_id(task: &TaskCreate, method: IdMethod, prefix: &str) -> String {
        match method {
            IdMethod::Hash => {
                let mut hasher = Sha256::new();
                hasher.update(&task.title);
                if let Some(ref desc) = task.description {
                    hasher.update(desc);
                }
                let hash = hasher.finalize();
                let short = hex::encode(&hash[..6]);
                format!("{}-{}", prefix, short)
            }
            IdMethod::Random => {
                let uuid = Uuid::new_v4();
                let short = &uuid.to_string()[..8];
                format!("{}-{}", prefix, short)
            }
        }
    }

    /// Convert a row to a Task
    fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<Task> {
        let status_str: String = row.get("status")?;
        let priority_val: i32 = row.get("priority")?;
        let type_str: String = row.get("task_type")?;
        let labels_json: String = row.get("labels")?;
        let deps_json: String = row.get("deps")?;
        let commits_json: String = row.get("commits")?;
        let comments_json: String = row.get("comments")?;
        let source_json: Option<String> = row.get("source")?;
        let pending_commit_json: Option<String> = row.get("pending_commit")?;
        let created_at_str: String = row.get("created_at")?;
        let updated_at_str: String = row.get("updated_at")?;
        let closed_at_str: Option<String> = row.get("closed_at")?;

        Ok(Task {
            id: row.get("id")?,
            title: row.get("title")?,
            description: row.get::<_, Option<String>>("description")?.unwrap_or_default(),
            status: TaskStatus::from_str(&status_str).unwrap_or_default(),
            priority: TaskPriority::from_u8(priority_val as u8).unwrap_or_default(),
            task_type: TaskType::from_str(&type_str).unwrap_or_default(),
            assignee: row.get("assignee")?,
            labels: serde_json::from_str(&labels_json).unwrap_or_default(),
            deps: serde_json::from_str(&deps_json).unwrap_or_default(),
            commits: serde_json::from_str(&commits_json).unwrap_or_default(),
            comments: serde_json::from_str(&comments_json).unwrap_or_default(),
            created_at: DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            closed_at: closed_at_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            }),
            close_reason: row.get("close_reason")?,
            source: source_json.and_then(|s| serde_json::from_str(&s).ok()),
            design: row.get("design")?,
            acceptance_criteria: row.get("acceptance_criteria")?,
            notes: row.get("notes")?,
            estimated_minutes: row.get("estimated_minutes")?,
            pending_commit: pending_commit_json.and_then(|s| serde_json::from_str(&s).ok()),
        })
    }

    /// Build ORDER BY clause from sort policy
    fn order_by_clause(sort: SortPolicy) -> &'static str {
        match sort {
            SortPolicy::Hybrid => "priority ASC, created_at ASC",
            SortPolicy::Priority => "priority ASC, created_at DESC",
            SortPolicy::Oldest => "created_at ASC",
            SortPolicy::Newest => "created_at DESC",
        }
    }
}

impl TaskRepository for SqliteRepository {
    fn init(&self) -> TaskResult<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            r#"
            -- Main tasks table
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                status TEXT NOT NULL DEFAULT 'open',
                priority INTEGER NOT NULL DEFAULT 2,
                task_type TEXT NOT NULL DEFAULT 'task',
                assignee TEXT,
                labels TEXT NOT NULL DEFAULT '[]',
                deps TEXT NOT NULL DEFAULT '[]',
                commits TEXT NOT NULL DEFAULT '[]',
                comments TEXT NOT NULL DEFAULT '[]',
                source TEXT,
                design TEXT,
                acceptance_criteria TEXT,
                notes TEXT,
                estimated_minutes INTEGER,
                pending_commit TEXT,
                close_reason TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                closed_at TEXT,
                deleted_at TEXT
            );

            -- Dependencies table (for efficient blocking queries)
            CREATE TABLE IF NOT EXISTS task_dependencies (
                task_id TEXT NOT NULL,
                depends_on_task_id TEXT NOT NULL,
                dependency_type TEXT NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY (task_id, depends_on_task_id)
            );

            -- Deletion tombstones
            CREATE TABLE IF NOT EXISTS task_deletions (
                task_id TEXT PRIMARY KEY,
                deleted_at TEXT NOT NULL,
                deleted_by TEXT,
                reason TEXT
            );

            -- Index for ready task queries
            CREATE INDEX IF NOT EXISTS idx_tasks_ready
                ON tasks(status, priority, created_at)
                WHERE deleted_at IS NULL;

            -- Index for dependencies
            CREATE INDEX IF NOT EXISTS idx_deps_task
                ON task_dependencies(task_id);
            CREATE INDEX IF NOT EXISTS idx_deps_depends_on
                ON task_dependencies(depends_on_task_id);

            -- Full-text search
            CREATE VIRTUAL TABLE IF NOT EXISTS tasks_fts USING fts5(
                id, title, description,
                content='tasks',
                content_rowid='rowid'
            );

            -- Triggers to keep FTS in sync
            CREATE TRIGGER IF NOT EXISTS tasks_ai AFTER INSERT ON tasks BEGIN
                INSERT INTO tasks_fts(rowid, id, title, description)
                VALUES (new.rowid, new.id, new.title, new.description);
            END;

            CREATE TRIGGER IF NOT EXISTS tasks_ad AFTER DELETE ON tasks BEGIN
                INSERT INTO tasks_fts(tasks_fts, rowid, id, title, description)
                VALUES('delete', old.rowid, old.id, old.title, old.description);
            END;

            CREATE TRIGGER IF NOT EXISTS tasks_au AFTER UPDATE ON tasks BEGIN
                INSERT INTO tasks_fts(tasks_fts, rowid, id, title, description)
                VALUES('delete', old.rowid, old.id, old.title, old.description);
                INSERT INTO tasks_fts(rowid, id, title, description)
                VALUES (new.rowid, new.id, new.title, new.description);
            END;
            "#,
        )
        .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    fn create(&self, task: TaskCreate) -> TaskResult<Task> {
        self.create_with_id_method(task, IdMethod::Random, "oa")
    }

    fn create_with_id_method(
        &self,
        task: TaskCreate,
        method: IdMethod,
        prefix: &str,
    ) -> TaskResult<Task> {
        // Validate
        if task.title.is_empty() {
            return Err(TaskError::ValidationError("Title cannot be empty".into()));
        }
        if task.title.len() > 500 {
            return Err(TaskError::ValidationError(
                "Title cannot exceed 500 characters".into(),
            ));
        }

        let id = Self::generate_id(&task, method, prefix);
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        let conn = self.conn.lock().unwrap();

        // Check for existing task with same ID (for hash-based dedup)
        let exists: bool = conn
            .query_row("SELECT 1 FROM tasks WHERE id = ?1", params![id], |_| Ok(true))
            .optional()
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?
            .unwrap_or(false);

        if exists {
            return Err(TaskError::AlreadyExists(id));
        }

        let labels_json = serde_json::to_string(&task.labels).unwrap_or_else(|_| "[]".into());
        let deps_json = serde_json::to_string(&task.deps).unwrap_or_else(|_| "[]".into());
        let source_json = task
            .source
            .as_ref()
            .map(|s| serde_json::to_string(s).ok())
            .flatten();

        conn.execute(
            r#"
            INSERT INTO tasks (
                id, title, description, status, priority, task_type, assignee,
                labels, deps, commits, comments, source, design, acceptance_criteria,
                notes, estimated_minutes, created_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '[]', '[]', ?10, ?11, ?12, ?13, ?14, ?15, ?16
            )
            "#,
            params![
                id,
                task.title,
                task.description,
                TaskStatus::Open.as_str(),
                task.priority.as_u8(),
                task.task_type.as_str(),
                task.assignee,
                labels_json,
                deps_json,
                source_json,
                task.design,
                task.acceptance_criteria,
                task.notes,
                task.estimated_minutes,
                now_str,
                now_str,
            ],
        )
        .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        // Insert dependencies into the dependencies table
        for dep in &task.deps {
            conn.execute(
                "INSERT INTO task_dependencies (task_id, depends_on_task_id, dependency_type, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![id, dep.id, dep.dep_type.as_str(), now_str],
            )
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;
        }

        self.get(&id)
    }

    fn get(&self, id: &str) -> TaskResult<Task> {
        let conn = self.conn.lock().unwrap();

        conn.query_row(
            "SELECT * FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
            Self::row_to_task,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => TaskError::NotFound(id.to_string()),
            _ => TaskError::DatabaseError(e.to_string()),
        })
    }

    fn exists(&self, id: &str) -> TaskResult<bool> {
        let conn = self.conn.lock().unwrap();

        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |_| Ok(true),
            )
            .optional()
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?
            .unwrap_or(false);

        Ok(exists)
    }

    fn update(&self, id: &str, update: TaskUpdate) -> TaskResult<Task> {
        let task = self.get(id)?;
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        // Update field by field for simplicity and type safety
        let conn = self.conn.lock().unwrap();

        if let Some(ref title) = update.title {
            conn.execute(
                "UPDATE tasks SET title = ?1, updated_at = ?2 WHERE id = ?3",
                params![title, now_str, id],
            )
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;
        }

        if let Some(ref desc) = update.description {
            conn.execute(
                "UPDATE tasks SET description = ?1, updated_at = ?2 WHERE id = ?3",
                params![desc, now_str, id],
            )
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;
        }

        if let Some(status) = update.status {
            let closed_at = if status == TaskStatus::Closed {
                Some(now_str.clone())
            } else {
                None
            };
            conn.execute(
                "UPDATE tasks SET status = ?1, closed_at = ?2, updated_at = ?3 WHERE id = ?4",
                params![status.as_str(), closed_at, now_str, id],
            )
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;
        }

        if let Some(priority) = update.priority {
            conn.execute(
                "UPDATE tasks SET priority = ?1, updated_at = ?2 WHERE id = ?3",
                params![priority.as_u8(), now_str, id],
            )
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;
        }

        if let Some(task_type) = update.task_type {
            conn.execute(
                "UPDATE tasks SET task_type = ?1, updated_at = ?2 WHERE id = ?3",
                params![task_type.as_str(), now_str, id],
            )
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;
        }

        if let Some(ref assignee) = update.assignee {
            conn.execute(
                "UPDATE tasks SET assignee = ?1, updated_at = ?2 WHERE id = ?3",
                params![assignee, now_str, id],
            )
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;
        }

        if let Some(ref labels) = update.labels {
            let json = serde_json::to_string(labels).unwrap_or_else(|_| "[]".into());
            conn.execute(
                "UPDATE tasks SET labels = ?1, updated_at = ?2 WHERE id = ?3",
                params![json, now_str, id],
            )
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;
        }

        if let Some(ref deps) = update.deps {
            let json = serde_json::to_string(deps).unwrap_or_else(|_| "[]".into());
            conn.execute(
                "UPDATE tasks SET deps = ?1, updated_at = ?2 WHERE id = ?3",
                params![json, now_str, id],
            )
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

            // Update dependencies table
            conn.execute(
                "DELETE FROM task_dependencies WHERE task_id = ?1",
                params![id],
            )
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

            for dep in deps {
                conn.execute(
                    "INSERT INTO task_dependencies (task_id, depends_on_task_id, dependency_type, created_at) VALUES (?1, ?2, ?3, ?4)",
                    params![id, dep.id, dep.dep_type.as_str(), now_str],
                )
                .map_err(|e| TaskError::DatabaseError(e.to_string()))?;
            }
        }

        if let Some(ref add_commits) = update.add_commits {
            // Use the task we fetched at the start
            let mut commits = task.commits.clone();
            commits.extend(add_commits.iter().cloned());
            let json = serde_json::to_string(&commits).unwrap_or_else(|_| "[]".into());
            conn.execute(
                "UPDATE tasks SET commits = ?1, updated_at = ?2 WHERE id = ?3",
                params![json, now_str, id],
            )
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;
        }

        if let Some(ref close_reason) = update.close_reason {
            conn.execute(
                "UPDATE tasks SET close_reason = ?1, updated_at = ?2 WHERE id = ?3",
                params![close_reason, now_str, id],
            )
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;
        }

        if let Some(ref design) = update.design {
            conn.execute(
                "UPDATE tasks SET design = ?1, updated_at = ?2 WHERE id = ?3",
                params![design, now_str, id],
            )
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;
        }

        if let Some(ref ac) = update.acceptance_criteria {
            conn.execute(
                "UPDATE tasks SET acceptance_criteria = ?1, updated_at = ?2 WHERE id = ?3",
                params![ac, now_str, id],
            )
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;
        }

        if let Some(ref notes) = update.notes {
            conn.execute(
                "UPDATE tasks SET notes = ?1, updated_at = ?2 WHERE id = ?3",
                params![notes, now_str, id],
            )
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;
        }

        if let Some(ref est) = update.estimated_minutes {
            conn.execute(
                "UPDATE tasks SET estimated_minutes = ?1, updated_at = ?2 WHERE id = ?3",
                params![est, now_str, id],
            )
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;
        }

        if let Some(ref pc) = update.pending_commit {
            let json = pc.as_ref().map(|p| serde_json::to_string(p).ok()).flatten();
            conn.execute(
                "UPDATE tasks SET pending_commit = ?1, updated_at = ?2 WHERE id = ?3",
                params![json, now_str, id],
            )
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;
        }

        drop(conn);
        self.get(id)
    }

    fn delete(&self, id: &str, reason: Option<&str>) -> TaskResult<()> {
        let _ = self.get(id)?; // Verify exists
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "UPDATE tasks SET deleted_at = ?1 WHERE id = ?2",
            params![now, id],
        )
        .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        conn.execute(
            "INSERT INTO task_deletions (task_id, deleted_at, reason) VALUES (?1, ?2, ?3)",
            params![id, now, reason],
        )
        .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    fn list(&self, filter: TaskFilter) -> TaskResult<Vec<Task>> {
        let conn = self.conn.lock().unwrap();

        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = filter.status {
            conditions.push(format!("status = ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(status.as_str().to_string()));
        }

        if let Some(priority) = filter.priority {
            conditions.push(format!("priority = ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(priority.as_u8() as i32));
        }

        if let Some(task_type) = filter.task_type {
            conditions.push(format!("task_type = ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(task_type.as_str().to_string()));
        }

        if let Some(ref assignee) = filter.assignee {
            conditions.push(format!("assignee = ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(assignee.clone()));
        }

        let order = Self::order_by_clause(filter.sort);
        let limit_clause = filter
            .limit
            .map(|l| format!(" LIMIT {}", l))
            .unwrap_or_default();

        let sql = format!(
            "SELECT * FROM tasks WHERE {} ORDER BY {}{}",
            conditions.join(" AND "),
            order,
            limit_clause
        );

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        let tasks = stmt
            .query_map(params_refs.as_slice(), Self::row_to_task)
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        // Filter by labels if specified
        let tasks = if let Some(ref labels) = filter.labels {
            tasks
                .into_iter()
                .filter(|t| labels.iter().any(|l| t.labels.contains(l)))
                .collect()
        } else {
            tasks
        };

        Ok(tasks)
    }

    fn count(&self, filter: TaskFilter) -> TaskResult<usize> {
        Ok(self.list(filter)?.len())
    }

    fn search(&self, query: &str, filter: TaskFilter) -> TaskResult<Vec<Task>> {
        let conn = self.conn.lock().unwrap();

        // Use FTS5 for search
        let sql = r#"
            SELECT t.* FROM tasks t
            JOIN tasks_fts fts ON t.id = fts.id
            WHERE tasks_fts MATCH ?1
            AND t.deleted_at IS NULL
            ORDER BY rank
        "#;

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        let tasks = stmt
            .query_map(params![query], Self::row_to_task)
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        // Apply additional filters
        let tasks = tasks
            .into_iter()
            .filter(|t| {
                filter
                    .status
                    .map(|s| t.status == s)
                    .unwrap_or(true)
                    && filter
                        .priority
                        .map(|p| t.priority == p)
                        .unwrap_or(true)
                    && filter
                        .task_type
                        .map(|ty| t.task_type == ty)
                        .unwrap_or(true)
                    && filter
                        .assignee
                        .as_ref()
                        .map(|a| t.assignee.as_ref() == Some(a))
                        .unwrap_or(true)
            })
            .collect();

        Ok(tasks)
    }

    fn ready_tasks(&self, filter: TaskFilter) -> TaskResult<Vec<Task>> {
        let conn = self.conn.lock().unwrap();
        let order = Self::order_by_clause(filter.sort);
        let limit_clause = filter
            .limit
            .map(|l| format!(" LIMIT {}", l))
            .unwrap_or_default();

        // Core ready task query: open, not deleted, no blocking deps
        let sql = format!(
            r#"
            SELECT t.* FROM tasks t
            WHERE t.status = 'open'
            AND t.deleted_at IS NULL
            AND NOT EXISTS (
                SELECT 1 FROM task_dependencies td
                JOIN tasks blocker ON td.depends_on_task_id = blocker.id
                WHERE td.task_id = t.id
                AND td.dependency_type IN ('blocks', 'parent-child')
                AND blocker.status IN ('open', 'in_progress')
                AND blocker.deleted_at IS NULL
            )
            ORDER BY {}
            {}
            "#,
            order, limit_clause
        );

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        let tasks = stmt
            .query_map([], Self::row_to_task)
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        // Apply additional filters
        let tasks = tasks
            .into_iter()
            .filter(|t| {
                filter
                    .priority
                    .map(|p| t.priority == p)
                    .unwrap_or(true)
                    && filter
                        .task_type
                        .map(|ty| t.task_type == ty)
                        .unwrap_or(true)
                    && filter
                        .assignee
                        .as_ref()
                        .map(|a| t.assignee.as_ref() == Some(a))
                        .unwrap_or(true)
                    && filter
                        .labels
                        .as_ref()
                        .map(|labels| labels.iter().any(|l| t.labels.contains(l)))
                        .unwrap_or(true)
            })
            .collect();

        Ok(tasks)
    }

    fn is_ready(&self, id: &str) -> TaskResult<bool> {
        let task = self.get(id)?;

        if task.status != TaskStatus::Open {
            return Ok(false);
        }

        let conn = self.conn.lock().unwrap();

        // Check for blocking dependencies
        let has_blockers: bool = conn
            .query_row(
                r#"
                SELECT 1 FROM task_dependencies td
                JOIN tasks blocker ON td.depends_on_task_id = blocker.id
                WHERE td.task_id = ?1
                AND td.dependency_type IN ('blocks', 'parent-child')
                AND blocker.status IN ('open', 'in_progress')
                AND blocker.deleted_at IS NULL
                LIMIT 1
                "#,
                params![id],
                |_| Ok(true),
            )
            .optional()
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?
            .unwrap_or(false);

        Ok(!has_blockers)
    }

    fn close(&self, id: &str, reason: Option<&str>, commits: Vec<String>) -> TaskResult<Task> {
        let mut update = TaskUpdate::default();
        update.status = Some(TaskStatus::Closed);
        update.close_reason = Some(reason.map(String::from));
        if !commits.is_empty() {
            update.add_commits = Some(commits);
        }
        self.update(id, update)
    }

    fn reopen(&self, id: &str) -> TaskResult<Task> {
        let task = self.get(id)?;
        if task.status != TaskStatus::Closed {
            return Err(TaskError::InvalidStateTransition {
                from: task.status,
                to: TaskStatus::Open,
            });
        }

        let mut update = TaskUpdate::default();
        update.status = Some(TaskStatus::Open);
        self.update(id, update)
    }

    fn start(&self, id: &str) -> TaskResult<Task> {
        let task = self.get(id)?;
        if task.status != TaskStatus::Open {
            return Err(TaskError::InvalidStateTransition {
                from: task.status,
                to: TaskStatus::InProgress,
            });
        }

        let mut update = TaskUpdate::default();
        update.status = Some(TaskStatus::InProgress);
        self.update(id, update)
    }

    fn block(&self, id: &str, reason: Option<&str>) -> TaskResult<Task> {
        let mut update = TaskUpdate::default();
        update.status = Some(TaskStatus::Blocked);
        if let Some(r) = reason {
            update.notes = Some(Some(r.to_string()));
        }
        self.update(id, update)
    }

    fn unblock(&self, id: &str) -> TaskResult<Task> {
        let task = self.get(id)?;
        if task.status != TaskStatus::Blocked {
            return Err(TaskError::InvalidStateTransition {
                from: task.status,
                to: TaskStatus::Open,
            });
        }

        let mut update = TaskUpdate::default();
        update.status = Some(TaskStatus::Open);
        self.update(id, update)
    }

    fn add_dependency(&self, task_id: &str, dep: Dependency) -> TaskResult<()> {
        // Check for cycle
        if self.has_cycle(task_id, &dep.id)? {
            return Err(TaskError::CycleDetected(format!(
                "{} -> {}",
                task_id, dep.id
            )));
        }

        let task = self.get(task_id)?;
        let mut deps = task.deps;
        if !deps.iter().any(|d| d.id == dep.id) {
            deps.push(dep.clone());
        }

        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();

        let json = serde_json::to_string(&deps).unwrap_or_else(|_| "[]".into());
        conn.execute(
            "UPDATE tasks SET deps = ?1, updated_at = ?2 WHERE id = ?3",
            params![json, now, task_id],
        )
        .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        conn.execute(
            "INSERT OR REPLACE INTO task_dependencies (task_id, depends_on_task_id, dependency_type, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![task_id, dep.id, dep.dep_type.as_str(), now],
        )
        .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    fn remove_dependency(&self, task_id: &str, dep_id: &str) -> TaskResult<()> {
        let task = self.get(task_id)?;
        let deps: Vec<Dependency> = task.deps.into_iter().filter(|d| d.id != dep_id).collect();

        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();

        let json = serde_json::to_string(&deps).unwrap_or_else(|_| "[]".into());
        conn.execute(
            "UPDATE tasks SET deps = ?1, updated_at = ?2 WHERE id = ?3",
            params![json, now, task_id],
        )
        .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        conn.execute(
            "DELETE FROM task_dependencies WHERE task_id = ?1 AND depends_on_task_id = ?2",
            params![task_id, dep_id],
        )
        .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    fn blockers(&self, task_id: &str) -> TaskResult<Vec<Task>> {
        let conn = self.conn.lock().unwrap();

        let sql = r#"
            SELECT t.* FROM tasks t
            JOIN task_dependencies td ON t.id = td.depends_on_task_id
            WHERE td.task_id = ?1
            AND td.dependency_type IN ('blocks', 'parent-child')
            AND t.deleted_at IS NULL
        "#;

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        let tasks = stmt
            .query_map(params![task_id], Self::row_to_task)
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        Ok(tasks)
    }

    fn blocked_by(&self, task_id: &str) -> TaskResult<Vec<Task>> {
        let conn = self.conn.lock().unwrap();

        let sql = r#"
            SELECT t.* FROM tasks t
            JOIN task_dependencies td ON t.id = td.task_id
            WHERE td.depends_on_task_id = ?1
            AND td.dependency_type IN ('blocks', 'parent-child')
            AND t.deleted_at IS NULL
        "#;

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        let tasks = stmt
            .query_map(params![task_id], Self::row_to_task)
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        Ok(tasks)
    }

    fn has_cycle(&self, task_id: &str, dep_id: &str) -> TaskResult<bool> {
        // DFS to detect cycle: would adding task_id -> dep_id create a cycle?
        // A cycle exists if dep_id can reach task_id through blocking deps

        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![dep_id.to_string()];

        while let Some(current) = stack.pop() {
            if current == task_id {
                return Ok(true);
            }

            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());

            // Get blocking deps of current
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn
                .prepare(
                    r#"
                    SELECT depends_on_task_id FROM task_dependencies
                    WHERE task_id = ?1
                    AND dependency_type IN ('blocks', 'parent-child')
                    "#,
                )
                .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

            let deps: Vec<String> = stmt
                .query_map(params![current], |row| row.get(0))
                .map_err(|e| TaskError::DatabaseError(e.to_string()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

            drop(stmt);
            drop(conn);

            for dep in deps {
                if !visited.contains(&dep) {
                    stack.push(dep);
                }
            }
        }

        Ok(false)
    }

    fn add_comment(&self, task_id: &str, comment: Comment) -> TaskResult<()> {
        let task = self.get(task_id)?;
        let mut comments = task.comments;
        comments.push(comment);

        let now = Utc::now().to_rfc3339();
        let json = serde_json::to_string(&comments).unwrap_or_else(|_| "[]".into());

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE tasks SET comments = ?1, updated_at = ?2 WHERE id = ?3",
            params![json, now, task_id],
        )
        .map_err(|e| TaskError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    fn comments(&self, task_id: &str) -> TaskResult<Vec<Comment>> {
        let task = self.get(task_id)?;
        Ok(task.comments)
    }

    fn pending_commits(&self) -> TaskResult<Vec<Task>> {
        let mut filter = TaskFilter::default();
        filter.status = Some(TaskStatus::CommitPending);
        self.list(filter)
    }

    fn complete_pending_commit(&self, id: &str, sha: &str) -> TaskResult<Task> {
        let task = self.get(id)?;

        if task.status != TaskStatus::CommitPending {
            return Err(TaskError::InvalidStateTransition {
                from: task.status,
                to: TaskStatus::Closed,
            });
        }

        let mut update = TaskUpdate::default();
        update.status = Some(TaskStatus::Closed);

        // Update pending commit with SHA
        if let Some(mut pc) = task.pending_commit {
            pc.sha = Some(sha.to_string());
            update.pending_commit = Some(Some(pc));
        }

        update.add_commits = Some(vec![sha.to_string()]);
        self.update(id, update)
    }

    fn vacuum(&self) -> TaskResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("VACUUM", [])
            .map_err(|e| TaskError::DatabaseError(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> SqliteRepository {
        let repo = SqliteRepository::in_memory().unwrap();
        repo.init().unwrap();
        repo
    }

    #[test]
    fn test_create_and_get() {
        let repo = setup();

        let task = TaskCreate {
            title: "Test task".into(),
            description: Some("A test description".into()),
            priority: TaskPriority::High,
            task_type: TaskType::Feature,
            ..Default::default()
        };

        let created = repo.create(task).unwrap();
        assert!(created.id.starts_with("oa-"));
        assert_eq!(created.title, "Test task");
        assert_eq!(created.description, "A test description");
        assert_eq!(created.status, TaskStatus::Open);
        assert_eq!(created.priority, TaskPriority::High);

        let fetched = repo.get(&created.id).unwrap();
        assert_eq!(fetched.id, created.id);
    }

    #[test]
    fn test_update() {
        let repo = setup();

        let task = TaskCreate {
            title: "Original title".into(),
            ..Default::default()
        };

        let created = repo.create(task).unwrap();

        let mut update = TaskUpdate::default();
        update.title = Some("Updated title".into());
        update.priority = Some(TaskPriority::Critical);

        let updated = repo.update(&created.id, update).unwrap();
        assert_eq!(updated.title, "Updated title");
        assert_eq!(updated.priority, TaskPriority::Critical);
    }

    #[test]
    fn test_ready_tasks() {
        let repo = setup();

        // Create parent task
        let parent = repo
            .create(TaskCreate {
                title: "Parent".into(),
                ..Default::default()
            })
            .unwrap();

        // Create child with dependency
        let child = repo
            .create(TaskCreate {
                title: "Child".into(),
                deps: vec![Dependency {
                    id: parent.id.clone(),
                    dep_type: DependencyType::Blocks,
                }],
                ..Default::default()
            })
            .unwrap();

        // Only parent should be ready
        let ready = repo.ready_tasks(TaskFilter::default()).unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, parent.id);

        // Close parent
        repo.close(&parent.id, Some("Done"), vec![]).unwrap();

        // Now child should be ready
        let ready = repo.ready_tasks(TaskFilter::default()).unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, child.id);
    }

    #[test]
    fn test_cycle_detection() {
        let repo = setup();

        let a = repo
            .create(TaskCreate {
                title: "Task A".into(),
                ..Default::default()
            })
            .unwrap();

        let b = repo
            .create(TaskCreate {
                title: "Task B".into(),
                deps: vec![Dependency {
                    id: a.id.clone(),
                    dep_type: DependencyType::Blocks,
                }],
                ..Default::default()
            })
            .unwrap();

        // Try to add A -> B (would create cycle: A -> B -> A)
        let result = repo.add_dependency(
            &a.id,
            Dependency {
                id: b.id.clone(),
                dep_type: DependencyType::Blocks,
            },
        );

        assert!(matches!(result, Err(TaskError::CycleDetected(_))));
    }

    #[test]
    fn test_state_transitions() {
        let repo = setup();

        let task = repo
            .create(TaskCreate {
                title: "State test".into(),
                ..Default::default()
            })
            .unwrap();

        // Open -> InProgress
        let task = repo.start(&task.id).unwrap();
        assert_eq!(task.status, TaskStatus::InProgress);

        // Can't start again (wrong state)
        assert!(repo.start(&task.id).is_err());

        // Close
        let task = repo.close(&task.id, Some("Done"), vec!["abc123".into()]).unwrap();
        assert_eq!(task.status, TaskStatus::Closed);
        assert!(task.closed_at.is_some());
        assert!(task.commits.contains(&"abc123".to_string()));

        // Reopen
        let task = repo.reopen(&task.id).unwrap();
        assert_eq!(task.status, TaskStatus::Open);
    }

    #[test]
    fn test_priority_sorting() {
        let repo = setup();

        // Create tasks with different priorities
        repo.create(TaskCreate {
            title: "Low".into(),
            priority: TaskPriority::Low,
            ..Default::default()
        })
        .unwrap();

        repo.create(TaskCreate {
            title: "Critical".into(),
            priority: TaskPriority::Critical,
            ..Default::default()
        })
        .unwrap();

        repo.create(TaskCreate {
            title: "Medium".into(),
            priority: TaskPriority::Medium,
            ..Default::default()
        })
        .unwrap();

        let ready = repo.ready_tasks(TaskFilter::default()).unwrap();
        assert_eq!(ready[0].title, "Critical");
        assert_eq!(ready[1].title, "Medium");
        assert_eq!(ready[2].title, "Low");
    }

    #[test]
    fn test_hash_based_id() {
        let repo = setup();

        let task = TaskCreate {
            title: "Deterministic task".into(),
            description: Some("Same content".into()),
            ..Default::default()
        };

        let t1 = repo
            .create_with_id_method(task.clone(), IdMethod::Hash, "oa")
            .unwrap();

        // Same content should produce same ID (and fail as duplicate)
        let result = repo.create_with_id_method(task, IdMethod::Hash, "oa");
        assert!(matches!(result, Err(TaskError::AlreadyExists(_))));

        // Verify ID is deterministic (hash-based)
        assert!(t1.id.starts_with("oa-"));
    }
}
