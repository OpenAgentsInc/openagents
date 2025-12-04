# Stuck/in-progress task detection

- Run `bun run tasks:stale --days 7 --status in_progress` before long loops to catch abandoned tasks. Adjust `--days` for your project cadence.
- If stale tasks are found:
  - Inspect the task/logs to see if work is recoverable.
  - Either `bun run tasks:reopen --id <task>` to resume or `bun run tasks:update --id <task> --status blocked --reason "<why>"`.
  - Clean up any stale subtasks or worktrees linked to the task before resuming.
- Agents should surface stale in-progress tasks in orientation; human operators can run `tasks:stale` manually or on a schedule.
- `tasks:validate`/`tasks:doctor` (future) should include stale detection; until then, `tasks:stale` is the supported path.
