# Database Schema

The issues crate uses SQLite with a versioned migration system. Current schema version: **1**.

## Tables

### issues

The core issue tracking table.

| Column | Type | Default | Description |
|--------|------|---------|-------------|
| `id` | TEXT | - | UUID primary key |
| `number` | INTEGER | - | Sequential issue number (human-friendly) |
| `title` | TEXT | - | Issue title (required) |
| `description` | TEXT | NULL | Detailed description |
| `status` | TEXT | 'open' | Current status: `open`, `in_progress`, `done` |
| `priority` | TEXT | 'medium' | Priority: `urgent`, `high`, `medium`, `low` |
| `issue_type` | TEXT | 'task' | Type: `task`, `bug`, `feature` |
| `is_blocked` | INTEGER | 0 | Boolean flag (0 or 1) |
| `blocked_reason` | TEXT | NULL | Why the issue is blocked |
| `claimed_by` | TEXT | NULL | Run ID that claimed this issue |
| `claimed_at` | TEXT | NULL | ISO 8601 timestamp of claim |
| `created_at` | TEXT | - | ISO 8601 creation timestamp |
| `updated_at` | TEXT | - | ISO 8601 last update timestamp |
| `completed_at` | TEXT | NULL | ISO 8601 completion timestamp |

**Indexes:**
- `idx_issues_status` on `status`
- `idx_issues_number` on `number`

### issue_events

Audit log for all issue changes.

| Column | Type | Default | Description |
|--------|------|---------|-------------|
| `id` | TEXT | - | UUID primary key |
| `issue_id` | TEXT | - | Foreign key to issues.id |
| `actor` | TEXT | NULL | Who/what made the change (run ID, user, etc.) |
| `event_type` | TEXT | - | Event type (see below) |
| `old_value` | TEXT | NULL | Previous value (JSON or plain text) |
| `new_value` | TEXT | NULL | New value (JSON or plain text) |
| `created_at` | TEXT | - | ISO 8601 timestamp |

**Indexes:**
- `idx_issue_events_issue` on `issue_id`

**Event types:**
- `created` - Issue was created
- `claimed` - Issue was claimed by a run
- `unclaimed` - Claim was released
- `completed` - Issue marked done
- `blocked` - Issue was blocked
- `unblocked` - Issue was unblocked
- `updated` - Title or description changed
- `priority_changed` - Priority was updated
- `status_changed` - Status was changed

### issue_counter

Atomic counter for sequential issue numbering.

| Column | Type | Default | Description |
|--------|------|---------|-------------|
| `id` | INTEGER | - | Primary key, constrained to 1 |
| `next_number` | INTEGER | 1 | Next issue number to assign |

### schema_version

Tracks database migration version.

| Column | Type | Description |
|--------|------|-------------|
| `version` | INTEGER | Current schema version |

## SQL Definitions

```sql
-- Issues table
CREATE TABLE issues (
    id TEXT PRIMARY KEY,
    number INTEGER NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'open',
    priority TEXT DEFAULT 'medium',
    issue_type TEXT DEFAULT 'task',
    is_blocked INTEGER DEFAULT 0,
    blocked_reason TEXT,
    claimed_by TEXT,
    claimed_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT
);

CREATE INDEX idx_issues_status ON issues(status);
CREATE INDEX idx_issues_number ON issues(number);

-- Issue events (audit log)
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

-- Issue counter for sequential numbering
CREATE TABLE issue_counter (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    next_number INTEGER NOT NULL DEFAULT 1
);
```

## Status State Machine

```
     ┌─────────────────────────────────────┐
     │                                     │
     ▼                                     │
  ┌──────┐    claim     ┌─────────────┐    │ unclaim
  │ open │ ──────────▶  │ in_progress │ ───┘
  └──────┘              └─────────────┘
     │                        │
     │      complete          │ complete
     │    (skip in_progress)  │
     │                        ▼
     └──────────────────▶ ┌──────┐
                          │ done │
                          └──────┘
```

## Priority Ordering

When selecting the next ready issue, priority determines order:

1. `urgent` (0) - Drop everything
2. `high` (1) - Important
3. `medium` (2) - Normal (default)
4. `low` (3) - When time permits

Within the same priority, issues are ordered by `created_at` (oldest first).

## Claim Expiration

Claims expire after **15 minutes** of inactivity. This prevents abandoned claims from blocking work.

The `get_next_ready_issue` query considers an issue claimable if:
- `claimed_by IS NULL`, OR
- `claimed_at < datetime('now', '-15 minutes')`

To maintain a claim during long-running work, periodically update `claimed_at`:

```sql
UPDATE issues SET claimed_at = datetime('now') WHERE id = ?
```

## Blocking

Blocked issues are excluded from the ready queue regardless of status or claim state.

To block an issue:
```sql
UPDATE issues SET
    is_blocked = 1,
    blocked_reason = 'Waiting for API access',
    status = 'open',
    claimed_by = NULL
WHERE id = ?
```

To unblock:
```sql
UPDATE issues SET
    is_blocked = 0,
    blocked_reason = NULL
WHERE id = ?
```
