# issues

Local issue tracking with SQLite for autonomous agents. Provides database schema, CRUD operations, state transitions, and multi-project support for the OpenAgents autopilot system.

## Overview

The `issues` crate is a **lightweight issue tracking library** backed by SQLite. It provides:

- **Issue lifecycle** - Create, claim, complete, block issues with status tracking
- **Priority queue** - Get next ready issue sorted by priority and age
- **Multi-agent support** - Filter issues by agent (codex, codex)
- **Project management** - Track multiple projects with sessions
- **Session tracking** - Monitor autopilot runs with budget and completion metrics
- **Automatic numbering** - Sequential issue numbers with trigger-based sync
- **Migrations** - Schema versioning from v1 to v5
- **Type-safe API** - Strongly-typed Rust interfaces with serde support

Perfect for:
- Autonomous agent work queues
- Multi-project autopilot orchestration
- Session monitoring and metrics
- Issue prioritization and blocking

## Quick Start

### Basic Usage

```rust
use issues::{db, issue, Priority, IssueType};
use std::path::Path;

// Initialize database
let conn = db::init_db(Path::new("autopilot.db"))?;

// Create an issue
let issue = issue::create_issue(
    &conn,
    "Fix authentication bug",
    Some("Users can't log in after upgrade"),
    Priority::Urgent,
    IssueType::Bug,
    Some("codex"), // agent
)?;

println!("Created issue #{}: {}", issue.number, issue.title);

// Get next ready issue
if let Some(next) = issue::get_next_ready_issue(&conn, Some("codex"))? {
    println!("Next task: #{} - {}", next.number, next.title);

    // Claim it
    issue::claim_issue(&conn, &next.id, "run-123")?;

    // ... do work ...

    // Complete it
    issue::complete_issue(&conn, &next.id)?;
}
```

### In-Memory Database (Testing)

```rust
use issues::{db, issue, Priority, IssueType};

// Create in-memory database (perfect for tests)
let conn = db::init_memory_db()?;

let issue = issue::create_issue(
    &conn,
    "Test issue",
    None,
    Priority::Medium,
    IssueType::Task,
    None, // defaults to "codex"
)?;

assert_eq!(issue.number, 1);
assert_eq!(issue.status, issue::Status::Open);
```

## API Reference

### Database Module (`db`)

#### `init_db(path: &Path) -> Result<Connection>`

Initialize database with migrations. Creates file if it doesn't exist.

```rust
let conn = db::init_db(Path::new("autopilot.db"))?;
```

**Schema versions:**
- v1: Initial schema (issues, events, counter)
- v2: Add NOT NULL constraint on issue.id, UNIQUE on number
- v3: Add trigger to sync counter on manual inserts
- v4: Add projects and sessions tables
- v5: Add agent column to issues

#### `init_memory_db() -> Result<Connection>`

Create in-memory database (for testing).

```rust
let conn = db::init_memory_db()?;
```

**Use cases:**
- Unit tests (fast, isolated)
- Temporary work queues
- CI/CD environments

#### `next_issue_number(conn: &Connection) -> Result<i32>`

Get next sequential issue number atomically.

```rust
let number = db::next_issue_number(&conn)?;
// Returns 1, 2, 3, ... (atomic increment)
```

**Implementation:** Uses `UPDATE ... RETURNING` for atomic increment. Trigger syncs counter if manual inserts bypass API.

### Issue Module (`issue`)

#### Types

**Status**
```rust
pub enum Status {
    Open,        // Ready to be claimed
    InProgress,  // Claimed by a run
    Done,        // Completed
}
```

**Priority**
```rust
pub enum Priority {
    Urgent,   // Returned first by get_next_ready_issue
    High,
    Medium,   // Default
    Low,
}
```

**IssueType**
```rust
pub enum IssueType {
    Task,     // Default
    Bug,
    Feature,
}
```

**Issue**
```rust
pub struct Issue {
    pub id: String,                     // UUID
    pub number: i32,                    // Sequential (1, 2, 3...)
    pub title: String,
    pub description: Option<String>,
    pub status: Status,
    pub priority: Priority,
    pub issue_type: IssueType,
    pub agent: String,                  // "codex" or "codex"
    pub is_blocked: bool,
    pub blocked_reason: Option<String>,
    pub claimed_by: Option<String>,     // run_id
    pub claimed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
```

#### CRUD Operations

**create_issue**
```rust
pub fn create_issue(
    conn: &Connection,
    title: &str,
    description: Option<&str>,
    priority: Priority,
    issue_type: IssueType,
    agent: Option<&str>, // defaults to "codex"
) -> Result<Issue>
```

Example:
```rust
let issue = issue::create_issue(
    &conn,
    "Add comprehensive README for issues crate",
    Some("Document database schema, API, state transitions..."),
    Priority::Medium,
    IssueType::Task,
    Some("codex"),
)?;
```

**get_issue_by_id**
```rust
pub fn get_issue_by_id(
    conn: &Connection,
    id: &str,
) -> Result<Option<Issue>>
```

**get_issue_by_number**
```rust
pub fn get_issue_by_number(
    conn: &Connection,
    number: i32,
) -> Result<Option<Issue>>
```

**list_issues**
```rust
pub fn list_issues(
    conn: &Connection,
    status: Option<Status>,
) -> Result<Vec<Issue>>
```

Examples:
```rust
// All issues
let all = issue::list_issues(&conn, None)?;

// Only open issues
let open = issue::list_issues(&conn, Some(Status::Open))?;

// Only completed
let done = issue::list_issues(&conn, Some(Status::Done))?;
```

**update_issue**
```rust
pub fn update_issue(
    conn: &Connection,
    issue_id: &str,
    title: Option<&str>,
    description: Option<&str>,
    priority: Option<Priority>,
    issue_type: Option<IssueType>,
) -> Result<bool>
```

Example:
```rust
issue::update_issue(
    &conn,
    &issue.id,
    Some("Updated title"),
    None,                     // keep existing description
    Some(Priority::Urgent),   // increase priority
    None,                     // keep existing type
)?;
```

**delete_issue**
```rust
pub fn delete_issue(
    conn: &Connection,
    issue_id: &str,
) -> Result<bool>
```

**IMPORTANT:** Hard delete. Also deletes related events. Use for cleanup/testing only.

#### Workflow Operations

**get_next_ready_issue**
```rust
pub fn get_next_ready_issue(
    conn: &Connection,
    agent: Option<&str>,
) -> Result<Option<Issue>>
```

Returns the highest-priority, oldest open issue that is:
- Status = `Open`
- Not blocked (`is_blocked = false`)
- Not claimed OR claim expired (>15 minutes)
- Matches agent filter (if provided)

**Priority ordering:**
1. Urgent (priority = 0)
2. High (priority = 1)
3. Medium (priority = 2)
4. Low (priority = 3)

Within same priority: oldest first (created_at ASC)

Examples:
```rust
// Get next issue for any agent
let next = issue::get_next_ready_issue(&conn, None)?;

// Get next issue for codex
let codex_next = issue::get_next_ready_issue(&conn, Some("codex"))?;

// Get next issue for codex
let codex_next = issue::get_next_ready_issue(&conn, Some("codex"))?;
```

**claim_issue**
```rust
pub fn claim_issue(
    conn: &Connection,
    issue_id: &str,
    run_id: &str,
) -> Result<bool>
```

Atomically claim an issue for a run. Returns `false` if:
- Issue not in `Open` status
- Issue is blocked
- Issue already claimed (within 15 minutes)

When successful:
- Sets status = `InProgress`
- Sets `claimed_by` = run_id
- Sets `claimed_at` = now
- Updates `updated_at`

Example:
```rust
if issue::claim_issue(&conn, &issue.id, "autopilot_main")? {
    println!("Claimed issue #{}", issue.number);
    // ... do work ...
} else {
    println!("Could not claim (already claimed or blocked)");
}
```

**unclaim_issue**
```rust
pub fn unclaim_issue(
    conn: &Connection,
    issue_id: &str,
) -> Result<bool>
```

Release claim without completing. Sets status back to `Open`.

**complete_issue**
```rust
pub fn complete_issue(
    conn: &Connection,
    issue_id: &str,
) -> Result<bool>
```

Mark issue as done:
- Sets status = `Done`
- Clears `claimed_by` and `claimed_at`
- Sets `completed_at` = now
- Updates `updated_at`

Example:
```rust
issue::complete_issue(&conn, &issue.id)?;
println!("Issue #{} completed!", issue.number);
```

**block_issue**
```rust
pub fn block_issue(
    conn: &Connection,
    issue_id: &str,
    reason: &str,
) -> Result<bool>
```

Block an issue with a reason:
- Sets `is_blocked` = true
- Sets `blocked_reason` = reason
- Sets status = `Open`
- Clears claim

Blocked issues won't be returned by `get_next_ready_issue`.

Example:
```rust
issue::block_issue(
    &conn,
    &issue.id,
    "Waiting for dependency: issue #15",
)?;
```

**unblock_issue**
```rust
pub fn unblock_issue(
    conn: &Connection,
    issue_id: &str,
) -> Result<bool>
```

Remove block:
- Sets `is_blocked` = false
- Clears `blocked_reason`

Issue becomes available in ready queue again.

### Project Module (`project`)

#### Types

**Project**
```rust
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub default_model: Option<String>,
    pub default_budget: Option<f64>,
    pub created_at: String,
    pub updated_at: String,
}
```

#### Operations

**create_project**
```rust
pub fn create_project(
    conn: &Connection,
    name: &str,
    path: &str,
    description: Option<&str>,
    default_model: Option<&str>,
    default_budget: Option<f64>,
) -> Result<Project>
```

Example:
```rust
let project = project::create_project(
    &conn,
    "openagents",
    "/home/user/code/openagents",
    Some("OpenAgents desktop foundation"),
    Some("sonnet"),
    Some(5.0),
)?;
```

**list_projects**
```rust
pub fn list_projects(
    conn: &Connection,
) -> Result<Vec<Project>>
```

**get_project_by_name**
```rust
pub fn get_project_by_name(
    conn: &Connection,
    name: &str,
) -> Result<Option<Project>>
```

**get_project_by_id**
```rust
pub fn get_project_by_id(
    conn: &Connection,
    id: &str,
) -> Result<Option<Project>>
```

**delete_project**
```rust
pub fn delete_project(
    conn: &Connection,
    id: &str,
) -> Result<bool>
```

**IMPORTANT:** Cascade deletes all sessions for this project.

### Session Module (`session`)

#### Types

**SessionStatus**
```rust
pub enum SessionStatus {
    Running,    // Currently active
    Completed,  // Finished successfully
    Failed,     // Crashed or errored
    Cancelled,  // Manually stopped
}
```

**Session**
```rust
pub struct Session {
    pub id: String,
    pub project_id: String,
    pub status: SessionStatus,
    pub prompt: String,
    pub model: String,
    pub pid: Option<i32>,
    pub trajectory_path: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub budget_spent: f64,
    pub issues_completed: i32,
}
```

#### Operations

**create_session**
```rust
pub fn create_session(
    conn: &Connection,
    project_id: &str,
    prompt: &str,
    model: &str,
    pid: Option<i32>,
) -> Result<Session>
```

Example:
```rust
let session = session::create_session(
    &conn,
    &project.id,
    "FULL AUTO MODE: infinite autopilot loop",
    "sonnet",
    Some(std::process::id() as i32),
)?;
```

**update_session_status**
```rust
pub fn update_session_status(
    conn: &Connection,
    session_id: &str,
    status: SessionStatus,
) -> Result<bool>
```

Sets status and `ended_at`.

**update_session_trajectory**
```rust
pub fn update_session_trajectory(
    conn: &Connection,
    session_id: &str,
    trajectory_path: &str,
) -> Result<bool>
```

**update_session_metrics**
```rust
pub fn update_session_metrics(
    conn: &Connection,
    session_id: &str,
    budget_spent: f64,
    issues_completed: i32,
) -> Result<bool>
```

Example:
```rust
session::update_session_metrics(&conn, &session.id, 2.5, 10)?;
```

**list_sessions**
```rust
pub fn list_sessions(
    conn: &Connection,
    project_id: Option<&str>,
) -> Result<Vec<Session>>
```

**get_session**
```rust
pub fn get_session(
    conn: &Connection,
    session_id: &str,
) -> Result<Option<Session>>
```

**get_active_sessions**
```rust
pub fn get_active_sessions(
    conn: &Connection,
) -> Result<Vec<Session>>
```

Returns all sessions with status = `Running`.

## Database Schema

### Issues Table

```sql
CREATE TABLE issues (
    id TEXT NOT NULL PRIMARY KEY,
    number INTEGER NOT NULL UNIQUE,
    title TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'open',
    priority TEXT DEFAULT 'medium',
    issue_type TEXT DEFAULT 'task',
    agent TEXT NOT NULL DEFAULT 'codex',
    is_blocked INTEGER DEFAULT 0,
    blocked_reason TEXT,
    claimed_by TEXT,
    claimed_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    CHECK(id != ''),
    CHECK(status IN ('open', 'in_progress', 'done'))
);

CREATE INDEX idx_issues_status ON issues(status);
CREATE INDEX idx_issues_number ON issues(number);
```

### Issue Events Table

Audit log for issue state changes.

```sql
CREATE TABLE issue_events (
    id TEXT PRIMARY KEY,
    issue_id TEXT NOT NULL REFERENCES issues(id),
    actor TEXT,
    event_type TEXT NOT NULL,
    old_value TEXT,
    new_value TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_issue_events_issue ON issue_events(issue_id);
```

**Note:** Event logging not yet implemented in API. Table exists for future audit trail.

### Issue Counter Table

```sql
CREATE TABLE issue_counter (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    next_number INTEGER NOT NULL DEFAULT 1
);

INSERT OR IGNORE INTO issue_counter (id, next_number) VALUES (1, 1);
```

**Auto-sync trigger:**
```sql
CREATE TRIGGER sync_issue_counter_on_insert
AFTER INSERT ON issues
BEGIN
    UPDATE issue_counter
    SET next_number = MAX(next_number, NEW.number + 1)
    WHERE id = 1;
END;
```

This ensures counter stays synced even if someone manually inserts with `sqlite3` (bypassing API).

### Projects Table

```sql
CREATE TABLE projects (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    path TEXT NOT NULL,
    description TEXT,
    default_model TEXT,
    default_budget REAL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK(id != ''),
    CHECK(name != '')
);

CREATE INDEX idx_projects_name ON projects(name);
```

### Sessions Table

```sql
CREATE TABLE sessions (
    id TEXT NOT NULL PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'running',
    prompt TEXT NOT NULL,
    model TEXT NOT NULL,
    pid INTEGER,
    trajectory_path TEXT,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    budget_spent REAL DEFAULT 0.0,
    issues_completed INTEGER DEFAULT 0,
    CHECK(id != ''),
    CHECK(status IN ('running', 'completed', 'failed', 'cancelled'))
);

CREATE INDEX idx_sessions_project ON sessions(project_id);
CREATE INDEX idx_sessions_status ON sessions(status);
CREATE INDEX idx_sessions_started ON sessions(started_at);
```

### Schema Version Table

```sql
CREATE TABLE schema_version (
    version INTEGER NOT NULL
);
```

Current version: **5**

## State Transitions

### Issue Lifecycle

```
         create_issue
              ↓
         [Open: status=open, not claimed]
              ↓
         claim_issue (run_id)
              ↓
         [InProgress: status=in_progress, claimed_by=run_id]
              ↓
    complete_issue
              ↓
         [Done: status=done, completed_at set]


Alternative paths:

[Open] → block_issue → [Open: is_blocked=1]
[Open: is_blocked=1] → unblock_issue → [Open: is_blocked=0]

[InProgress] → unclaim_issue → [Open: claimed_by cleared]

[InProgress] → block_issue → [Open: is_blocked=1, claim cleared]
```

### Session Lifecycle

```
         create_session
              ↓
         [Running: status=running, started_at set]
              ↓
    update_session_metrics (track progress)
              ↓
    update_session_status(Completed/Failed/Cancelled)
              ↓
         [Terminal: ended_at set]
```

## Cross-Machine Sync

The `issues` crate supports syncing issues between machines via JSON export/import.

### Quick Sync Workflow

```bash
# On machine A: Export issues to JSON
cargo autopilot issue export
# Creates .openagents/issues.json (tracked in git)

# Commit and push
git add .openagents/issues.json
git commit -m "Sync issues"
git push

# On machine B: Pull and import
git pull
cargo autopilot issue import
```

### Export Command

```bash
# Export open issues (default)
cargo autopilot issue export

# Include completed issues
cargo autopilot issue export --include-completed

# Custom output path
cargo autopilot issue export -o /tmp/my-issues.json
```

Exports all issues to JSON, preserving:
- UUIDs (for deduplication)
- Issue numbers
- All metadata (priority, status, agent, etc.)
- Timestamps

### Import Command

```bash
# Import from default path (.openagents/issues.json)
cargo autopilot issue import

# Import from custom path
cargo autopilot issue import -i /tmp/my-issues.json

# Force update existing issues
cargo autopilot issue import --force
```

Import behavior:
- **By default**: Skips issues with existing UUIDs (no duplicates)
- **With `--force`**: Updates existing issues with imported data
- **Counter sync**: Automatically updates issue counter if imported numbers are higher
- **UUID preservation**: Maintains issue identity across machines

### Use Cases

**Scenario 1: Work on issue on laptop, continue on desktop**
```bash
# Laptop: Export after working on issues
cargo autopilot issue export
git add .openagents/issues.json && git commit -m "Sync" && git push

# Desktop: Import to get latest state
git pull
cargo autopilot issue import
```

**Scenario 2: Share issue backlog with team**
```bash
# Team member creates issues
cargo autopilot issue create "Implement feature X"
cargo autopilot issue export
git push

# You import their issues
git pull
cargo autopilot issue import
```

**Scenario 3: Backup and restore**
```bash
# Backup
cargo autopilot issue export --include-completed -o backup.json

# Restore (on new machine or after db corruption)
cargo autopilot issue import -i backup.json --force
```

See the [main README](../../README.md#issue-management) for more details.

## Autopilot Integration

The issues crate is the **storage layer** for the autopilot system:

### Autopilot Loop

```rust
loop {
    // 1. Get next issue
    let Some(issue) = issue::get_next_ready_issue(&conn, Some("codex"))? else {
        // No issues available - analyze codebase and create new one
        let new_issue = issue::create_issue(&conn, ...)?;
        continue;
    };

    // 2. Claim it
    if !issue::claim_issue(&conn, &issue.id, "autopilot_main")? {
        continue; // Already claimed or blocked
    }

    // 3. Implement solution
    // ... read files, write code, run tests, commit, push ...

    // 4. Complete it
    issue::complete_issue(&conn, &issue.id)?;

    // 5. Update session metrics
    session::update_session_metrics(&conn, &session_id, budget, completed_count)?;

    // 6. IMMEDIATELY continue (no pause)
}
```

### FULL AUTO MODE Rules

From `crates/autopilot-core/README.md`:

- **NEVER stop** - Continue loop until budget exhausted
- **NEVER output summaries** - "I've completed X issues" is a STOP SIGNAL
- **ALWAYS push** - After each commit: `git push origin main`
- **Create issues when queue empty** - If `get_next_ready_issue` returns None
- **IMMEDIATE continuation** - After `complete_issue`, VERY NEXT action is `get_next_ready_issue`

### Example Autopilot Session

```rust
// Initialize
let conn = db::init_db(Path::new("autopilot.db"))?;
let project = project::get_project_by_name(&conn, "openagents")?.unwrap();
let session = session::create_session(&conn, &project.id, "FULL AUTO", "sonnet", None)?;

let mut completed = 0;
let mut budget = 0.0;

loop {
    // Get next ready issue
    let Some(next) = issue::get_next_ready_issue(&conn, Some("codex"))? else {
        // Create new issue
        let new_issue = issue::create_issue(
            &conn,
            "Add comprehensive README for foo crate",
            None,
            Priority::Medium,
            IssueType::Task,
            Some("codex"),
        )?;
        continue;
    };

    // Claim issue
    if !issue::claim_issue(&conn, &next.id, &session.id)? {
        continue;
    }

    // ... implement solution ...

    // Complete issue
    issue::complete_issue(&conn, &next.id)?;
    completed += 1;
    budget += 0.15; // Track costs

    // Update session metrics
    session::update_session_metrics(&conn, &session.id, budget, completed)?;

    // IMMEDIATELY continue (no pause)
}
```

## Testing

All modules have comprehensive unit tests.

### Running Tests

```bash
# All tests
cargo test -p issues

# Specific module
cargo test -p issues --lib db
cargo test -p issues --lib issue
cargo test -p issues --lib project
cargo test -p issues --lib session

# Integration tests
cargo test -p issues --test integration
```

### Test Coverage

**db module:**
- Schema initialization
- Migrations (v1 → v5)
- Counter increment
- Counter auto-resync on manual inserts
- Unique constraints
- NOT NULL constraints

**issue module:**
- Create and get issues
- Agent filtering
- Priority ordering
- Claim/unclaim/complete
- Block/unblock
- Delete with cascade
- Update fields
- Ready queue logic

**project module:**
- Create and list projects
- Get by name/id
- Delete with cascade
- Unique name constraint

**session module:**
- Create and list sessions
- Update status/trajectory/metrics
- Get active sessions
- Cascade delete on project delete

**integration tests (crates/issues/tests/integration.rs):**
- Full lifecycle (create → claim → complete)
- State transitions
- Priority-based queue
- Blocked issue filtering
- Claim expiration (15 minutes)
- Concurrent claims
- Delete operations

### Example Test

```rust
#[test]
fn test_claim_and_complete() {
    let conn = init_memory_db().unwrap();
    let issue = create_issue(&conn, "Task", None, Priority::Medium, IssueType::Task, None).unwrap();

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
```

## Error Handling

All functions return `rusqlite::Result<T>`.

Common errors:
- `QueryReturnedNoRows` - Issue/project/session not found
- `SqliteFailure` - Constraint violation (e.g., duplicate number, foreign key)
- `InvalidColumnType` - Data corruption
- `DatabaseBusy` - Concurrent write conflict (rare with WAL mode)

Example error handling:

```rust
match issue::create_issue(&conn, title, None, Priority::High, IssueType::Bug, None) {
    Ok(issue) => println!("Created issue #{}", issue.number),
    Err(rusqlite::Error::SqliteFailure(err, msg)) => {
        eprintln!("Database error: {:?} - {:?}", err, msg);
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## Performance

Characteristics:
- **Database size**: ~100KB for 1000 issues
- **Create issue**: <1ms (atomic counter increment)
- **Get next ready**: <5ms (indexed query on status + priority)
- **List issues**: ~1ms per 100 issues
- **Claim issue**: <2ms (single UPDATE with WHERE clause)
- **Complete issue**: <2ms (single UPDATE)

**Indexes:**
- `idx_issues_status` - Fast filtering by status
- `idx_issues_number` - Fast lookups by issue number
- `idx_projects_name` - Fast project lookups
- `idx_sessions_project` - Fast session listing per project
- `idx_sessions_status` - Fast active session queries

**Concurrent access:**
- SQLite uses WAL mode (write-ahead logging)
- Multiple readers + single writer
- Busy timeout: 5000ms (default)

For high-concurrency scenarios (e.g., 10+ agents), consider PostgreSQL migration.

## Database Management

### CRITICAL: Always Use API, Not Raw SQL

**NEVER use raw sqlite3 commands to insert or modify data:**

```bash
# WRONG - bypasses counters and triggers
sqlite3 autopilot.db "INSERT INTO issues ..."

# RIGHT - use the API
cargo autopilot issue create --title "..."
```

Direct SQL bypasses:
- Counter increment
- Timestamp generation
- UUID generation
- Event logging (future)

**Read-only queries are fine:**
```bash
sqlite3 autopilot.db "SELECT * FROM issues WHERE status = 'done'"
```

### Viewing Data

```bash
# Open database
sqlite3 autopilot.db

# List all issues
sqlite> SELECT number, title, status, priority FROM issues ORDER BY number;

# Show issue details
sqlite> SELECT * FROM issues WHERE number = 23;

# Count by status
sqlite> SELECT status, COUNT(*) FROM issues GROUP BY status;

# Show projects
sqlite> SELECT name, path, default_model FROM projects;

# Show active sessions
sqlite> SELECT id, prompt, model, budget_spent, issues_completed FROM sessions WHERE status = 'running';
```

### Backup

```bash
# Backup database
sqlite3 autopilot.db ".backup autopilot-backup.db"

# Restore from backup
cp autopilot-backup.db autopilot.db
```

### Migrations

The crate automatically runs migrations on `init_db()` or `init_memory_db()`.

**Migration history:**
- **v1** (initial): Issues, events, counter tables
- **v2**: Add NOT NULL + UNIQUE constraints
- **v3**: Add auto-sync trigger for counter
- **v4**: Add projects and sessions tables
- **v5**: Add agent column to issues

To manually check schema version:
```bash
sqlite3 autopilot.db "SELECT version FROM schema_version"
```

## Troubleshooting

### Issue number mismatch

**Symptom:** `next_issue_number` returns wrong value

**Cause:** Manual SQL insert bypassed counter

**Fix:** Counter auto-syncs via trigger. Create new issue normally and counter will fix itself.

### Unique constraint violation

**Symptom:** `UNIQUE constraint failed: issues.number`

**Cause:** Concurrent writes or corrupted counter

**Fix:**
```bash
sqlite3 autopilot.db "UPDATE issue_counter SET next_number = (SELECT COALESCE(MAX(number) + 1, 1) FROM issues) WHERE id = 1"
```

### Foreign key constraint violation

**Symptom:** `FOREIGN KEY constraint failed`

**Cause:** Creating session for non-existent project

**Fix:** Ensure project exists before creating session:
```rust
if project::get_project_by_id(&conn, project_id)?.is_none() {
    return Err(Error::InvalidProjectId);
}
```

### Database locked

**Symptom:** `database is locked`

**Cause:** Long-running transaction or multiple writers

**Fix:**
- Use short transactions
- Enable WAL mode: `PRAGMA journal_mode=WAL`
- Increase busy timeout: `conn.busy_timeout(Duration::from_secs(10))?`

### Claim expiration not working

**Symptom:** Claims don't expire after 15 minutes

**Cause:** System clock issue or datetime comparison error

**Fix:** Verify `claimed_at` is RFC3339 format:
```bash
sqlite3 autopilot.db "SELECT number, claimed_at FROM issues WHERE claimed_by IS NOT NULL"
```

Should show: `2025-12-20T10:30:00Z` (not epoch or local time)

## Future Work

- [ ] Event logging API (populate `issue_events` table)
- [ ] Issue labels/tags
- [ ] Issue dependencies (blocked_by field)
- [ ] Full-text search on title/description
- [ ] Batch operations (claim multiple issues)
- [ ] PostgreSQL backend option
- [ ] GraphQL query layer
- [ ] WebSocket notifications on state changes
- [ ] Metrics/telemetry (claim latency, completion rate)

## Related Documentation

- **MCP Server**: `crates/issues-mcp/README.md` - JSON-RPC interface
- **Autopilot System**: `crates/autopilot-core/README.md` - FULL AUTO MODE workflow
- **CLI**: `crates/autopilot-core/README.md` - `autopilot issue` commands

## License

Same as the OpenAgents workspace (MIT).
