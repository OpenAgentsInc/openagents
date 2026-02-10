# Overnight Self Improvement Plan (Agent-Run, Convex-First, Prod-Verified)

- **Status:** Proposed (implementation + nightly ops plan)
- **Last updated:** 2026-02-10
- **Priority:** programmatic loop first; UI is read-only later
- **Primary refs (do not duplicate):**
  - Operator workflow + current endpoints: `docs/autopilot/DSE_PLAYBOOK.md`
  - System plan + Convex-first constraints: `docs/autopilot/SELF_IMPROVE_PLAN.md`
  - RLM program + phases: `docs/autopilot/RLM_UNIFIED_ROADMAP.md`
  - Prod E2E harness: `docs/autopilot/PROD_E2E_TESTING.md`
  - UI determinism posture: `docs/autopilot/STREAM_TESTING.md`
  - Deep spec/intent: `docs/autopilot/dse.md`
- **If this doc conflicts with code behavior:** code wins

This is the single overnight plan to make Autopilot *actually* self-improve in a loop in production:

- **Agents (Codex and others) can run it programmatically** (no clicking required).
- **All evidence is stored in Convex** for later visualization (read-only pages).
- **Prod is verified** (E2E + request-id correlation), and changes ship behind canary/promotion gates.

## Non-Goals (Overnight)

- Building any “control UI” for compile/promote/canary. (We’ll add read-only visualization later.)
- Solving subjective tasks (recap quality) via fully automated judging. (We start with discrete, labelable signatures.)

## Definition Of Done (By Morning)

We consider “overnight self-improve” successful when, for at least one target signature:

- We have a real dataset in Convex (`dseExamples`) with a holdout split.
- We ran compile in prod and stored:
  - `dseCompileReports` (jobHash + datasetHash)
  - `dseArtifacts` (compiled artifact)
- We ran either:
  - a canary rollout (stored in `dseCanaries` + `dseCanaryHistory`), or
  - a promotion (stored in `dseActiveArtifacts` + `dseActiveArtifactHistory`).
- We have an **ops run record** in Convex (new) that links everything (commit sha, signature ids, compile report ids, promoted ids, E2E summary).
- Prod E2E passes Autopilot + DSE observability:
  - `apps-web.prod.autopilot.dse-canary-recap-shows-debug-card-and-trace` in `packages/effuse-test/src/suites/apps-web.ts`.

## Scope: What We Improve Overnight

We run two tracks, but only one needs to “ship” overnight.

### Track 1 (Ship): Discrete decision surface

- Target signature: `@openagents/autopilot/blueprint/SelectTool.v1`
  - Source: `apps/autopilot-worker/src/dseCatalog.ts`
  - Why: discrete output, cheap labels, deterministic reward, direct UX impact.

### Track 2 (Verify): RLM-lite observability remains intact

- Canary signature: `@openagents/autopilot/canary/RecapThread.v1`
  - Source: `apps/autopilot-worker/src/dseCatalog.ts`
  - Why: ensures `rlm_lite.v1` remains visible + debuggable in prod (receipts + trace blobs).

## Convex: What Must Be Stored (Source Of Truth)

Already implemented tables (see `apps/web/convex/schema.ts`):

- Datasets: `dseExamples`
- Compile outputs: `dseArtifacts`, `dseCompileReports`
- Runtime pointers + history: `dseActiveArtifacts`, `dseActiveArtifactHistory`
- Canary config + history: `dseCanaries`, `dseCanaryHistory`
- RLM persistence: `dseBlobs`, `dseVarSpace`
- Predict receipts: `receipts` with `kind="dse.predict"` (+ `receiptId`, `signatureId`, `compiled_id`)

Missing (must add for agent-run overnight ops):

- `dseOpsRuns` (one row per overnight run)
- `dseOpsRunEvents` (append-only, bounded event stream per ops run)

Why: later visualization should not require scraping local logs or `output/effuse-test/*`.

## Phase Plan (Programmatic Loop First)

This plan is **6 phases**. Phases 1–5 are required to make the loop agent-runnable and Convex-backed.
Phase 6 is read-only visualization later.

### Phase 1: Programmatic Ops Auth + Run Recording (Convex)

Objective: agents can operate `/api/dse/*` without a browser session, and every overnight run is recorded in Convex.

Deliverables:

- Worker secret: `OA_DSE_ADMIN_SECRET`
- Admin auth mode for DSE endpoints:
  - accept `Authorization: Bearer <OA_DSE_ADMIN_SECRET>` as an alternative to WorkOS session cookies
  - applies to: compile/promote/canary/trace-export + new ops endpoints
- Convex tables:
  - `dseOpsRuns`: `{ runId, startedAtMs, endedAtMs?, commitSha, baseUrl, actor, status, signatureIds, notes?, links... }`
  - `dseOpsRunEvents`: `{ runId, tsMs, phase, level, message, json? }`
- Worker endpoints (admin-secret gated):
  - `POST /api/dse/ops/run/start`
  - `POST /api/dse/ops/run/event`
  - `POST /api/dse/ops/run/finish`

Exit criteria:

- A headless script can start a run, append events, and finish a run, and the data is visible in Convex.

### Phase 2: Programmatic Runner (One Command) + Baseline Checks

Objective: one command runs the overnight loop steps deterministically and logs everything to Convex.

Deliverables:

- Add a Bun/Node script (repo-local) that becomes the canonical agent entrypoint:
  - Suggested path: `apps/web/scripts/dse-overnight.ts` (or `tools/dse-overnight.ts`)
- Script responsibilities:
  - preflight checks (env vars, baseUrl reachability, secrets)
  - run local verification when running locally (optional but default-on):
    - `cd packages/dse && bun test && bun run typecheck`
    - `cd apps/web && npm run lint && npm test`
  - run prod E2E smoke (see Phase 5) OR at minimum the DSE recap visibility test
  - write progress into `dseOpsRunEvents`

Inputs:

- `BASE_URL` (e.g. `https://openagents.com`)
- `OA_DSE_ADMIN_SECRET`
- `EFFUSE_TEST_E2E_BYPASS_SECRET` (for prod E2E)

Exit criteria:

- `bun run apps/web/scripts/dse-overnight.ts --base-url ...` creates a `dseOpsRuns` record and emits events.

### Phase 3: Dataset Ingestion (Convex) For SelectTool

Objective: agents can seed/update a real dataset in Convex without manual UI.

Deliverables:

- A canonical dataset file committed to the repo (stable ids):
  - Suggested: `docs/autopilot/fixtures/dse-selecttool.dataset.v1.jsonl`
  - Each row includes: `exampleId`, `inputJson`, `expectedJson`, `split`, `tags`
- A programmatic import path:
  - Option A (preferred): a Worker endpoint `POST /api/dse/examples/import` (admin-secret gated) that upserts into `dseExamples`.
  - Option B: the overnight script calls Convex directly with a privileged token (harder in prod).
- Dataset rules:
  - at least ~30 examples
  - at least 10 holdout examples (`split="holdout"`)
  - tags include `overnight`, `selecttool`, dataset version

Exit criteria:

- Re-running the import is idempotent (upsert by `signatureId+exampleId`).
- `POST /api/dse/compile` against prod uses the Convex dataset.

### Phase 4: Make Compile Non-Trivial + Single Source Of Truth Job Spec

Objective: compile actually changes something (not a single-candidate no-op) and promotion gates are aligned.

Deliverables:

- Centralize compile job spec definition so compile and promote/canary gates cannot diverge:
  - Move shared logic into one module (e.g. `apps/web/src/effuse-host/dseJobs.ts`).
  - Both `apps/web/src/effuse-host/dseCompile.ts` and `apps/web/src/effuse-host/dseAdmin.ts` must use it.
- Add a real search space for `SelectTool.v1`:
  - instruction variants (2–6)
  - optionally few-shot selection (if/when we have enough few-shots)
  - optionally decode policy variants (only if needed)
- Keep reward deterministic overnight:
  - `exact_json_match` is OK for this discrete signature.

Exit criteria:

- Compile evaluates multiple candidates (and reports candidate count).
- Compile is idempotent by `(jobHash, datasetHash)`.
- Promote/canary gates use the exact same `(jobSpec, jobHash)` definition.

### Phase 5: Fully Automated Prod Run (Compile -> Canary -> Promote/Rollback) + Prod E2E

Objective: agents can run the full loop in prod, safely, with all outcomes recorded in Convex.

Deliverables:

- Overnight script performs:
  1. compile (prod): `POST /api/dse/compile`
  2. start canary (prod): `POST /api/dse/canary/start`
  3. generate traffic to reach `minSamples` quickly:
     - run `packages/effuse-test` prod tests that exercise `/autopilot` messages
     - optionally add a dedicated “signature exerciser” endpoint for SelectTool to generate samples deterministically
  4. monitor canary counters via Convex queries (`dseCanaries`) until:
     - minSamples reached and errorRate OK, OR
     - auto-stop triggers, OR
     - timeout
  5. if clean: promote via `POST /api/dse/promote`
     - else: stop canary or rollback pointers
- Prod E2E verification:

```bash
cd apps/web
EFFUSE_TEST_E2E_BYPASS_SECRET="..." \
  npm run test:e2e -- --base-url https://openagents.com --tag prod --grep "apps-web\\.prod\\.autopilot"
```

- Store E2E summary in Convex (`dseOpsRuns` + events):
  - pass/fail
  - failing test ids
  - request ids (`x-oa-request-id`) when available

Exit criteria:

- By morning, Convex contains:
  - dataset rows, compile report, artifact, canary history, and either promotion or rollback history
  - plus a single `dseOpsRuns` record linking them all.

### Phase 6 (Later): Read-Only Visualization

Objective: display the Convex-stored loop evidence on web pages, without control surfaces.

Deliverables:

- Read-only pages that show:
  - recent ops runs (`dseOpsRuns`)
  - per-signature history (active pointer changes, canary history, compile reports)
  - datasets + example diffs
  - links to receipts + traces

Exit criteria:

- A non-operator can answer “what improved last night?” by reading web pages backed by Convex.

## Notes / Current Code Surface

Current endpoints and storage:

- Compile: `POST /api/dse/compile` (`apps/web/src/effuse-host/dseCompile.ts`)
- Promote/canary/trace export/receipt+blob reads: `apps/web/src/effuse-host/dseAdmin.ts`
- Convex DSE tables/functions: `apps/web/convex/dse/*`
- Target signatures: `apps/autopilot-worker/src/dseCatalog.ts`
- Prod E2E suite: `packages/effuse-test/src/suites/apps-web.ts`

## Implementation Log

- 2026-02-10T08:19:24Z Phase 1: programmatic DSE ops auth + Convex ops run recording.
- Added Worker env typing + admin-secret auth helper:
  - `OA_DSE_ADMIN_SECRET` support (`Authorization: Bearer <OA_DSE_ADMIN_SECRET>`)
  - Worker-minted admin JWT (subject=`user_dse_admin`) via `OA_E2E_JWT_PRIVATE_JWK` for headless Convex access.
- Added Convex ops persistence:
  - tables: `dseOpsRuns`, `dseOpsRunEvents`
  - mutations: `startRun`, `appendEvent`, `finishRun` (admin-only).
- Added Worker ops endpoints (admin-secret gated):
  - `POST /api/dse/ops/run/start`
  - `POST /api/dse/ops/run/event`
  - `POST /api/dse/ops/run/finish`
- Enabled admin-secret mode for DSE endpoints (no cookies required):
  - `POST /api/dse/compile`
  - `POST /api/dse/promote`
  - `POST /api/dse/canary/start`, `POST /api/dse/canary/stop`
  - `POST /api/dse/trace/export` (includes admin-only Convex reads for receipt/blob).
- Tests / verification:
  - `cd apps/web && npx convex codegen` (ok)
  - `cd apps/web && npm run lint` (ok)
  - `cd apps/web && npm test` (ok)

- 2026-02-10T04:54:49Z Phase 7: judge-based rewards for recap/summarization + Convex-stored eval reports.
- Added judge signature:
  - `@openagents/autopilot/judge/ThreadSummaryQuality.v1` in `apps/autopilot-worker/src/dseCatalog.ts`
- Added pinned judge artifact (compiled + hashed):
  - `apps/web/src/effuse-host/dsePinnedArtifacts.ts` (`THREAD_SUMMARY_JUDGE_ARTIFACT_V1`)
  - test: `apps/web/tests/worker/dse-pinned-artifacts.test.ts` ensures hashes match the signature contract + params
- Added judge-based reward bundle and wired it for recap/summarization signatures:
  - `reward_thread_summary_judge.v1` in `apps/web/src/effuse-host/dseJobs.ts`
  - applied to:
    - `@openagents/autopilot/canary/RecapThread.v1`
    - `@openagents/autopilot/rlm/SummarizeThread.v1`
- Extended datasets to support BlobRef-backed examples safely:
  - added optional `meta` field to `dseExamples` (for blob texts to seed `BlobStore`)
  - helper: `apps/web/src/effuse-host/dseDatasetBlobs.ts`
  - compile + baseline eval paths now seed `BlobStore` from example metadata when present
- Added Convex eval reports:
  - table: `dseEvalReports` in `apps/web/convex/schema.ts`
  - queries/mutations: `apps/web/convex/dse/evalReports.ts`
  - tests: `apps/web/tests/convex/dse-eval-reports.test.ts`
- Added Worker endpoint:
  - `POST /api/dse/eval` (stores a stable `evalHash` + full JSON report into Convex)
  - test: `apps/web/tests/worker/dse-eval-endpoint.test.ts` asserts judge pin info is present in stored report JSON
- Added read-only visualization for eval reports:
  - signature page now lists eval reports
  - new `/dse/eval-report/:evalHash/:signatureId` detail page
  - test: `apps/web/tests/worker/routes.test.ts`
- Tests / verification:
  - `cd apps/web && npx convex codegen` (ok)
  - `cd apps/web && npm run lint` (ok)
  - `cd apps/web && npm test` (ok)

- 2026-02-10T08:28:52Z Phase 2: added canonical headless runner script + unit tests.
- Added runner:
  - `apps/web/scripts/dse-overnight.ts` (CLI)
  - `apps/web/scripts/dse-overnight-lib.ts` (pure logic, testable)
- Script behavior:
  - starts an ops run, emits progress events, runs local verification by default for localhost, runs prod E2E smoke for prod-ish base URLs, always finishes the ops run with a summary JSON.
- Tests:
  - `apps/web/tests/scripts/dse-overnight.test.ts` (arg parsing + start/event/finish emission with mocked fetch)

- 2026-02-10T08:39:56Z Phase 3: dataset fixture + admin-secret import endpoint.
- Added canonical dataset fixture:
  - `docs/autopilot/fixtures/dse-selecttool.dataset.v1.jsonl` (30+ examples, 10+ holdout)
- Added admin-secret gated import endpoint:
  - `POST /api/dse/examples/import` (upserts `dseExamples` via `putExample`, optional `opsRunId` emits ops-run events)
- Tests:
  - `apps/web/tests/fixtures/dse-selecttool-dataset.test.ts` (fixture integrity + splits)
  - `apps/web/tests/worker/dse-examples-import-endpoint.test.ts` (endpoint shape + upsert calls)

- 2026-02-10T09:00:40Z Phase 4: centralized compile job spec + made SelectTool compile non-trivial.
- Centralized job spec definition (single source of truth):
  - `apps/web/src/effuse-host/dseJobs.ts`
  - shared by both compile (`apps/web/src/effuse-host/dseCompile.ts`) and gating (`apps/web/src/effuse-host/dseAdmin.ts`)
- Made `@openagents/autopilot/blueprint/SelectTool.v1` compile non-trivial:
  - instruction-variant search space (4 variants) under `instruction_grid.v1` (deterministic reward remains `exact_json_match`)
- Tests / verification:
  - `apps/web/tests/worker/dse-compile-endpoint.test.ts` asserts instruction variants + candidate count >= 2
  - `apps/web/tests/worker/dse-admin-jobhash-gating.test.ts` asserts promote/canary gating uses the shared job hash
  - `cd apps/web && npm run lint` (ok)
  - `cd apps/web && npm test` (ok)

- 2026-02-10T09:25:39Z Phase 5: automated prod loop wiring (compile -> canary -> traffic -> monitor -> promote/stop) + endpoints.
- Extended the canonical overnight runner to perform the Phase 5 loop and log every step to Convex:
  - `apps/web/scripts/dse-overnight-lib.ts`
- Added admin-secret gated “monitor/exercise” endpoints (Workers):
  - `GET /api/dse/canary/status?signatureId=...` (poll `dseCanaries` counters)
  - `POST /api/dse/exercise/thread/ensure` (creates/returns an ops-owned thread id)
  - `POST /api/dse/exercise/predict` (runs N predicts for a signature on dataset examples, records receipts, drives canary counters)
  - Impl: `apps/web/src/effuse-host/dseAdmin.ts`
- Tests / verification:
  - `apps/web/tests/worker/dse-exercise-endpoints.test.ts` (new endpoints)
  - `apps/web/tests/scripts/dse-overnight.test.ts` (runner sequencing + failure cleanup)
  - `cd apps/web && npm run lint` (ok)
  - `cd apps/web && npm test` (ok)

- 2026-02-10T10:13:11Z Phase 6: read-only visualization pages (Effuse) backed by Convex.
- Added admin-only `/dse/*` pages (read-only):
  - `/dse` ops runs list
  - `/dse/ops/:runId` ops run detail + events timeline
  - `/dse/signature/:signatureId` per-signature view (active pointer/history, canary status/history, compile reports, dataset examples, receipts)
  - `/dse/compile-report/:jobHash/:datasetHash/:signatureId` compile report JSON view
- Added Convex read queries needed for the pages:
  - `dse.opsRuns.listRuns`, `dse.opsRuns.getRun`, `dse.opsRuns.listRunEvents` (admin-only)
  - `dse.active.listActiveHistory`
  - `dse.canary.listCanaryHistory`
  - `dse.receipts.listPredictReceiptsBySignatureIdAdmin` (admin-only list)
- Made receipt/blob debug endpoints usable for ops admin sessions (and admin-secret mode):
  - `/api/dse/receipt/:receiptId` uses admin query when session subject is `user_dse_admin`
  - `/api/dse/blob/:receiptId/:blobId` uses admin query + `getTextAdmin` when session subject is `user_dse_admin`
- Fixed Worker asset routing false-positives for dotted signature ids (e.g. `SelectTool.v1`) so `/dse/signature/...` reaches SSR.
- Tests / verification:
  - `cd apps/web && npx convex codegen` (ok)
  - `cd apps/web && npm run lint` (ok)
  - `cd apps/web && npm test` (ok)

- 2026-02-10T11:08:58Z Phase 8: headless trace mining pipeline (review -> export -> tagging) (`dc8b1f2f2`).
- Added an ops-admin receipt listing endpoint (headless via `OA_DSE_ADMIN_SECRET`):
  - `GET /api/dse/receipts/list?signatureId=...&limit=...&requireRlmTrace=1&resultTag=Ok&strategyId=rlm_lite.v1`
  - implementation: `apps/web/src/effuse-host/dseAdmin.ts`
- Extended trace export to persist structured linkage metadata on exported examples:
  - `/api/dse/trace/export` now writes `meta.kind=openagents.trace_export.v1` with `receiptId`, `threadId/runId`, `rlmTrace.blobId`, `strategyId`, `compiled_id`.
  - implementation: `apps/web/src/effuse-host/dseAdmin.ts`, test: `apps/web/tests/worker/dse-trace-export.test.ts`
- Added a headless miner script:
  - `apps/web/scripts/dse-trace-mine.ts` (CLI) + `apps/web/scripts/dse-trace-mine-lib.ts` (testable)
  - Uses Bearer auth and calls: receipts list -> trace export (writes to `dseExamples`)
  - Auto-tags exported rows with `trace_mined` + any user-provided tags.
  - tests: `apps/web/tests/scripts/dse-trace-mine.test.ts`, `apps/web/tests/worker/dse-receipts-list.test.ts`
- Tests / verification:
  - `cd apps/web && npm run lint` (ok)
  - `cd apps/web && npm test` (ok)

- 2026-02-10T05:40:55Z Autopilot UI: DSE signature cards collapsed by default; prod E2E updated to expand before asserting debug visibility.
- `/autopilot` now renders `dse.signature` parts as a one-line summary + expandable details:
  - implementation: `apps/web/src/effuse-pages/autopilot.ts` (`<details data-dse-signature-details="1">`)
- Updated prod E2E `apps-web.prod.autopilot.dse-canary-recap-shows-debug-card-and-trace` to:
  - wait for the recap signature card
  - expand it (`summary.click()`)
  - assert strategy + counters + trace link are visible inside the expanded card
  - implementation: `packages/effuse-test/src/suites/apps-web.ts`
- Tests / verification:
  - `cd apps/web && npm run lint` (ok)
  - `cd apps/web && npm test` (ok)
  - `cd packages/effuse-test && bun run typecheck` (ok)
  - `cd packages/effuse-test && bun run test` (ok)

- 2026-02-10T11:22:35Z Phase 9: compiler-visible knobs for RLM-lite compilation (controller/chunking/roles/budgets) with Convex-stored compile reports (`2941dfa0c`).
- Extended recap/summarization compile jobs to use Phase G knob search spaces:
  - controller instruction variants (`rlmControllerInstructionVariants`)
  - chunking policy variants (`rlmChunkingPolicyVariants`)
  - sub-role selection (`rlmSubRoleVariants`)
  - budget profiles (`budgetProfiles`)
  - optimizer: `knobs_grid_refine.v1` (bounded)
  - implementation: `apps/web/src/effuse-host/dseJobs.ts`
- Added a small predict-cost penalty signal to the recap judge reward so budget profiles are meaningfully selectable.
  - implementation: `apps/web/src/effuse-host/dseJobs.ts`
- Tests:
  - `apps/web/tests/worker/dse-compile-endpoint.test.ts` asserts recap compile jobs include knob search spaces and store a compile report.
- Tests / verification:
  - `cd apps/web && npm run lint` (ok)
  - `cd apps/web && npm test` (ok)
