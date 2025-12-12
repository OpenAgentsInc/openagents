# $TS Typefixes Progress

- Status: Updated `bench/model-adapter.ts` and `tbench` CLI to satisfy `exactOptionalPropertyTypes` (TaskRunResult, RunState, learning metrics, and runSingleTask args now allow `undefined` where needed).
- Tests: `bun run typecheck` (still failing about remaining sandbox, desktop, effuse, skills, storage, and tasks/service errors).
- Next: Continue fixing the remaining type errors reported by the latest typecheck run, starting with the most critical CLI/desktop/db mismatches.
