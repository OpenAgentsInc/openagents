-- OpenAgents SQLite Schema v1.0.0
-- Initial migration: Tasks, dependencies, deletions, and FTS

-- ============================================================================
-- Schema Version Tracking
-- ============================================================================

CREATE TABLE _schema_version (
  version TEXT PRIMARY KEY,
  applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO _schema_version (version) VALUES ('1.0.0');

-- ============================================================================
-- Tasks Table
-- ============================================================================

CREATE TABLE tasks (
  -- Primary key
  id TEXT PRIMARY KEY,

  -- Core fields
  title TEXT NOT NULL,
  description TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('open', 'in_progress', 'blocked', 'closed', 'commit_pending')),
  priority INTEGER NOT NULL CHECK (priority BETWEEN 0 AND 4),
  type TEXT NOT NULL CHECK (type IN ('bug', 'feature', 'task', 'epic', 'chore')),

  -- Optional fields
  assignee TEXT,
  close_reason TEXT,

  -- JSON fields (for complex/variable data)
  labels JSON,           -- Array of strings
  commits JSON,          -- Array of commit SHAs
  comments JSON,         -- Array of comment objects
  pending_commit JSON,   -- PendingCommit object for two-phase commit

  -- Extended fields (from beads compatibility)
  design TEXT,
  acceptance_criteria TEXT,
  notes TEXT,
  estimated_minutes REAL,

  -- Source tracking
  source_repo TEXT,
  source_discovered_from TEXT,
  source_external_ref TEXT,

  -- Timestamps
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  closed_at TEXT,

  -- Soft delete
  deleted_at TEXT
);

-- ============================================================================
-- Task Dependencies (many-to-many)
-- ============================================================================

CREATE TABLE task_dependencies (
  task_id TEXT NOT NULL,
  depends_on_task_id TEXT NOT NULL,
  dependency_type TEXT NOT NULL CHECK (
    dependency_type IN ('blocks', 'related', 'parent-child', 'discovered-from')
  ),
  created_at TEXT NOT NULL DEFAULT (datetime('now')),

  PRIMARY KEY (task_id, depends_on_task_id),
  FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
  FOREIGN KEY (depends_on_task_id) REFERENCES tasks(id) ON DELETE CASCADE
);

-- ============================================================================
-- Deletion Tombstones
-- ============================================================================

CREATE TABLE task_deletions (
  task_id TEXT PRIMARY KEY,
  deleted_at TEXT NOT NULL,
  deleted_by TEXT,
  reason TEXT
);

-- ============================================================================
-- Indexes for Performance
-- ============================================================================

-- Single column indexes
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_priority ON tasks(priority);
CREATE INDEX idx_tasks_type ON tasks(type);
CREATE INDEX idx_tasks_assignee ON tasks(assignee);
CREATE INDEX idx_tasks_created_at ON tasks(created_at);
CREATE INDEX idx_tasks_updated_at ON tasks(updated_at);
CREATE INDEX idx_tasks_deleted_at ON tasks(deleted_at);

-- Composite indexes for common queries
CREATE INDEX idx_tasks_status_priority ON tasks(status, priority);

-- Partial index for ready task query (CRITICAL for performance)
-- This index is used heavily by readyTasks() and pickNextTask()
CREATE INDEX idx_tasks_ready ON tasks(status, priority, created_at)
  WHERE deleted_at IS NULL;

-- Index for dependency lookups
CREATE INDEX idx_task_dependencies_task_id ON task_dependencies(task_id);
CREATE INDEX idx_task_dependencies_depends_on ON task_dependencies(depends_on_task_id);

-- ============================================================================
-- Full-Text Search (FTS5)
-- ============================================================================

-- Virtual table for full-text search on title and description
CREATE VIRTUAL TABLE tasks_fts USING fts5(
  id UNINDEXED,
  title,
  description,
  content=tasks,
  content_rowid=rowid
);

-- Triggers to keep FTS table in sync with tasks table
CREATE TRIGGER tasks_fts_insert AFTER INSERT ON tasks BEGIN
  INSERT INTO tasks_fts(rowid, id, title, description)
  VALUES (NEW.rowid, NEW.id, NEW.title, NEW.description);
END;

CREATE TRIGGER tasks_fts_update AFTER UPDATE ON tasks BEGIN
  UPDATE tasks_fts SET title = NEW.title, description = NEW.description
  WHERE rowid = NEW.rowid;
END;

CREATE TRIGGER tasks_fts_delete AFTER DELETE ON tasks BEGIN
  DELETE FROM tasks_fts WHERE rowid = OLD.rowid;
END;

-- ============================================================================
-- Metadata Comments
-- ============================================================================

-- This schema supports:
-- - Two-phase commit pattern via pending_commit JSON field and commit_pending status
-- - Soft deletes via deleted_at timestamp (enables recovery and audit trail)
-- - Efficient ready task queries via partial index on (status, priority, created_at)
-- - Full-text search on title and description via FTS5
-- - Dependency tracking with cascading deletes
-- - Deletion tombstones for audit trail

-- Performance notes:
-- - idx_tasks_ready is critical for readyTasks() - indexes only non-deleted open tasks
-- - FTS5 enables fast text search without needing LIKE '%term%' scans
-- - Composite indexes reduce query time from O(n) to O(log n)

-- Migration notes:
-- - All timestamps are ISO 8601 format (YYYY-MM-DDTHH:MM:SS.sssZ)
-- - JSON fields are stored as TEXT and parsed at application level
-- - Soft deletes preserve data for recovery (set deleted_at to restore)
-- - task_deletions provides audit trail separate from main table
