# OpenAgents SQLite Schema Design

## Overview

This document defines the SQLite schema for `.openagents/` storage, replacing the current JSONL-based approach with a unified database for better performance, querying, and analytics.

### Rationale

**Current problems with JSONL:**
- Sequential O(n) scans for every query
- No indexing or efficient filtering
- Can't do joins or aggregations
- Multiple file reads for cross-cutting queries
- No vector similarity search
- Potential corruption from concurrent writes

**SQLite benefits:**
- O(log n) indexed lookups
- Efficient filtering, joins, aggregations
- Built-in FTS5 for full-text search
- Vector search with sqlite-vec extension
- ACID guarantees
- Single portable file
- Native JSON support for nested data

### Database Location

**Path**: `.openagents/openagents.db`

**Migrations**: `.openagents/migrations/` (SQL migration scripts)

**Backup**: JSONL files remain during transition as fallback/backup

---

## Schema Version

**Current version**: `1.0.0`

Schema version is tracked in a metadata table for migration management.

```sql
CREATE TABLE _schema_version (
  version TEXT PRIMARY KEY,
  applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO _schema_version (version) VALUES ('1.0.0');
```

---

## Core Tables

### 1. Tasks

Replaces: `tasks.jsonl`

```sql
CREATE TABLE tasks (
  -- Primary key
  id TEXT PRIMARY KEY,  -- e.g., "oa-281868"

  -- Core fields
  title TEXT NOT NULL,
  description TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('open', 'in_progress', 'blocked', 'closed')),
  priority INTEGER NOT NULL CHECK (priority BETWEEN 0 AND 4),  -- 0=P0, 4=backlog
  type TEXT NOT NULL CHECK (type IN ('bug', 'feature', 'task', 'epic', 'chore')),

  -- Optional fields
  assignee TEXT,
  close_reason TEXT,

  -- JSON fields (for complex/variable data)
  labels JSON,          -- Array of strings
  commits JSON,         -- Array of commit SHAs
  comments JSON,        -- Array of comment objects

  -- Timestamps
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  closed_at TEXT
);

-- Task dependencies (many-to-many)
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

-- Indexes for common queries
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_priority ON tasks(priority);
CREATE INDEX idx_tasks_type ON tasks(type);
CREATE INDEX idx_tasks_assignee ON tasks(assignee);
CREATE INDEX idx_tasks_created_at ON tasks(created_at);
CREATE INDEX idx_tasks_status_priority ON tasks(status, priority);

-- Full-text search on title and description
CREATE VIRTUAL TABLE tasks_fts USING fts5(
  id UNINDEXED,
  title,
  description,
  content=tasks,
  content_rowid=rowid
);

-- Triggers to keep FTS in sync
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
```

**Example queries:**

```sql
-- Find all ready tasks (open with no open blocking deps)
SELECT t.* FROM tasks t
WHERE t.status = 'open'
AND NOT EXISTS (
  SELECT 1 FROM task_dependencies td
  JOIN tasks blocker ON td.depends_on_task_id = blocker.id
  WHERE td.task_id = t.id
  AND td.dependency_type IN ('blocks', 'parent-child')
  AND blocker.status IN ('open', 'in_progress')
)
ORDER BY t.priority ASC, t.created_at ASC;

-- Full-text search tasks
SELECT * FROM tasks WHERE id IN (
  SELECT id FROM tasks_fts WHERE tasks_fts MATCH 'CLI OR task-system'
);

-- Get task with all dependencies
SELECT
  t.*,
  json_group_array(
    json_object(
      'task_id', d.depends_on_task_id,
      'type', d.dependency_type,
      'title', dt.title,
      'status', dt.status
    )
  ) as dependencies
FROM tasks t
LEFT JOIN task_dependencies d ON t.id = d.task_id
LEFT JOIN tasks dt ON d.depends_on_task_id = dt.id
WHERE t.id = 'oa-281868'
GROUP BY t.id;
```

---

### 2. Trajectories

Replaces: `trajectories.jsonl`, `trajectories/YYYYMMDD/*.atif.jsonl`

```sql
CREATE TABLE trajectories (
  -- Primary key
  session_id TEXT PRIMARY KEY,

  -- Extracted metadata (for efficient querying)
  schema_version TEXT NOT NULL,  -- e.g., "ATIF-v1.4"
  agent_name TEXT NOT NULL,
  agent_version TEXT,
  model_name TEXT,
  step_count INTEGER NOT NULL,

  -- Computed fields
  first_step_at TEXT,
  last_step_at TEXT,
  total_cost_usd REAL,
  total_tokens INTEGER,

  -- Full ATIF JSON (for complete data)
  trajectory JSON NOT NULL,

  -- Vector embedding for semantic search
  embedding BLOB,

  -- Timestamps
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  indexed_at TEXT
);

-- Indexes
CREATE INDEX idx_trajectories_agent ON trajectories(agent_name);
CREATE INDEX idx_trajectories_model ON trajectories(model_name);
CREATE INDEX idx_trajectories_created_at ON trajectories(created_at);
CREATE INDEX idx_trajectories_step_count ON trajectories(step_count);
CREATE INDEX idx_trajectories_cost ON trajectories(total_cost_usd);

-- Full-text search on trajectory content
-- (indexes all step messages for searchability)
CREATE VIRTUAL TABLE trajectories_fts USING fts5(
  session_id UNINDEXED,
  agent_name UNINDEXED,
  content  -- All step messages concatenated
);
```

**Example queries:**

```sql
-- Find trajectories by agent and date range
SELECT session_id, agent_name, model_name, step_count, total_cost_usd
FROM trajectories
WHERE agent_name = 'mechacoder-orchestrator'
AND created_at BETWEEN '2025-12-01' AND '2025-12-31'
ORDER BY created_at DESC;

-- Get full trajectory JSON
SELECT json(trajectory) FROM trajectories WHERE session_id = 'session-123';

-- Aggregate stats by agent
SELECT
  agent_name,
  COUNT(*) as trajectory_count,
  AVG(step_count) as avg_steps,
  SUM(total_cost_usd) as total_cost,
  SUM(total_tokens) as total_tokens
FROM trajectories
GROUP BY agent_name
ORDER BY total_cost DESC;

-- Semantic search (with sqlite-vec)
SELECT session_id, agent_name, step_count
FROM trajectories
WHERE vec_distance_cosine(embedding, ?) < 0.5
ORDER BY vec_distance_cosine(embedding, ?)
LIMIT 10;
```

---

### 3. Memories

Replaces: `memories.jsonl`

```sql
CREATE TABLE memories (
  -- Primary key
  id TEXT PRIMARY KEY,  -- e.g., "mem-epi-miuvye9d-nc6a49"

  -- Core fields
  memory_type TEXT NOT NULL CHECK (memory_type IN ('episodic', 'semantic', 'procedural')),
  scope TEXT NOT NULL CHECK (scope IN ('session', 'project', 'global')),
  status TEXT NOT NULL CHECK (status IN ('active', 'archived', 'deleted')),
  description TEXT NOT NULL,

  -- Metadata
  importance TEXT CHECK (importance IN ('critical', 'high', 'medium', 'low')),
  tags JSON,  -- Array of strings

  -- Content (variable structure by memory type)
  content JSON NOT NULL,

  -- Vector embedding for semantic search
  embedding BLOB,

  -- Access tracking
  access_count INTEGER NOT NULL DEFAULT 0,
  last_accessed_at TEXT,

  -- Timestamps
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),

  -- Source tracking
  source TEXT  -- e.g., "task", "conversation", "reflection"
);

-- Indexes
CREATE INDEX idx_memories_type ON memories(memory_type);
CREATE INDEX idx_memories_scope ON memories(scope);
CREATE INDEX idx_memories_status ON memories(status);
CREATE INDEX idx_memories_importance ON memories(importance);
CREATE INDEX idx_memories_created_at ON memories(created_at);
CREATE INDEX idx_memories_access_count ON memories(access_count);

-- Full-text search
CREATE VIRTUAL TABLE memories_fts USING fts5(
  id UNINDEXED,
  description,
  content  -- JSON content as text
);
```

**Example queries:**

```sql
-- Find active episodic memories by importance
SELECT id, description, importance, access_count
FROM memories
WHERE memory_type = 'episodic'
AND status = 'active'
AND importance IN ('critical', 'high')
ORDER BY created_at DESC
LIMIT 50;

-- Semantic search for relevant memories
SELECT id, description, importance,
       vec_distance_cosine(embedding, ?) as similarity
FROM memories
WHERE status = 'active'
AND vec_distance_cosine(embedding, ?) < 0.7
ORDER BY similarity ASC
LIMIT 10;

-- Get most frequently accessed memories
SELECT id, description, access_count, last_accessed_at
FROM memories
WHERE status = 'active'
ORDER BY access_count DESC
LIMIT 20;
```

---

### 4. Episodes (Gym Training)

Replaces: `gym/episodes.jsonl`

```sql
CREATE TABLE episodes (
  -- Primary key
  id TEXT PRIMARY KEY,  -- e.g., "tbrun-20251205-205035-l7sn-001"

  -- Run metadata
  run_id TEXT NOT NULL,
  iteration INTEGER NOT NULL,
  model TEXT NOT NULL,
  suite_version TEXT NOT NULL,

  -- Timing
  started_at TEXT NOT NULL,
  finished_at TEXT,
  duration_ms INTEGER,

  -- Status
  status TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed', 'timeout', 'error')),

  -- Summary stats (extracted from JSON for indexing)
  total_tests INTEGER,
  passed_tests INTEGER,
  failed_tests INTEGER,
  timeout_tests INTEGER,
  error_tests INTEGER,
  pass_rate REAL,
  avg_turns REAL,
  avg_tokens REAL,

  -- Full episode data
  summary JSON,
  results_path TEXT,

  -- Timestamps
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Indexes
CREATE INDEX idx_episodes_run_id ON episodes(run_id);
CREATE INDEX idx_episodes_model ON episodes(model);
CREATE INDEX idx_episodes_status ON episodes(status);
CREATE INDEX idx_episodes_pass_rate ON episodes(pass_rate);
CREATE INDEX idx_episodes_started_at ON episodes(started_at);
```

**Example queries:**

```sql
-- Find best episodes by pass rate
SELECT id, run_id, model, pass_rate, total_tests
FROM episodes
WHERE status = 'completed'
ORDER BY pass_rate DESC, total_tests DESC
LIMIT 10;

-- Aggregate stats by model
SELECT
  model,
  COUNT(*) as episode_count,
  AVG(pass_rate) as avg_pass_rate,
  AVG(avg_turns) as avg_turns,
  AVG(duration_ms) as avg_duration
FROM episodes
WHERE status = 'completed'
GROUP BY model
ORDER BY avg_pass_rate DESC;

-- Track progress over time
SELECT
  DATE(started_at) as date,
  AVG(pass_rate) as avg_pass_rate,
  COUNT(*) as episodes
FROM episodes
WHERE model = 'claude-code'
AND status = 'completed'
GROUP BY DATE(started_at)
ORDER BY date DESC;
```

---

### 5. Usage Logs

Replaces: `usage.jsonl`

```sql
CREATE TABLE usage_logs (
  -- Auto-increment primary key
  id INTEGER PRIMARY KEY AUTOINCREMENT,

  -- Session reference
  session_id TEXT NOT NULL,
  project_id TEXT NOT NULL,

  -- Usage metrics
  agent TEXT NOT NULL,
  input_tokens INTEGER NOT NULL DEFAULT 0,
  output_tokens INTEGER NOT NULL DEFAULT 0,
  cache_read_tokens INTEGER NOT NULL DEFAULT 0,
  cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
  total_cost_usd REAL NOT NULL DEFAULT 0.0,

  -- Execution metadata
  subtasks INTEGER NOT NULL DEFAULT 0,
  duration_ms INTEGER,
  idempotency_key TEXT,

  -- Timestamp
  timestamp TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Indexes
CREATE INDEX idx_usage_session_id ON usage_logs(session_id);
CREATE INDEX idx_usage_project_id ON usage_logs(project_id);
CREATE INDEX idx_usage_agent ON usage_logs(agent);
CREATE INDEX idx_usage_timestamp ON usage_logs(timestamp);
CREATE INDEX idx_usage_cost ON usage_logs(total_cost_usd);
```

**Example queries:**

```sql
-- Total cost by project
SELECT
  project_id,
  SUM(total_cost_usd) as total_cost,
  SUM(input_tokens + output_tokens) as total_tokens,
  COUNT(*) as request_count
FROM usage_logs
GROUP BY project_id
ORDER BY total_cost DESC;

-- Daily usage breakdown
SELECT
  DATE(timestamp) as date,
  agent,
  SUM(total_cost_usd) as daily_cost,
  SUM(input_tokens + output_tokens) as daily_tokens,
  COUNT(*) as requests
FROM usage_logs
WHERE DATE(timestamp) >= DATE('now', '-30 days')
GROUP BY DATE(timestamp), agent
ORDER BY date DESC, agent;

-- Most expensive sessions
SELECT
  session_id,
  agent,
  SUM(total_cost_usd) as session_cost,
  SUM(duration_ms) as total_duration,
  COUNT(*) as requests
FROM usage_logs
GROUP BY session_id, agent
ORDER BY session_cost DESC
LIMIT 20;
```

---

### 6. Sessions

Replaces: `sessions/*.jsonl`

```sql
CREATE TABLE sessions (
  -- Primary key
  session_id TEXT PRIMARY KEY,

  -- Session metadata
  project_id TEXT NOT NULL,
  user_id TEXT,
  agent_name TEXT NOT NULL,

  -- Session state
  status TEXT NOT NULL CHECK (status IN ('active', 'completed', 'failed', 'abandoned')),

  -- Timing
  started_at TEXT NOT NULL,
  ended_at TEXT,
  duration_ms INTEGER,

  -- Stats
  message_count INTEGER NOT NULL DEFAULT 0,
  turn_count INTEGER NOT NULL DEFAULT 0,
  total_tokens INTEGER NOT NULL DEFAULT 0,
  total_cost_usd REAL NOT NULL DEFAULT 0.0,

  -- Full session log (if needed)
  log_path TEXT,

  -- Timestamps
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Indexes
CREATE INDEX idx_sessions_project_id ON sessions(project_id);
CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_agent ON sessions(agent_name);
CREATE INDEX idx_sessions_status ON sessions(status);
CREATE INDEX idx_sessions_started_at ON sessions(started_at);
```

**Example queries:**

```sql
-- Active sessions
SELECT session_id, agent_name, started_at, message_count
FROM sessions
WHERE status = 'active'
ORDER BY started_at DESC;

-- Session analytics by agent
SELECT
  agent_name,
  COUNT(*) as session_count,
  AVG(duration_ms) as avg_duration,
  AVG(turn_count) as avg_turns,
  SUM(total_cost_usd) as total_cost
FROM sessions
WHERE status = 'completed'
GROUP BY agent_name;
```

---

### 7. Run Logs

Replaces: `run-logs/*.jsonl`

```sql
CREATE TABLE run_logs (
  -- Auto-increment primary key
  id INTEGER PRIMARY KEY AUTOINCREMENT,

  -- Run reference
  run_id TEXT NOT NULL,
  session_id TEXT,

  -- Log entry
  level TEXT NOT NULL CHECK (level IN ('debug', 'info', 'warn', 'error', 'fatal')),
  message TEXT NOT NULL,

  -- Context
  component TEXT,  -- e.g., "orchestrator", "planner", "coder"
  task_id TEXT,

  -- Structured data
  metadata JSON,

  -- Timestamp
  timestamp TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Indexes
CREATE INDEX idx_runlogs_run_id ON run_logs(run_id);
CREATE INDEX idx_runlogs_session_id ON run_logs(session_id);
CREATE INDEX idx_runlogs_level ON run_logs(level);
CREATE INDEX idx_runlogs_component ON run_logs(component);
CREATE INDEX idx_runlogs_timestamp ON run_logs(timestamp);

-- Full-text search
CREATE VIRTUAL TABLE run_logs_fts USING fts5(
  message,
  component UNINDEXED,
  content=run_logs,
  content_rowid=id
);
```

**Example queries:**

```sql
-- Find errors in a run
SELECT timestamp, component, message
FROM run_logs
WHERE run_id = 'tbrun-20251205-205035-l7sn'
AND level IN ('error', 'fatal')
ORDER BY timestamp;

-- Search logs
SELECT rl.* FROM run_logs rl
WHERE rl.id IN (
  SELECT rowid FROM run_logs_fts WHERE run_logs_fts MATCH 'timeout OR failed'
)
ORDER BY timestamp DESC
LIMIT 50;

-- Log volume by component
SELECT
  component,
  level,
  COUNT(*) as count
FROM run_logs
WHERE timestamp >= datetime('now', '-1 day')
GROUP BY component, level
ORDER BY component, count DESC;
```

---

## Vector Search Setup

Using `sqlite-vec` extension for embedding-based semantic search.

### Installation

```bash
# Download sqlite-vec extension (platform-specific)
# https://github.com/asg017/sqlite-vec

# Load in SQLite
.load ./vec0
```

### Vector Tables

For tables with embeddings (trajectories, memories):

```sql
-- Create virtual table for vector index
CREATE VIRTUAL TABLE trajectories_vec USING vec0(
  session_id TEXT PRIMARY KEY,
  embedding float[1536]  -- Dimension depends on embedding model
);

-- Insert embeddings
INSERT INTO trajectories_vec (session_id, embedding)
SELECT session_id, embedding FROM trajectories WHERE embedding IS NOT NULL;

-- Nearest neighbor search
SELECT
  t.session_id,
  t.agent_name,
  t.step_count,
  v.distance
FROM trajectories_vec v
JOIN trajectories t ON v.session_id = t.session_id
WHERE v.embedding MATCH ?
AND v.k = 10
ORDER BY v.distance;
```

---

## Migration Strategy

### Phase 1: Parallel Operation
1. Create SQLite database alongside JSONL files
2. Implement dual-write: write to both JSONL and SQLite
3. All reads come from SQLite (with JSONL fallback if row missing)
4. Keeps JSONL as backup/source of truth during transition

### Phase 2: Backfill
1. Write migration script to import all existing JSONL → SQLite
2. Validate data integrity (row counts, spot checks)
3. Keep JSONL files as backup (don't delete yet)

### Phase 3: SQLite Primary
1. Switch to SQLite-only writes
2. Remove JSONL fallback reads
3. JSONL becomes append-only backup/export format
4. Add periodic SQLite → JSONL export for backup/portability

### Phase 4: Cleanup (optional)
1. Archive old JSONL files
2. Keep only recent JSONL exports as backup

---

## Migration Scripts

Location: `.openagents/migrations/`

```
.openagents/migrations/
  001_initial_schema.sql       # Create all tables
  002_import_tasks.sql          # Import tasks.jsonl
  003_import_trajectories.sql   # Import trajectories
  004_import_memories.sql       # Import memories
  ...
```

---

## Implementation Checklist

- [ ] Create `.openagents/migrations/` directory
- [ ] Write `001_initial_schema.sql` with all table definitions
- [ ] Implement `DatabaseService` in `src/storage/database.ts`
  - [ ] Schema initialization
  - [ ] Migration runner
  - [ ] CRUD operations for each table
  - [ ] Query builders
- [ ] Implement dual-write adapters for each service:
  - [ ] TaskService → SQLite + JSONL
  - [ ] TrajectoryService → SQLite + JSONL
  - [ ] MemoryService → SQLite + JSONL
  - [ ] etc.
- [ ] Write import scripts for existing JSONL data
- [ ] Add indexes and FTS tables
- [ ] Integrate sqlite-vec for embedding search
- [ ] Write tests for all database operations
- [ ] Document query patterns and examples
- [ ] Add SQLite backup/export commands

---

## Performance Considerations

### Indexes
- Index all foreign keys
- Index all columns used in WHERE clauses
- Composite indexes for common query patterns
- Keep FTS tables in sync with triggers

### Query Optimization
- Use EXPLAIN QUERY PLAN to check index usage
- Avoid SELECT * when only specific columns needed
- Use prepared statements for repeated queries
- Batch inserts with transactions

### Database Maintenance
- Regular VACUUM to reclaim space
- ANALYZE to update query planner statistics
- Periodic integrity checks with PRAGMA integrity_check

### Backup
- Use SQLite backup API for hot backups
- Export to JSONL periodically for portability
- Keep database file in version control (if small)

---

## Example Implementation

```typescript
// src/storage/database.ts
import { Database } from "bun:sqlite";
import { Effect, Layer } from "effect";

export class DatabaseService extends Context.Tag("DatabaseService")<
  DatabaseService,
  {
    // Tasks
    createTask: (task: Task) => Effect.Effect<void, DatabaseError>;
    getTask: (id: string) => Effect.Effect<Task | null, DatabaseError>;
    listTasks: (filter: TaskFilter) => Effect.Effect<Task[], DatabaseError>;

    // Trajectories
    saveTrajectory: (trajectory: Trajectory) => Effect.Effect<void, DatabaseError>;
    getTrajectory: (sessionId: string) => Effect.Effect<Trajectory | null, DatabaseError>;
    searchTrajectories: (query: string) => Effect.Effect<Trajectory[], DatabaseError>;

    // Memories
    addMemory: (memory: Memory) => Effect.Effect<void, DatabaseError>;
    searchMemories: (embedding: Float32Array, limit: number) => Effect.Effect<Memory[], DatabaseError>;

    // Analytics
    getUsageStats: (projectId: string, startDate: string, endDate: string) => Effect.Effect<UsageStats, DatabaseError>;
  }
>() {}

export const DatabaseLive = Layer.effect(
  DatabaseService,
  Effect.gen(function* () {
    const db = new Database(".openagents/openagents.db");

    // Initialize schema
    yield* runMigrations(db);

    return {
      createTask: (task) => Effect.try({
        try: () => {
          const stmt = db.prepare(`
            INSERT INTO tasks (id, title, description, status, priority, type, labels, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
          `);
          stmt.run(
            task.id,
            task.title,
            task.description,
            task.status,
            task.priority,
            task.type,
            JSON.stringify(task.labels),
            task.createdAt
          );
        },
        catch: (error) => new DatabaseError("insert_failed", String(error))
      }),

      // ... implement other methods
    };
  })
);
```

---

## Future Enhancements

1. **Sharding**: Split large tables (trajectories) by date/agent
2. **Replication**: SQLite Litestream for continuous backup
3. **Read replicas**: Multiple SQLite files for read scaling
4. **Analytics views**: Materialized views for common dashboards
5. **Time-series optimization**: Separate tables for time-series data
6. **Graph queries**: Store task dependencies in graph-optimized format
7. **Embeddings pipeline**: Automatic embedding generation on insert

---

## References

- [SQLite Documentation](https://www.sqlite.org/docs.html)
- [SQLite FTS5](https://www.sqlite.org/fts5.html)
- [sqlite-vec](https://github.com/asg017/sqlite-vec)
- [Bun SQLite API](https://bun.sh/docs/api/sqlite)
- [ATIF Schema](../atif/schema.ts)
