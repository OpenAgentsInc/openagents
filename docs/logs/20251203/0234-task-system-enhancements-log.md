# 0234 Work Log - Task System Enhancement Epic

## Task
oa-4e8aa5: Task System Enhancement Epic (from beads)

## Intent
Implement Tier 1 CLI commands for the task system:
- tasks:reopen (oa-d51a28)
- tasks:stats (oa-cfba10)
- tasks:stale (oa-54c465)
- tasks:show with dependency tree (oa-b8bc22)

## Completed

### 1. tasks:reopen command (oa-d51a28)
- Added `reopenTask` function to service.ts
- Simple status transition: closed → open
- Clears closedAt and closeReason fields
- Added `cmdReopen` to cli.ts with `--id` option
- Added tests for happy path and error cases

### 2. tasks:stats command (oa-cfba10)
- Added `getTaskStats` function to service.ts
- Returns counts grouped by status, type, and priority
- Human-readable output with labeled priorities
- JSON output for machine consumption

### 3. tasks:stale command (oa-54c465)
- Added `getStaleTasks` function to service.ts
- Finds tasks not updated in N days (default 30)
- Optional --status filter
- Shows days since last update in human-readable output

### 4. tasks:show command (oa-b8bc22)
- Added `getTaskWithDeps` function to service.ts
- Shows full task details including description
- Displays dependency tree: blockedBy and blocking tasks
- Uses checkmarks to show completion status of deps

## Validation
- All tests pass: `bun test src/tasks/` (79 tests)
- Typecheck passes: `bun run typecheck`
- Manual testing of all 4 new commands

## Files Modified
- src/tasks/service.ts - Added reopenTask, getTaskStats, getStaleTasks, getTaskWithDeps
- src/tasks/index.ts - Exported new functions and types
- src/tasks/cli.ts - Added cmdReopen, cmdStats, cmdStale, cmdShow and help text
- src/tasks/service.test.ts - Added tests for reopenTask
