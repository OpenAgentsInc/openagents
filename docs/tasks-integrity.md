# Task File Integrity Check

Use `bun run tasks:integrity` to catch problems with `.openagents/tasks.jsonl` before running agents or CI. The check verifies:

- The file exists and is tracked by git (not skip-worktree/assume-unchanged).
- The JSONL content passes the Task schema.

## Commands

- `bun run tasks:integrity` – fail fast on issues (non-zero exit).
- `bun run tasks:integrity --json` – machine-readable output for CI.
- `bun run tasks:integrity --fix` – clears the skip-worktree bit if set (leaves schema errors intact).

Run this in preflight hooks or CI to avoid agents silently missing task updates.
