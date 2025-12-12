# 1005 - oa-1c366f: Deletion tombstones tracking

## Summary
Implemented deletion tombstones tracking system for task management. Added schema and service functions to record task deletions in deletions.jsonl for potential restore functionality.

## Changes
- Added DeletionEntry schema to src/tasks/schema.ts
- Added decodeDeletionEntry decoder
- Implemented deletion tracking functions in src/tasks/service.ts:
  - readDeletions() - Read deletions from deletions.jsonl
  - writeDeletions() - Write deletions to deletions.jsonl  
  - recordDeletion() - Record a task deletion with timestamp, actor, and reason
- Updated exports in src/tasks/index.ts

## Validation
- All task service tests pass (18/18)
- Typecheck passes  
- CLI works correctly (bun run tasks:ready)

## Task Status
- Task oa-1c366f marked as ready to close

