//! SQLite schema definitions for taskmaster
//!
//! This module contains all SQL schema definitions, ported from Beads.

/// Current schema version
pub const SCHEMA_VERSION: u32 = 3;

/// Initial schema - creates all tables
pub const SCHEMA_V1: &str = r#"
-- Schema version tracking
CREATE TABLE IF NOT EXISTS _schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Main issues table (22+ fields from Beads)
CREATE TABLE IF NOT EXISTS issues (
    -- Primary key
    id TEXT PRIMARY KEY,

    -- Core fields
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    design TEXT,
    acceptance_criteria TEXT,
    notes TEXT,

    -- Status with tombstone support
    status TEXT NOT NULL CHECK (status IN (
        'open', 'in_progress', 'blocked', 'closed', 'tombstone'
    )) DEFAULT 'open',

    -- Classification
    priority INTEGER NOT NULL CHECK (priority BETWEEN 0 AND 4) DEFAULT 2,
    issue_type TEXT NOT NULL CHECK (issue_type IN (
        'bug', 'feature', 'task', 'epic', 'chore'
    )) DEFAULT 'task',

    -- Assignment
    assignee TEXT,

    -- Time tracking
    estimated_minutes INTEGER,

    -- Compaction (Beads feature)
    compaction_level INTEGER NOT NULL DEFAULT 0,

    -- Close metadata
    close_reason TEXT,

    -- Source tracking
    external_ref TEXT,
    source_repo TEXT,
    discovered_from TEXT,

    -- Content hash for dedup
    content_hash TEXT,

    -- Timestamps
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    closed_at TEXT,

    -- Tombstone fields
    tombstoned_at TEXT,
    tombstone_ttl_days INTEGER DEFAULT 30,
    tombstone_reason TEXT
);

-- Labels (normalized for AND/OR filtering)
CREATE TABLE IF NOT EXISTS issue_labels (
    issue_id TEXT NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (issue_id, label)
);

-- Dependencies (4 types)
CREATE TABLE IF NOT EXISTS issue_dependencies (
    issue_id TEXT NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    depends_on_id TEXT NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    dependency_type TEXT NOT NULL CHECK (dependency_type IN (
        'blocks', 'related', 'parent-child', 'discovered-from'
    )),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (issue_id, depends_on_id)
);

-- Comments
CREATE TABLE IF NOT EXISTS issue_comments (
    id TEXT PRIMARY KEY,
    issue_id TEXT NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    author TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT
);

-- Events/Audit trail
CREATE TABLE IF NOT EXISTS issue_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id TEXT NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    actor TEXT,
    field_name TEXT,
    old_value TEXT,
    new_value TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Statistics snapshots
CREATE TABLE IF NOT EXISTS stats_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_date TEXT NOT NULL,
    total_issues INTEGER,
    open_count INTEGER,
    in_progress_count INTEGER,
    blocked_count INTEGER,
    closed_count INTEGER,
    tombstone_count INTEGER,
    avg_time_to_close_hours REAL,
    labels_json TEXT,
    priority_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Full-text search
CREATE VIRTUAL TABLE IF NOT EXISTS issues_fts USING fts5(
    id,
    title,
    description,
    notes,
    content='issues',
    content_rowid='rowid'
);

-- Triggers to keep FTS in sync
CREATE TRIGGER IF NOT EXISTS issues_fts_insert AFTER INSERT ON issues BEGIN
    INSERT INTO issues_fts(rowid, id, title, description, notes)
    VALUES (NEW.rowid, NEW.id, NEW.title, NEW.description, NEW.notes);
END;

CREATE TRIGGER IF NOT EXISTS issues_fts_delete AFTER DELETE ON issues BEGIN
    INSERT INTO issues_fts(issues_fts, rowid, id, title, description, notes)
    VALUES ('delete', OLD.rowid, OLD.id, OLD.title, OLD.description, OLD.notes);
END;

CREATE TRIGGER IF NOT EXISTS issues_fts_update AFTER UPDATE ON issues BEGIN
    INSERT INTO issues_fts(issues_fts, rowid, id, title, description, notes)
    VALUES ('delete', OLD.rowid, OLD.id, OLD.title, OLD.description, OLD.notes);
    INSERT INTO issues_fts(rowid, id, title, description, notes)
    VALUES (NEW.rowid, NEW.id, NEW.title, NEW.description, NEW.notes);
END;

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_issues_status ON issues(status);
CREATE INDEX IF NOT EXISTS idx_issues_priority ON issues(priority);
CREATE INDEX IF NOT EXISTS idx_issues_type ON issues(issue_type);
CREATE INDEX IF NOT EXISTS idx_issues_assignee ON issues(assignee);
CREATE INDEX IF NOT EXISTS idx_issues_created ON issues(created_at);
CREATE INDEX IF NOT EXISTS idx_issues_updated ON issues(updated_at);
CREATE INDEX IF NOT EXISTS idx_issues_content_hash ON issues(content_hash);

-- Partial index for ready issues (critical for performance)
CREATE INDEX IF NOT EXISTS idx_issues_ready ON issues(status, priority, created_at)
    WHERE status = 'open';

-- Partial index for tombstones
CREATE INDEX IF NOT EXISTS idx_issues_tombstone ON issues(tombstoned_at)
    WHERE status = 'tombstone';

-- Label indexes
CREATE INDEX IF NOT EXISTS idx_labels_issue ON issue_labels(issue_id);
CREATE INDEX IF NOT EXISTS idx_labels_label ON issue_labels(label);

-- Dependency indexes
CREATE INDEX IF NOT EXISTS idx_deps_issue ON issue_dependencies(issue_id);
CREATE INDEX IF NOT EXISTS idx_deps_target ON issue_dependencies(depends_on_id);
CREATE INDEX IF NOT EXISTS idx_deps_type ON issue_dependencies(dependency_type);

-- Event indexes
CREATE INDEX IF NOT EXISTS idx_events_issue ON issue_events(issue_id);
CREATE INDEX IF NOT EXISTS idx_events_time ON issue_events(created_at);
CREATE INDEX IF NOT EXISTS idx_events_type ON issue_events(event_type);

-- Comment indexes
CREATE INDEX IF NOT EXISTS idx_comments_issue ON issue_comments(issue_id);
CREATE INDEX IF NOT EXISTS idx_comments_time ON issue_comments(created_at);

-- Record schema version
INSERT OR IGNORE INTO _schema_version (version) VALUES (1);
"#;

/// Schema V2 migration - adds execution context fields for container-based parallel execution
pub const SCHEMA_V2: &str = r#"
-- Add execution context columns to issues table
ALTER TABLE issues ADD COLUMN execution_mode TEXT NOT NULL DEFAULT 'none';
ALTER TABLE issues ADD COLUMN execution_state TEXT NOT NULL DEFAULT 'unscheduled';
ALTER TABLE issues ADD COLUMN container_id TEXT;
ALTER TABLE issues ADD COLUMN agent_id TEXT;
ALTER TABLE issues ADD COLUMN execution_branch TEXT;
ALTER TABLE issues ADD COLUMN execution_started_at TEXT;
ALTER TABLE issues ADD COLUMN execution_finished_at TEXT;
ALTER TABLE issues ADD COLUMN execution_exit_code INTEGER;

-- Index for finding issues by execution state (for container orchestration)
CREATE INDEX IF NOT EXISTS idx_issues_execution_state ON issues(execution_state);

-- Index for finding issues by agent (for tracking what agents are working on)
CREATE INDEX IF NOT EXISTS idx_issues_agent_id ON issues(agent_id);

-- Partial index for running containers
CREATE INDEX IF NOT EXISTS idx_issues_container ON issues(container_id)
    WHERE container_id IS NOT NULL;

-- Partial index for active executions
CREATE INDEX IF NOT EXISTS idx_issues_execution_active ON issues(execution_state, agent_id)
    WHERE execution_state IN ('queued', 'provisioning', 'running');

-- Record schema version
INSERT OR REPLACE INTO _schema_version (version) VALUES (2);
"#;

/// Schema V3 migration - adds commits field for tracking agent work
pub const SCHEMA_V3: &str = r#"
-- Add commits column to issues table (JSON array of commit SHAs)
ALTER TABLE issues ADD COLUMN commits TEXT NOT NULL DEFAULT '[]';

-- Record schema version
INSERT OR REPLACE INTO _schema_version (version) VALUES (3);
"#;

/// SQL to check current schema version
pub const CHECK_VERSION: &str = "SELECT MAX(version) FROM _schema_version";

/// SQL to get issue by ID
pub const GET_ISSUE: &str = r#"
SELECT
    id, title, description, design, acceptance_criteria, notes,
    status, priority, issue_type, assignee, estimated_minutes,
    compaction_level, close_reason, external_ref, source_repo,
    discovered_from, content_hash, created_at, updated_at, closed_at,
    tombstoned_at, tombstone_ttl_days, tombstone_reason,
    execution_mode, execution_state, container_id, agent_id,
    execution_branch, execution_started_at, execution_finished_at, execution_exit_code,
    commits
FROM issues
WHERE id = ?1 AND status != 'tombstone'
"#;

/// SQL to get issue by ID including tombstones
pub const GET_ISSUE_WITH_TOMBSTONES: &str = r#"
SELECT
    id, title, description, design, acceptance_criteria, notes,
    status, priority, issue_type, assignee, estimated_minutes,
    compaction_level, close_reason, external_ref, source_repo,
    discovered_from, content_hash, created_at, updated_at, closed_at,
    tombstoned_at, tombstone_ttl_days, tombstone_reason,
    execution_mode, execution_state, container_id, agent_id,
    execution_branch, execution_started_at, execution_finished_at, execution_exit_code,
    commits
FROM issues
WHERE id = ?1
"#;

/// SQL to insert a new issue
pub const INSERT_ISSUE: &str = r#"
INSERT INTO issues (
    id, title, description, design, acceptance_criteria, notes,
    status, priority, issue_type, assignee, estimated_minutes,
    compaction_level, close_reason, external_ref, source_repo,
    discovered_from, content_hash, created_at, updated_at, closed_at,
    tombstoned_at, tombstone_ttl_days, tombstone_reason,
    execution_mode, execution_state, container_id, agent_id,
    execution_branch, execution_started_at, execution_finished_at, execution_exit_code,
    commits
) VALUES (
    ?1, ?2, ?3, ?4, ?5, ?6,
    ?7, ?8, ?9, ?10, ?11,
    ?12, ?13, ?14, ?15,
    ?16, ?17, ?18, ?19, ?20,
    ?21, ?22, ?23,
    ?24, ?25, ?26, ?27,
    ?28, ?29, ?30, ?31,
    ?32
)
"#;

/// SQL to check if issue exists
pub const EXISTS_ISSUE: &str = "SELECT 1 FROM issues WHERE id = ?1 LIMIT 1";

/// SQL to get labels for an issue
pub const GET_LABELS: &str = "SELECT label FROM issue_labels WHERE issue_id = ?1 ORDER BY label";

/// SQL to get dependencies for an issue
pub const GET_DEPENDENCIES: &str = r#"
SELECT depends_on_id, dependency_type, created_at
FROM issue_dependencies
WHERE issue_id = ?1
ORDER BY created_at
"#;

/// SQL to insert a label
pub const INSERT_LABEL: &str = r#"
INSERT OR IGNORE INTO issue_labels (issue_id, label, created_at)
VALUES (?1, ?2, datetime('now'))
"#;

/// SQL to delete a label
pub const DELETE_LABEL: &str = "DELETE FROM issue_labels WHERE issue_id = ?1 AND label = ?2";

/// SQL to insert a dependency
pub const INSERT_DEPENDENCY: &str = r#"
INSERT OR REPLACE INTO issue_dependencies (issue_id, depends_on_id, dependency_type, created_at)
VALUES (?1, ?2, ?3, datetime('now'))
"#;

/// SQL to delete a dependency
pub const DELETE_DEPENDENCY: &str = r#"
DELETE FROM issue_dependencies WHERE issue_id = ?1 AND depends_on_id = ?2
"#;

/// SQL to get comments for an issue
pub const GET_COMMENTS: &str = r#"
SELECT id, issue_id, author, body, created_at, updated_at
FROM issue_comments
WHERE issue_id = ?1
ORDER BY created_at
"#;

/// SQL to insert a comment
pub const INSERT_COMMENT: &str = r#"
INSERT INTO issue_comments (id, issue_id, author, body, created_at)
VALUES (?1, ?2, ?3, ?4, datetime('now'))
"#;

/// SQL to insert an event
pub const INSERT_EVENT: &str = r#"
INSERT INTO issue_events (issue_id, event_type, actor, field_name, old_value, new_value, metadata, created_at)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
"#;

/// SQL to get events for an issue
pub const GET_EVENTS: &str = r#"
SELECT id, issue_id, event_type, actor, field_name, old_value, new_value, metadata, created_at
FROM issue_events
WHERE issue_id = ?1
ORDER BY created_at DESC
LIMIT ?2
"#;

/// SQL to get all labels with counts
pub const GET_ALL_LABELS: &str = r#"
SELECT label, COUNT(*) as count
FROM issue_labels
GROUP BY label
ORDER BY count DESC, label
"#;

/// SQL to count issues by status
pub const COUNT_BY_STATUS: &str = r#"
SELECT status, COUNT(*) as count
FROM issues
WHERE status != 'tombstone'
GROUP BY status
"#;

/// SQL to count issues by priority
pub const COUNT_BY_PRIORITY: &str = r#"
SELECT priority, COUNT(*) as count
FROM issues
WHERE status != 'tombstone'
GROUP BY priority
"#;

/// SQL to count issues by type
pub const COUNT_BY_TYPE: &str = r#"
SELECT issue_type, COUNT(*) as count
FROM issues
WHERE status != 'tombstone'
GROUP BY issue_type
"#;

/// SQL to find ready issues using recursive CTE
pub const READY_ISSUES: &str = r#"
WITH RECURSIVE blocked_issues AS (
    -- Base case: issues directly blocked by open/in_progress/blocked issues
    SELECT DISTINCT d.issue_id
    FROM issue_dependencies d
    JOIN issues i ON d.depends_on_id = i.id
    WHERE d.dependency_type IN ('blocks', 'parent-child')
    AND i.status IN ('open', 'in_progress', 'blocked')

    UNION

    -- Recursive case: issues blocked by blocked issues
    SELECT DISTINCT d.issue_id
    FROM issue_dependencies d
    JOIN blocked_issues b ON d.depends_on_id = b.issue_id
    WHERE d.dependency_type IN ('blocks', 'parent-child')
)
SELECT
    id, title, description, design, acceptance_criteria, notes,
    status, priority, issue_type, assignee, estimated_minutes,
    compaction_level, close_reason, external_ref, source_repo,
    discovered_from, content_hash, created_at, updated_at, closed_at,
    tombstoned_at, tombstone_ttl_days, tombstone_reason,
    execution_mode, execution_state, container_id, agent_id,
    execution_branch, execution_started_at, execution_finished_at, execution_exit_code,
    commits
FROM issues
WHERE status = 'open'
AND id NOT IN (SELECT issue_id FROM blocked_issues)
ORDER BY priority ASC, created_at ASC
"#;

/// SQL to check if an issue is blocked
pub const IS_BLOCKED: &str = r#"
SELECT EXISTS(
    SELECT 1
    FROM issue_dependencies d
    JOIN issues i ON d.depends_on_id = i.id
    WHERE d.issue_id = ?1
    AND d.dependency_type IN ('blocks', 'parent-child')
    AND i.status IN ('open', 'in_progress', 'blocked')
)
"#;

/// SQL to detect cycles (using recursive CTE)
pub const DETECT_CYCLE: &str = r#"
WITH RECURSIVE dep_chain AS (
    -- Start from the potential new dependency
    SELECT ?2 as id, ?1 as start_id, 1 as depth

    UNION ALL

    -- Follow dependencies
    SELECT d.depends_on_id, c.start_id, c.depth + 1
    FROM issue_dependencies d
    JOIN dep_chain c ON d.issue_id = c.id
    WHERE d.dependency_type IN ('blocks', 'parent-child')
    AND c.depth < 100  -- Prevent infinite loops
)
SELECT EXISTS(
    SELECT 1 FROM dep_chain WHERE id = start_id AND depth > 0
)
"#;

/// SQL to find stale issues
pub const STALE_ISSUES: &str = r#"
SELECT
    id, title, description, design, acceptance_criteria, notes,
    status, priority, issue_type, assignee, estimated_minutes,
    compaction_level, close_reason, external_ref, source_repo,
    discovered_from, content_hash, created_at, updated_at, closed_at,
    tombstoned_at, tombstone_ttl_days, tombstone_reason,
    execution_mode, execution_state, container_id, agent_id,
    execution_branch, execution_started_at, execution_finished_at, execution_exit_code,
    commits
FROM issues
WHERE status IN ('open', 'in_progress', 'blocked')
AND datetime(updated_at) < datetime('now', ?1)
ORDER BY updated_at ASC
LIMIT ?2
"#;

/// SQL to find duplicates by content hash
pub const FIND_DUPLICATES: &str = r#"
SELECT content_hash, GROUP_CONCAT(id) as ids, MIN(title) as title
FROM issues
WHERE content_hash IS NOT NULL
AND status != 'tombstone'
GROUP BY content_hash
HAVING COUNT(*) > 1
"#;

/// SQL to get expired tombstones
pub const EXPIRED_TOMBSTONES: &str = r#"
SELECT id
FROM issues
WHERE status = 'tombstone'
AND datetime(tombstoned_at, '+' || COALESCE(tombstone_ttl_days, 30) || ' days') < datetime('now')
"#;
