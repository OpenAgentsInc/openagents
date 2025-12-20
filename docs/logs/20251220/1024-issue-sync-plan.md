# Issue Sync Between Computers - Plan

## Problem

The autopilot issue system uses a local SQLite database (`autopilot.db`) that doesn't sync between machines. The database is correctly gitignored (no sensitive data in git), but this means issues created on one machine don't appear on another.

## Current State

- **Database**: `autopilot.db` in project root (SQLite)
- **Config**: `.mcp.json` points to hardcoded path
- **Gitignore**: `*.db` already excluded (correct)
- **Tables**: issues, issue_events, issue_counter, projects, sessions
- **Sensitive data concerns**: prompts in sessions table, trajectory paths, blocked reasons may contain sensitive info

## Options

### Option 1: JSON Export/Import (Simplest)

Add CLI commands to export/import issues as JSON files that can be committed.

**Pros:**
- Simple to implement
- Selective sync (choose what to commit)
- Human-readable diffs
- Can sanitize sensitive fields on export

**Cons:**
- Manual process
- Merge conflicts possible
- Not real-time

**Implementation:**
1. Add `cargo autopilot export --output issues.json` command
2. Add `cargo autopilot import --input issues.json` command
3. Export only issues table (not sessions/events which are machine-specific)
4. Store in `issues.json` or `.openagents/issues.json`
5. Add merge logic for conflicting issue numbers

### Option 2: Nostr-Based Sync (Native to Project)

Use Nostr events to sync issues across machines. Fits the OpenAgents ethos.

**Pros:**
- Decentralized, no server needed
- Real-time sync possible
- Aligns with project's Nostr focus
- Works across any number of machines
- Built-in signing/verification

**Cons:**
- More complex implementation
- Need to define custom NIP or use existing structure
- Relay dependency
- Need to handle conflicts

**Implementation:**
1. Define issue event kind (replaceable parameterized event)
2. Sign issues with nsec (stored locally)
3. Publish to configured relays on create/update
4. Subscribe to own pubkey's issue events on startup
5. Merge remote issues into local DB
6. Use issue UUID as `d` tag for replaceable events

**Event Structure:**
```json
{
  "kind": 30090,  // Replaceable parameterized
  "tags": [
    ["d", "<issue-uuid>"],
    ["title", "Issue title"],
    ["status", "open"],
    ["priority", "high"],
    ["type", "task"]
  ],
  "content": "Issue description",
  "created_at": 1734567890
}
```

### Option 3: Git-Tracked YAML/TOML Files

Store issues as individual files instead of database.

**Pros:**
- Full git history
- Easy to edit manually
- Standard merge tools work

**Cons:**
- Performance at scale
- Lose SQLite query benefits
- Major refactor

**Implementation:**
1. Create `.openagents/issues/` directory
2. Each issue as `<number>.yaml`
3. Rewrite issues crate to use file-based storage
4. Add gitignore for sensitive fields file

### Option 4: Syncthing/External Sync

Use Syncthing or similar to sync the database file.

**Pros:**
- Zero code changes
- Works now

**Cons:**
- SQLite concurrent write issues
- Need Syncthing on all machines
- Can corrupt database if both machines write

**Not Recommended** due to SQLite write safety.

## Recommendation

**Phase 1: JSON Export/Import** (do first)
- Quick to implement
- Solves immediate problem
- No dependencies

**Phase 2: Nostr Sync** (future)
- Fits project direction
- Real-time sync
- Build on existing Nostr infrastructure

## Implementation Plan for Phase 1

1. Add `export` subcommand to autopilot CLI
   - Export issues table to JSON
   - Option to include/exclude completed issues
   - Sanitize sessions table (exclude prompts)

2. Add `import` subcommand
   - Read JSON file
   - Merge strategy: skip if same UUID exists, or force update
   - Preserve local issue_counter (renumber if conflicts)

3. Add convenience commands
   - `cargo autopilot sync-export` - export to `.openagents/issues.json`
   - `cargo autopilot sync-import` - import from same file

4. Update `.gitignore` to NOT ignore `.openagents/issues.json`

5. Document workflow in README

## File Changes Required

```
crates/autopilot/src/main.rs     # Add export/import commands
crates/issues/src/lib.rs         # Add serialize_all/deserialize_all
.gitignore                       # Whitelist issues.json
```

## Next Steps

1. Implement Option 1 (JSON export/import)
2. Test roundtrip: export on machine A, import on machine B
3. Later: Add Nostr sync for real-time capability
