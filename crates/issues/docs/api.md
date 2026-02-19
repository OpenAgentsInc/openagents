# API Reference

Complete reference for all public functions in the issues crate.

## Database Module (`db`)

### init_db

Initialize a database at the given path, running any pending migrations.

```rust
use issues::db::init_db;
use std::path::Path;

let conn = init_db(Path::new("autopilot.db"))?;
```

Creates the database file if it doesn't exist. Safe to call on an existing database - only runs migrations for versions not yet applied.

### init_memory_db

Create an in-memory database for testing.

```rust
use issues::db::init_memory_db;

let conn = init_memory_db()?;
// Database exists only for lifetime of `conn`
```

### next_issue_number

Get the next sequential issue number atomically.

```rust
use issues::db::next_issue_number;

let num = next_issue_number(&conn)?;
assert_eq!(num, 1);

let num2 = next_issue_number(&conn)?;
assert_eq!(num2, 2);
```

This is used internally by `create_issue` but exposed for custom workflows.

---

## Issue Module (`issue`)

### Types

#### Status

```rust
pub enum Status {
    Open,       // Available for work
    InProgress, // Currently being worked on
    Done,       // Completed
}
```

#### Priority

```rust
pub enum Priority {
    Urgent, // Highest priority
    High,
    Medium, // Default
    Low,    // Lowest priority
}
```

#### IssueType

```rust
pub enum IssueType {
    Task,    // Default, general work item
    Bug,     // Something broken
    Feature, // New functionality
}
```

#### Issue

```rust
pub struct Issue {
    pub id: String,                        // UUID
    pub number: i32,                       // Sequential number
    pub title: String,
    pub description: Option<String>,
    pub status: Status,
    pub priority: Priority,
    pub issue_type: IssueType,
    pub is_blocked: bool,
    pub blocked_reason: Option<String>,
    pub claimed_by: Option<String>,        // Run ID
    pub claimed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
```

---

### Creating Issues

#### create_issue

Create a new issue with the given details.

```rust
use issues::issue::{create_issue, Priority, IssueType};

let issue = create_issue(
    &conn,
    "Implement dark mode",           // title (required)
    Some("Add theme toggle to UI"),  // description (optional)
    Priority::Medium,                // priority
    IssueType::Feature,              // type
)?;

println!("Created #{}: {}", issue.number, issue.title);
```

Returns the created `Issue` with all fields populated including `id`, `number`, and timestamps.

---

### Reading Issues

#### get_issue_by_id

Fetch an issue by its UUID.

```rust
use issues::issue::get_issue_by_id;

if let Some(issue) = get_issue_by_id(&conn, "550e8400-e29b-41d4-a716-446655440000")? {
    println!("Found: {}", issue.title);
} else {
    println!("Not found");
}
```

#### get_issue_by_number

Fetch an issue by its sequential number.

```rust
use issues::issue::get_issue_by_number;

if let Some(issue) = get_issue_by_number(&conn, 42)? {
    println!("Issue #42: {}", issue.title);
}
```

#### list_issues

List all issues, optionally filtered by status.

```rust
use issues::issue::{list_issues, Status};

// All issues
let all = list_issues(&conn, None)?;

// Only open issues
let open = list_issues(&conn, Some(Status::Open))?;

// Only completed
let done = list_issues(&conn, Some(Status::Done))?;

for issue in open {
    println!("#{} [{}] {}", issue.number, issue.priority.as_str(), issue.title);
}
```

#### get_next_ready_issue

Get the highest-priority issue that is ready to work on.

```rust
use issues::issue::get_next_ready_issue;

if let Some(issue) = get_next_ready_issue(&conn)? {
    println!("Next up: #{} {}", issue.number, issue.title);
}
```

An issue is "ready" if:
- Status is `open`
- Not blocked (`is_blocked = 0`)
- Not claimed, OR claim expired (older than 15 minutes)

Issues are returned in priority order (urgent first), then by creation time (oldest first).

---

### Claiming Issues

#### claim_issue

Claim an issue for a run. Returns `true` if claim succeeded.

```rust
use issues::issue::claim_issue;

let run_id = "run-abc123";

if claim_issue(&conn, &issue.id, run_id)? {
    println!("Claimed issue #{}", issue.number);
} else {
    println!("Could not claim (already claimed or blocked)");
}
```

Claiming:
- Sets `status` to `in_progress`
- Sets `claimed_by` to the run ID
- Sets `claimed_at` to current time

Claim fails if issue is blocked or already claimed (within 15 min).

#### unclaim_issue

Release a claim without completing the issue.

```rust
use issues::issue::unclaim_issue;

unclaim_issue(&conn, &issue.id)?;
// Issue returns to 'open' status
```

Use this when work is interrupted or the issue should be returned to the queue.

---

### Completing Issues

#### complete_issue

Mark an issue as done.

```rust
use issues::issue::complete_issue;

complete_issue(&conn, &issue.id)?;
```

This:
- Sets `status` to `done`
- Clears `claimed_by` and `claimed_at`
- Sets `completed_at` to current time

---

### Blocking Issues

#### block_issue

Block an issue with a reason.

```rust
use issues::issue::block_issue;

block_issue(&conn, &issue.id, "Waiting for API credentials")?;
```

Blocked issues:
- Are excluded from `get_next_ready_issue`
- Have any existing claim released
- Return to `open` status

#### unblock_issue

Remove the block from an issue.

```rust
use issues::issue::unblock_issue;

unblock_issue(&conn, &issue.id)?;
// Issue is now available in the ready queue again
```

---

### Updating Issues

#### update_issue

Update the title and/or description.

```rust
use issues::issue::update_issue;

// Update title only
update_issue(&conn, &issue.id, Some("New title"), None)?;

// Update description only
update_issue(&conn, &issue.id, None, Some("New description"))?;

// Update both
update_issue(&conn, &issue.id, Some("New title"), Some("New description"))?;
```

Returns `true` if the issue was found and updated.

---

### Deleting Issues

#### delete_issue

Permanently delete an issue and its events.

```rust
use issues::issue::delete_issue;

if delete_issue(&conn, &issue.id)? {
    println!("Issue deleted");
}
```

This is a hard delete - the issue and all related `issue_events` are removed.

---

## Re-exports

For convenience, commonly used types are re-exported at the crate root:

```rust
use issues::{
    // Database
    init_db,
    init_memory_db,

    // Types
    Issue,
    IssueType,
    Priority,
    Status,
};
```
