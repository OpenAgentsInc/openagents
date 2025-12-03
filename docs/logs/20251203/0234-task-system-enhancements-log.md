# 0234 Work Log - Task System Enhancement Epic

## Task
oa-4e8aa5: Task System Enhancement Epic (from beads)

## Intent
Implement Tier 1 CLI commands for the task system:
- tasks:reopen (oa-d51a28)
- tasks:stats (oa-cfba10)
- tasks:stale (oa-54c465)
- tasks:show with dependency tree (oa-b8bc22)

## Progress

### 1. tasks:reopen command
- Simple status transition: closed → open
- Clears closedAt and closeReason fields
- Added `reopenTask` function to service.ts
- Added `cmdReopen` to cli.ts with `--id` option
