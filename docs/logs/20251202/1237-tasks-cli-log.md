# 1237 Tasks CLI Implementation Log

## Tasks Completed

| ID | Title | Status |
|---|---|---|
| oa-281868 | Implement tasks CLI (tasks:init, tasks:list, tasks:ready) | closed |
| oa-ab3cfc | Implement tasks CLI (tasks:next, tasks:create, tasks:update) | closed |
| oa-a2d2a1 | Add package.json scripts for tasks CLI | closed |
| oa-ba2bbc | Document task CLI usage in AGENTS.md | closed |

## Work Done

### 1. Created `src/tasks/cli.ts`
Full CLI implementation with commands:
- `init` - Initialize .openagents for a repo
- `list` - List tasks with filters (--status, --priority, --type, --labels, --assignee, --limit)
- `ready` - List ready tasks (no open blockers)
- `next` - Atomically pick next ready task, mark it in_progress
- `create` - Create new task from flags or JSON stdin
- `update` - Update/close task from JSON stdin

### 2. Added package.json scripts
```json
"tasks:init": "bun src/tasks/cli.ts init",
"tasks:list": "bun src/tasks/cli.ts list",
"tasks:ready": "bun src/tasks/cli.ts ready",
"tasks:next": "bun src/tasks/cli.ts next",
"tasks:create": "bun src/tasks/cli.ts create",
"tasks:update": "bun src/tasks/cli.ts update"
```

### 3. Updated AGENTS.md
Added "Using the OpenAgents Task CLI" section with:
- Example commands for external agents
- Link to full CLI help and TASK-SPEC.md

### 4. Fixed bootstrap-tasks.ts
Added missing `deps: []` field to TaskCreate objects.

## Validation
- `bun test` - 96 tests pass
- `bun run typecheck` - No errors
- Manual CLI testing:
  - `bun run tasks:list --json` - Works
  - `bun run tasks:ready --json` - Works  
  - `bun run tasks:next --json` - Works
  - `bun run tasks:update --json-input --json` - Works

## Files Changed
- `src/tasks/cli.ts` (new) - CLI implementation
- `package.json` - Added tasks:* scripts
- `AGENTS.md` - Documented CLI usage
- `scripts/bootstrap-tasks.ts` - Fixed TypeScript errors
- `.openagents/tasks.jsonl` - Tasks updated to closed

## Summary
The OpenAgents task CLI is now fully implemented. External agents (Claude Code, Codex, shell scripts) can interact with the task system using `bun run tasks:*` commands with JSON output.
