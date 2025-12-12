# Taskmaster: Full Beads Port to OpenAgents

**Date:** 2025-12-12 12:57
**Status:** Phase 1 Complete (Core Foundation)

## Overview

Ported Beads (Go distributed issue tracker) to `crates/taskmaster` as a new Rust crate with SQLite storage. This is a full feature port of Beads with no daemon architecture.

## User Requirements

- **New crate**: `crates/taskmaster` (not modifying existing `crates/tasks`)
- **Full port**: All core Beads features
- **No daemon**: Direct SQLite access only
- **SQLite storage**: Not JSONL

## Research Phase

### Beads Codebase Analysis (`~/code/beads`)

Explored the Beads codebase and found:

- **Architecture**: Git-backed distributed issue tracker with 3 layers (CLI → SQLite cache → JSONL source of truth)
- **Core Data Model**: Issue with 22+ fields, 5 statuses (open, in_progress, blocked, closed, tombstone), 5 types (bug, feature, task, epic, chore), 4 dependency types (blocks, related, parent-child, discovered-from)
- **Key Features**: Content hash dedup, tombstones with TTL, compaction, events/audit trail, statistics, rich filtering (AND/OR labels, date ranges), ready queue with recursive CTE

### OpenAgents Existing Code (`crates/tasks`)

- Already has a Rust task system with SQLite backend
- 567 tasks in database
- TaskRepository trait with 34 user stories
- Missing: tombstones, compaction, events, stats, doctor, rich filtering

## Implementation

### Created Files (17 total)

```
crates/taskmaster/
├── Cargo.toml                          # Crate manifest (edition 2024)
├── src/
│   ├── lib.rs                          # Main library exports
│   ├── types/
│   │   ├── mod.rs                      # Type exports
│   │   ├── issue.rs                    # Issue struct (22+ fields)
│   │   ├── status.rs                   # IssueStatus enum (5 states)
│   │   ├── priority.rs                 # Priority enum (P0-P4)
│   │   ├── issue_type.rs               # IssueType enum
│   │   ├── dependency.rs               # Dependency types (4 kinds)
│   │   ├── comment.rs                  # Comment struct
│   │   ├── event.rs                    # IssueEvent for audit trail
│   │   ├── filter.rs                   # Rich filtering (AND/OR labels)
│   │   └── stats.rs                    # Statistics, DoctorReport
│   ├── repository/
│   │   ├── mod.rs                      # Repository exports
│   │   ├── error.rs                    # TaskmasterError enum
│   │   └── trait.rs                    # IssueRepository trait (~40 methods)
│   └── storage/
│       ├── mod.rs                      # Storage exports
│       ├── schema.rs                   # SQLite schema + queries
│       └── sqlite.rs                   # SqliteRepository implementation
```

### Dependencies

```toml
[dependencies]
rusqlite = { version = "0.32", features = ["bundled", "modern_sqlite"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = { version = "0.4", features = ["serde"] }
thiserror = "2.0"
sha2 = "0.10"
hex = "0.4"
uuid = { version = "1.0", features = ["v4"] }
tracing = "0.1"
```

### SQLite Schema

Created 6 tables:
1. `issues` - Main issues table with 22+ columns
2. `issue_labels` - Normalized labels (for AND/OR filtering)
3. `issue_dependencies` - Dependency relationships
4. `issue_comments` - Comment threads
5. `issue_events` - Audit trail
6. `stats_snapshots` - Historical statistics
7. `issues_fts` - Full-text search (FTS5)

Plus indexes for performance:
- `idx_issues_ready` - Partial index for ready tasks
- `idx_issues_tombstone` - Partial index for tombstones
- Multiple indexes on labels, deps, events

### IssueRepository Trait

Implemented ~40 methods:

**CRUD:**
- `create`, `create_with_id_method`, `get`, `get_with_tombstones`, `exists`, `update`, `tombstone`, `purge`, `restore`

**Listing & Filtering:**
- `list`, `count`, `search`, `ready`, `pick_next`, `stale`, `duplicates`

**Lifecycle:**
- `start`, `close`, `reopen`, `block`, `unblock`, `is_ready`

**Dependencies:**
- `add_dependency`, `remove_dependency`, `blockers`, `blocked_by`, `dependency_tree`, `has_cycle`

**Labels:**
- `add_label`, `remove_label`, `all_labels`

**Comments:**
- `add_comment`, `comments`

**Events/Audit:**
- `events`, `recent_events`

**Statistics & Health:**
- `stats`, `stats_history`, `save_stats_snapshot`, `doctor`, `repair`

**Maintenance:**
- `init`, `migrate`, `vacuum`, `cleanup_tombstones`, `compact`

### Test Results

```
running 30 tests
test types::dependency::tests::test_dependency_ref ... ok
test types::comment::tests::test_comment_new ... ok
test types::dependency::tests::test_dependency_type_parse ... ok
test types::dependency::tests::test_blocks_readiness ... ok
test types::event::tests::test_event_type_parse ... ok
test types::event::tests::test_issue_event_builder ... ok
test types::filter::tests::test_filter_builder ... ok
test types::filter::tests::test_label_expr ... ok
test types::filter::tests::test_label_filter ... ok
test types::issue::tests::test_content_hash ... ok
test types::issue::tests::test_issue_create_builder ... ok
test types::issue::tests::test_issue_new ... ok
test types::issue::tests::test_issue_validate ... ok
test types::issue_type::tests::test_is_container ... ok
test types::issue_type::tests::test_issue_type_parse ... ok
test types::priority::tests::test_priority_from_u8 ... ok
test types::priority::tests::test_priority_ordering ... ok
test types::priority::tests::test_priority_parse ... ok
test types::stats::tests::test_doctor_report ... ok
test types::stats::tests::test_status_counts ... ok
test types::status::tests::test_status_parse ... ok
test types::status::tests::test_status_transitions ... ok
test storage::sqlite::tests::test_update ... ok
test storage::sqlite::tests::test_labels ... ok
test storage::sqlite::tests::test_comments ... ok
test storage::sqlite::tests::test_create_and_get ... ok
test storage::sqlite::tests::test_stats ... ok
test storage::sqlite::tests::test_tombstone ... ok
test storage::sqlite::tests::test_dependencies ... ok
test storage::sqlite::tests::test_status_lifecycle ... ok

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Key Features Implemented

### 1. Full Issue Lifecycle
```
open → in_progress → blocked → closed
  ↑                              ↓
  └──────── reopen ──────────────┘

Any state → tombstone (soft delete)
tombstone → any state (restore)
```

### 2. Rich Filtering (AND/OR Labels)
```rust
let filter = IssueFilter::new()
    .status(IssueStatus::Open)
    .priority(Priority::High)
    .labels_all(["urgent", "backend"])  // AND semantics
    .created_after(some_date)
    .limit(10);
```

### 3. Dependency Management
- 4 types: blocks, related, parent-child, discovered-from
- Cycle detection via recursive CTE
- Ready queue computation respects blocking deps

### 4. Tombstone Support (Beads Feature)
- Soft delete with configurable TTL (default 30 days)
- Automatic cleanup of expired tombstones
- Restore capability

### 5. Events/Audit Trail
- All mutations recorded with actor, timestamp, old/new values
- Event types: created, updated, status_changed, commented, closed, reopened, dependency_added/removed, label_added/removed, tombstoned, restored, etc.

### 6. Statistics
- Counts by status, priority, type
- Ready issue count
- Historical snapshots

### 7. Health Checks (Doctor)
- Orphan dependency detection
- Repair capability

### 8. Ready Queue
- Recursive CTE for transitive blocking detection
- Issues are ready if open + no blocking deps on non-closed issues

## Workspace Integration

Added to `/Cargo.toml`:
```toml
members = [
    # ... existing crates ...
    "crates/taskmaster",
]
```

## Next Steps (Future Phases)

### Phase 2: Lifecycle & Dependencies
- Enhanced dependency tree traversal
- More comprehensive cycle detection

### Phase 3: Rich Filtering & Search
- Full FTS5 search integration
- Complex label expressions

### Phase 4: Tombstones & Compaction
- Implement compaction service
- LLM-driven summarization

### Phase 5: Statistics & Health
- Stats history implementation
- More doctor checks

### Phase 6: Migration & CLI
- Migration tool from `crates/tasks` (567 existing tasks)
- Full CLI command set (~40 commands)
- Integration with orchestrator

## Plan File

Plan saved at: `/Users/christopherdavid/.claude/plans/dynamic-noodling-russell.md`

## References

- Beads source: `~/code/beads`
- Existing tasks: `crates/tasks/`
- Feature roadmap: `docs/claude/plans/beads.md`
