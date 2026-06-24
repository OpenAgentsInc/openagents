# OpenAgents Gym Issue Run Audit (#6163-#6167)

Updated: 2026-06-24

This audit tracks the sequential issue run for the OpenAgents Gym Phase 0 work
and the focused GPT-OSS owner/internal latency playground. The Phase 0 run is
intentionally scoped to no-spend fixture behavior. The GPT-OSS surface is a
separate owner-gated operational playground for the hourly Hydralisk L4 lane, not
a public paid-lane benchmark route.

## Current State

| Issue | Status | Main commit(s) | Notes |
| --- | --- | --- | --- |
| #6163 | Closed | `a6e95a944b`, `e74a63a0c8`, `48848bd8c6`, this closeout docs commit | Phase 0 epic acceptance satisfied by #6164, #6165, and #6166: public `/gym`, typed fixture config, fixture-only run path, deterministic scene, illustrative report viewer, and no real-spend path. |
| #6164 | Landed | `a6e95a944b`, `bd2c853502`, `ac211893a0` | Added the fixture-only `GymExperiment` schema/compiler/run path, widened the desktop Chrome CDP startup wait used by the deploy smoke gate, and created this issue-run audit. |
| #6165 | Landed | `e74a63a0c8` | Added the public `/gym` route, logged-out Foldkit page, typed fixture controls, locked economics panel, and no-spend fixture result trigger. |
| #6166 | Landed | `48848bd8c6` | Added the deterministic Three.js fixture run scene, public-safe report viewer metrics, illustrative banner, skipped-lane rendering, and null cost-per-accepted-outcome finding. |
| #6167 | Closed | `bcc9ff0015`, this closeout docs commit | Added the owner-gated `/gym/oss` GPT-OSS latency playground: prompt presets/custom prompt, sample count, concurrency/ramp, streaming execution, telemetry reconciliation, live scene, aggregate cards/table/chart, and hard in-flight cap. |

## #6163 Phase 0 Acceptance

| Acceptance item | Closeout evidence |
| --- | --- |
| `GET /gym` renders the explainer + knobs/dials surface. | `GymRoute()` is registered for `/gym`; the logged-out view renders `apps/openagents.com/apps/web/src/page/loggedOut/page/gym.ts` with typed controls and the no-spend banner. |
| Knobs bind to typed `GymExperiment` config. | Backend `GymExperiment` lives in `workers/api/src/inference/gym/experiment.ts`; public web fixture controls use Effect Schema-backed `PublicGymExperiment` in `apps/web/src/page/loggedOut/gym/flow.ts`. |
| "Run" compiles and executes through the fixture seam only. | `compileGymExperiment` rejects real seam without owner arming; the public web route materializes a fixture result only, with `budget.seam: "fixture"`. |
| A `three-effect` run scene animates the fixture run. | `apps/web/src/scene/gymFixtureRunSceneElement.ts` renders the deterministic fan-out/verdict/cost scene and is covered by scene tests. |
| Report panel shows percentiles, cost-per-accepted-outcome, verification rate, cache-hit rate, `illustrativeNotice`, and `decisionGrade: false`. | Public report viewer fields are rendered from `PublicGymReportViewer`; null cost-per-accepted-outcome is an explicit finding, not a fake number. |
| No code path can spend from `/gym` in Phase 0. | The logged-out Gym model only carries fixture budget data, and the backend compiler/run tests cover fixture-only behavior and real-seam refusal. |

## #6167 GPT-OSS Playground Acceptance

| Acceptance item | Closeout evidence |
| --- | --- |
| Owner/internal route for GPT-OSS only. | `/gym/oss` parses to `GymOssRoute()` and startup routing redirects logged-out and non-admin sessions away while allowing admin sessions. The target model is the neutral `openagents/khala-oss-20b` alias. |
| Prompt presets/custom prompt, sample count, concurrency dial, ramp mode, and Run. | `apps/web/src/page/loggedIn/gymOss/controller.ts` owns the form state and render surface; runner tests cover the normalized plan shape. |
| Streaming timing, server telemetry read, and honest reconciliation. | `apps/web/src/page/loggedIn/gymOss/stream.ts` parses OpenAI-compatible SSE frames; `runner.ts` reads `openagents.telemetry`, prefers measured server values, falls back to client timing, and keeps `not_measured` distinct from `0`. |
| Concurrency runner and 1->2->4->8 ramp with in-flight cap. | `MAX_IN_FLIGHT` is `8`; runner tests cover bounded concurrency and ramp sweep shape. |
| Live `three-effect` scene, aggregate cards/table/chart. | `gymOssSceneElement.ts` renders in-flight bars and aggregate throughput; controller/page tests cover the route shell and rendered result surface. |
| Offline deterministic test coverage. | Gym OSS route, startup, runner, stream, controller, aggregation, ramp, and scene tests are included in `check:deploy`. The live L4 click-through remains an owner smoke because CI must not load-test the GPU lane. |

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

## #6167 Verification

- `bun run --cwd apps/openagents.com/apps/web test -- src/route.test.ts src/routing/startup.test.ts src/product-policy.test.ts src/page/loggedIn/view.scene.test.ts src/scene/gymOssSceneElement.test.ts src/page/loggedIn/gymOss/gymOss.test.ts src/page/loggedIn/gymOss/stream.test.ts`
- `bun run --cwd apps/openagents.com/apps/web typecheck`
- `bun run check:deploy`

## Closeout Verification

- `bun run --cwd apps/openagents.com/apps/web test -- src/route.test.ts src/routing/startup.test.ts src/product-policy.test.ts src/page/loggedOut/update.test.ts src/page/loggedOut/page/gym.test.ts src/scene/gymFixtureRunSceneElement.test.ts src/page/loggedIn/view.scene.test.ts src/scene/gymOssSceneElement.test.ts src/page/loggedIn/gymOss/gymOss.test.ts src/page/loggedIn/gymOss/stream.test.ts`
- `bun run check:deploy`

## Notes For Continuation

- Keep Phase 0 fixture-only. `budget.seam: "real"` must remain rejected until a
  future issue explicitly adds real spend, wallet, and quota controls.
- The public `/gym` viewer consumes a public-safe report projection. The
  backend `buildBenchmarkReport` remains the canonical decision-grade report
  path for future real-seam work.
- The `/gym` UI should make skipped/unavailable lanes visible instead of
  implying that all coordinator/provider combinations are live.
- Keep `/gym/oss` owner/internal. It is intentionally allowed to hit the hourly
  GPT-OSS lane without a per-call balance gate, but it must not become a public
  unauthenticated load generator.
