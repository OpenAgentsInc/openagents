# Issues Crate Documentation

- **Status:** Accurate
- **Last verified:** (see commit)
- **Source of truth:** terminology → [GLOSSARY.md](../../../GLOSSARY.md), behavior → code
- **If this doc conflicts with code, code wins.**

Local issue tracking system backed by SQLite, designed for autonomous agents.

## Overview

The `issues` crate provides a lightweight issue tracking database that agents can use to:

- Create and manage tasks, bugs, and feature requests
- Claim issues with automatic lease expiration (15 minutes)
- Track issue status through open → in_progress → done
- Prioritize work with urgent/high/medium/low levels
- Block issues with reasons and unblock when ready
- Audit all changes via the issue_events table

## Quick Start

```rust
use issues::{init_db, issue, Priority, IssueType};
use std::path::Path;

// Initialize database (creates file if needed, runs migrations)
let conn = init_db(Path::new("autopilot.db"))?;

// Create an issue
let issue = issue::create_issue(
    &conn,
    "Fix login timeout",
    Some("Users report session expires too quickly"),
    Priority::High,
    IssueType::Bug,
)?;

println!("Created issue #{}: {}", issue.number, issue.title);

// Get next ready issue (respects priority and blocking)
if let Some(ready) = issue::get_next_ready_issue(&conn)? {
    // Claim it for this run
    issue::claim_issue(&conn, &ready.id, "run-abc123")?;

    // ... do work ...

    // Mark complete
    issue::complete_issue(&conn, &ready.id)?;
}
```

## Documentation

| Document | Description |
|----------|-------------|
| [Schema](./schema.md) | Database tables, columns, and indexes |
| [API](./api.md) | All public functions with examples |
| [Patterns](./patterns.md) | Common usage patterns for agents |

## Design Principles

1. **Simple over complex** - Three statuses, four priorities, minimal fields
2. **Agent-friendly** - Lease-based claiming prevents conflicts between parallel agents
3. **Auditable** - All changes logged to issue_events
4. **Portable** - Single SQLite file, no external dependencies

## Module Structure

```
issues/
├── db.rs      # Database initialization and migrations
├── issue.rs   # Issue struct and all CRUD operations
└── lib.rs     # Public exports
```
