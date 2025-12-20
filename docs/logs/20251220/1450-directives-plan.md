# Directives System Plan

## Overview

Add a "directives" system - a higher-level concept above issues that sets project direction. Directives are epics like "Implement 100% of Nostr protocol" or "Add comprehensive test coverage". The autopilot will periodically decompose active directives into concrete issues.

## Design Decisions

1. **Storage**: Markdown files in `.openagents/directives/` folder
2. **Format**: YAML frontmatter for metadata + markdown body for description
3. **Lifecycle**: `active`, `paused`, `completed` status
4. **Integration**: Autopilot periodically reviews directives and auto-generates sub-issues
5. **Code location**:
   - Core directive module in `crates/issues/` (parsing, loading, progress calculation)
   - CLI commands and loop integration in `crates/autopilot/`
6. **Issue linking**: Add `directive_id` column to issues table for progress tracking

## Directive File Format

```markdown
---
id: "d-001"
title: "Implement 100% of Nostr Protocol"
status: active  # active | paused | completed
priority: high  # urgent | high | medium | low
created: 2025-12-20
updated: 2025-12-20
---

## Goal

Fully implement the Nostr protocol in Rust for both client and relay functionality.

## Success Criteria

- [ ] All NIPs implemented in crates/nostr/core
- [ ] Relay passes all protocol tests
- [ ] Client can connect to public relays

## Notes

Additional context, links to specs, etc.
```

## Implementation Steps

### Step 1: Add Directive Module to Issues Crate

Create `crates/issues/src/directive.rs`:

**Directive struct:**
```rust
pub struct Directive {
    pub id: String,
    pub title: String,
    pub status: DirectiveStatus,
    pub priority: Priority,
    pub created: NaiveDate,
    pub updated: NaiveDate,
    pub body: String,          // Full markdown body
    pub file_path: PathBuf,    // Source file
}

pub enum DirectiveStatus {
    Active,
    Paused,
    Completed,
}
```

**Key functions:**
- `load_directives(dir: &Path) -> Result<Vec<Directive>>` - Load all from `.openagents/directives/`
- `load_directive(path: &Path) -> Result<Directive>` - Load single file
- `save_directive(directive: &Directive) -> Result<()>` - Write back to file
- `get_active_directives(dir: &Path) -> Result<Vec<Directive>>` - Filter active only
- `calculate_progress(conn: &Connection, directive_id: &str) -> u8` - Compute from linked issues

### Step 2: Add Database Migration

Add to `crates/issues/src/db.rs`:

**Migration v6:**
```rust
fn migrate_v6(conn: &Connection) -> Result<()> {
    conn.execute_batch(r#"
        ALTER TABLE issues ADD COLUMN directive_id TEXT;
        CREATE INDEX idx_issues_directive ON issues(directive_id);
    "#)?;
    set_schema_version(conn, 6)?;
    Ok(())
}
```

Update `crates/issues/src/issue.rs`:
- Add `directive_id: Option<String>` to `Issue` struct
- Update `create_issue()` to accept optional `directive_id`
- Add `list_issues_by_directive(directive_id)` function

### Step 3: Add MCP Tools

Add to `crates/issues-mcp/src/main.rs`:

**New tools:**
- `directive_list` - List all directives (optionally by status)
- `directive_get` - Get directive by ID with progress
- `directive_create` - Create new directive (writes markdown file)
- `directive_update` - Update status
- `directive_decompose` - Prompt to review and create issues

Update `issue_create` tool:
- Add optional `directive_id` parameter

### Step 4: Integrate with Autopilot Loop

Modify `crates/autopilot/src/main.rs`:

**Changes:**
1. On startup, load active directives from `.openagents/directives/`
2. When no ready issues, include directive context in prompt:
```
No ready issues. Review active directives and create concrete issues:

[Directive: Implement Nostr Protocol (d-001)]
Status: active, Priority: high, Progress: 2/10 issues (20%)
Goal: Fully implement the Nostr protocol...

Create 1-3 specific, actionable issues to advance this directive.
Use issue_create with directive_id="d-001" to link them.
```
3. Track directive progress when issues are completed

### Step 5: Create Directory Structure

Create `.openagents/directives/` folder.

**Files to create:**
- `.openagents/directives/README.md` - Format documentation

### Step 6: CLI Commands

Add to autopilot CLI (subcommand under `cargo autopilot`):

```bash
cargo autopilot directive list              # List all directives with progress
cargo autopilot directive list --active     # Active only
cargo autopilot directive show d-001        # Show directive with linked issues
cargo autopilot directive create "Title"    # Create new directive file
cargo autopilot directive pause d-001       # Update status to paused
cargo autopilot directive complete d-001    # Update status to completed
```

## Files to Modify

| File | Changes |
|------|---------|
| `crates/issues/src/directive.rs` | NEW - Directive struct, parsing, loading/saving |
| `crates/issues/src/lib.rs` | Add `pub mod directive;` and re-exports |
| `crates/issues/src/db.rs` | Add migration v6 for directive_id |
| `crates/issues/src/issue.rs` | Add directive_id field and functions |
| `crates/issues/Cargo.toml` | Add serde_yaml, glob dependencies |
| `crates/issues-mcp/src/main.rs` | Add directive tools, update issue_create |
| `crates/autopilot/src/main.rs` | Add CLI commands, integrate with loop |
| `.openagents/directives/README.md` | NEW - Format docs |

## Testing

- Unit tests in `crates/issues/src/directive.rs` for parsing
- Test loading from multiple files
- Test status transitions and file updates
- Test progress calculation from linked issues
- Integration test: directive -> issue creation flow
