# OpenAgents Gym Issue Run Audit (#6164-#6166)

Updated: 2026-06-24

This audit tracks the sequential issue run for the OpenAgents Gym Phase 0 work.
The run is intentionally scoped to no-spend fixture behavior until the public UI,
report viewer, and safety boundaries are all in place.

## Current State

| Issue | Status | Main commit(s) | Notes |
| --- | --- | --- | --- |
| #6164 | Landed | `a6e95a944b`, `bd2c853502`, `ac211893a0` | Added the fixture-only `GymExperiment` schema/compiler/run path, widened the desktop Chrome CDP startup wait used by the deploy smoke gate, and created this issue-run audit. |
| #6165 | Landed | `e74a63a0c8` | Added the public `/gym` route, logged-out Foldkit page, typed fixture controls, locked economics panel, and no-spend fixture result trigger. |
| #6166 | Landed | `48848bd8c6` | Added the deterministic Three.js fixture run scene, public-safe report viewer metrics, illustrative banner, skipped-lane rendering, and null cost-per-accepted-outcome finding. |

## #6164 Verification

- `bun run --cwd apps/openagents.com/workers/api test -- src/inference/gym/experiment.test.ts src/inference/benchmark/matrix.test.ts src/inference/benchmark/runner.test.ts src/inference/benchmark/report.test.ts src/inference/benchmark/lane-seam.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`
- `bun run --cwd apps/autopilot-desktop smoke:training-scene`
- `bun run --cwd apps/autopilot-desktop smoke:verse-launch`
- Full pre-push `bun run check:deploy` passed before pushing `bd2c853502` to `main`.

## #6165 Verification

- `bun run --cwd apps/openagents.com/apps/web test -- src/route.test.ts src/routing/startup.test.ts src/product-policy.test.ts src/page/loggedOut/update.test.ts src/page/loggedOut/page/gym.test.ts`
- `bun run --cwd apps/openagents.com/apps/web typecheck`
- `bun run --cwd apps/openagents.com check:architecture`
- `bun run --cwd apps/openagents.com check:effect-topology`
- Full pre-push `bun run check:deploy` passed before the #6165 landing commit.

## #6166 Verification

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedOut/page/gym.test.ts src/page/loggedOut/update.test.ts src/scene/gymFixtureRunSceneElement.test.ts`
- `bun run --cwd apps/openagents.com/apps/web typecheck`
- `bun run check:deploy`

## Notes For Continuation

- Keep Phase 0 fixture-only. `budget.seam: "real"` must remain rejected until a
  future issue explicitly adds real spend, wallet, and quota controls.
- The public `/gym` viewer consumes a public-safe report projection. The
  backend `buildBenchmarkReport` remains the canonical decision-grade report
  path for future real-seam work.
- The `/gym` UI should make skipped/unavailable lanes visible instead of
  implying that all coordinator/provider combinations are live.
