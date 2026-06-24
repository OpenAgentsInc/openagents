# OpenAgents Gym Issue Run Audit (#6164-#6166)

Updated: 2026-06-24

This audit tracks the sequential issue run for the OpenAgents Gym Phase 0 work.
The run is intentionally scoped to no-spend fixture behavior until the public UI,
report viewer, and safety boundaries are all in place.

## Current State

| Issue | Status | Main commit(s) | Notes |
| --- | --- | --- | --- |
| #6164 | Landed | `a6e95a944b`, `bd2c853502` | Added the fixture-only `GymExperiment` schema/compiler/run path and widened the desktop Chrome CDP startup wait used by the deploy smoke gate. |
| #6165 | Pending | n/a | Next: add the public `/gym` route with typed knobs/dials and a no-spend fixture run trigger. |
| #6166 | Pending | n/a | Next after #6165: add the public-safe report viewer and visual result summaries. |

## #6164 Verification

- `bun run --cwd apps/openagents.com/workers/api test -- src/inference/gym/experiment.test.ts src/inference/benchmark/matrix.test.ts src/inference/benchmark/runner.test.ts src/inference/benchmark/report.test.ts src/inference/benchmark/lane-seam.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`
- `bun run --cwd apps/autopilot-desktop smoke:training-scene`
- `bun run --cwd apps/autopilot-desktop smoke:verse-launch`
- Full pre-push `bun run check:deploy` passed before pushing `bd2c853502` to `main`.

## Notes For Continuation

- Keep Phase 0 fixture-only. `budget.seam: "real"` must remain rejected until a
  future issue explicitly adds real spend, wallet, and quota controls.
- Use the existing benchmark matrix, runner, report, and public-safety checker
  rather than creating a parallel Gym report format.
- The `/gym` UI should make skipped/unavailable lanes visible instead of
  implying that all coordinator/provider combinations are live.
