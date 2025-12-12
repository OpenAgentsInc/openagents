# $TS Type Fixes Progress

- Reworked `src/tasks/cli.ts` commands (`merge`, `rename`, `show`, `comment add`, `archive`, `compact`, `search`) to match the SQLite-backed TaskService signatures (no more `blockedBy`, `dryRun` payloads, or `tasks.jsonl` conventions).
- Added `applyTaskUpdate` helper and `satisfies Task` usage in `src/tasks/service.ts` so updates no longer inject `undefined` into the readonly schema fields, and updated `src/tasks/integrity.ts` to read from the SQLite DB with a proper layer.
- Reran `bun run typecheck` (still failing across archivist/bench/trainer/tooling piles but CLI/service regressions are resolved for now).
