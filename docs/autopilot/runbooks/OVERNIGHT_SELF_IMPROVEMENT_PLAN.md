# Overnight Self Improvement Plan (Agent-Run, Khala-First, Prod-Verified)

- **Status:** Implemented (Phases 1–9 shipped in code; this doc is the runbook/checklist)
- **Last updated:** 2026-02-10
- **Legacy scope:** This runbook documents the former `apps/web` Worker + Khala loop and is retained for historical/operator context.
- **Priority:** programmatic loop first; UI is read-only later
- **Primary refs (do not duplicate):**
  - Operator workflow + current endpoints: `docs/autopilot/runbooks/DSE_PLAYBOOK.md`
  - System plan + Khala-first constraints: `docs/autopilot/runbooks/SELF_IMPROVE_PLAN.md`
  - RLM program + phases: `docs/autopilot/dse/RLM_UNIFIED_ROADMAP.md`
  - Prod E2E harness: `docs/autopilot/testing/PROD_E2E_TESTING.md`
  - UI determinism posture: `docs/autopilot/testing/STREAM_TESTING.md`
  - Deep spec/intent: `docs/autopilot/dse/dse.md`
- **If this doc conflicts with code behavior:** code wins

This is the single overnight plan to make Autopilot *actually* self-improve in a loop in production:

- **Agents (Codex and others) can run it programmatically** (no clicking required).
- **All evidence is stored in Khala** for later visualization (read-only pages).
- **Prod is verified** (E2E + request-id correlation), and changes ship behind canary/promotion gates.

## Non-Goals (Overnight)

- Building any “control UI” for compile/promote/canary. (We’ll add read-only visualization later.)
- Fully automated *promotion* for subjective outputs (recaps/summaries) without human review. (We can judge + store eval reports, but promotion policy needs conservatism.)

## Definition Of Done (By Morning)

We consider “overnight self-improve” successful when, for at least one target signature:

- We have a real dataset in Khala (`dseExamples`) with a holdout split.
- We ran compile in prod and stored:
  - `dseCompileReports` (jobHash + datasetHash)
  - `dseArtifacts` (compiled artifact)
- We ran either:
  - a canary rollout (stored in `dseCanaries` + `dseCanaryHistory`), or
  - a promotion (stored in `dseActiveArtifacts` + `dseActiveArtifactHistory`).
- We have an **ops run record** in Khala (new) that links everything (commit sha, signature ids, compile report ids, promoted ids, E2E summary).
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

## Khala: What Must Be Stored (Source Of Truth)

Already implemented tables (see `apps/web/khala/schema.ts`):

- Datasets: `dseExamples`
- Compile outputs: `dseArtifacts`, `dseCompileReports`
- Runtime pointers + history: `dseActiveArtifacts`, `dseActiveArtifactHistory`
- Canary config + history: `dseCanaries`, `dseCanaryHistory`
- RLM persistence: `dseBlobs`, `dseVarSpace`
- Ops runs: `dseOpsRuns`, `dseOpsRunEvents`
- Eval reports (judge-based rewards): `dseEvalReports`
- Predict receipts: `receipts` with `kind="dse.predict"` (+ `receiptId`, `signatureId`, `compiled_id`)

Why: later visualization should not require scraping local logs or `output/effuse-test/*`.

## How To Run (Headless, One Command)

Worker secrets that must be present in the deployed Worker environment:

- `OA_DSE_ADMIN_SECRET` (authorizes DSE ops endpoints via `Authorization: Bearer ...`)
- `OA_E2E_JWT_PRIVATE_JWK` (used by the Worker to mint an ops-admin JWT for Khala access)
- (prod E2E only) `OA_E2E_BYPASS_SECRET` (enables deterministic login via `/api/auth/e2e/*`)
  - Runner-side env `EFFUSE_TEST_E2E_BYPASS_SECRET` must match this secret
- `OPENROUTER_API_KEY` (required for some compile/eval paths; RLM-lite trigger is gated without it)

To sync secrets from `.env.production` into Cloudflare Worker secrets:

```bash
cd apps/web
npm run wrangler:secrets
```

Important:

- `npm run wrangler:secrets` only pushes keys that are set in `apps/web/.env.production`. Missing keys are skipped.
- In particular, ops-admin + prod E2E typically require setting these Worker secrets explicitly:
  - `OA_DSE_ADMIN_SECRET`
  - `OA_E2E_JWT_PRIVATE_JWK` (RSA private JWK JSON)
  - `OA_E2E_BYPASS_SECRET`

Suggested secret bootstrap (run from `apps/web`):

```bash
# 1) Headless DSE ops admin bearer secret (any random string is fine)
node -e 'console.log(`oa_dse_${require(\"crypto\").randomUUID()}`)' | npx wrangler secret put OA_DSE_ADMIN_SECRET

# 2) E2E JWT signing key (RSA private JWK, used for ops-admin Khala JWT minting too)
node --input-type=module -e 'import { generateKeyPair, exportJWK } from \"jose\"; const { privateKey } = await generateKeyPair(\"RS256\", { extractable: true }); console.log(JSON.stringify(await exportJWK(privateKey)));' | npx wrangler secret put OA_E2E_JWT_PRIVATE_JWK

# 3) E2E bypass secret (must match runner-side EFFUSE_TEST_E2E_BYPASS_SECRET)
node -e 'console.log(`oa_e2e_${require(\"crypto\").randomUUID()}`)' | npx wrangler secret put OA_E2E_BYPASS_SECRET
```

Notes:

- Rotating `OA_E2E_JWT_PRIVATE_JWK` changes the public keys served at `/api/auth/e2e/jwks`; Khala verifies E2E/ops-admin JWTs against that JWKS URL (see `apps/web/khala/auth.config.ts`).
- After rotating secrets, deploy the Worker (`cd apps/web && npm run deploy:worker`) before running the overnight loop.

Local prerequisites (run once per session, in a separate terminal):

```bash
cd apps/web
npm run dev
```

Local notes:

- `wrangler dev` loads `.env.local`/`.env` (non-secret vars). For ops-admin mode you must also provide:
  - `OA_DSE_ADMIN_SECRET`
  - `OA_E2E_JWT_PRIVATE_JWK`
- The overnight runner uses `OA_DSE_ADMIN_SECRET`; it must match the Worker value.
- `OA_E2E_JWT_PRIVATE_JWK` must correspond to the public key served at `https://openagents.com/api/auth/e2e/jwks` (see `apps/web/khala/auth.config.ts`).
  - You cannot generate a random key for local ops-admin mode: Khala will reject admin JWTs with `InvalidAuthHeader` (kid/JWKS mismatch).

Recommended local dev pattern (do not commit secrets):

```bash
cd apps/web
cp .env.dse.local.example .env.dse.local
wrangler dev --port 3001 --env-file .env.local --env-file .env.dse.local
```

Local (no E2E):

```bash
OA_DSE_ADMIN_SECRET="..." \
  bun run apps/web/scripts/dse-overnight.ts --base-url http://localhost:3000 --verify --no-e2e
```

Prod-ish (runs E2E smoke by default; requires E2E bypass secret for deterministic login):

```bash
OA_DSE_ADMIN_SECRET="..." \
EFFUSE_TEST_E2E_BYPASS_SECRET="..." \
  bun run apps/web/scripts/dse-overnight.ts --base-url https://openagents.com
```

By default, the overnight runner runs only the single “DSE recap visibility” prod E2E test.
To run the full Autopilot prod suite instead:

```bash
OA_DSE_ADMIN_SECRET="..." \
EFFUSE_TEST_E2E_BYPASS_SECRET="..." \
  bun run apps/web/scripts/dse-overnight.ts --base-url https://openagents.com --e2e-grep "apps-web\\.prod\\.autopilot"
```

Where to look for results (read-only pages backed by Khala):

- `/dse` (ops run list)
- `/dse/ops/:runId` (timeline + links)
- `/dse/signature/:signatureId` (active pointer + compile/canary history + dataset/receipts)

Note: the CLI `apps/web/scripts/dse-overnight.ts` auto-sets its working directory to the repo root, so it can be run from any `cwd`.

## Phase Plan (Programmatic Loop First)

This plan is **9 phases**.

- Phases 1–6 make the loop agent-runnable and Khala-backed (plus read-only visualization).
- Phases 7–9 extend rewards/eval/trace-mining/knobs so we can self-improve non-discrete long-context signatures.

### Phase 1: Programmatic Ops Auth + Run Recording (Khala)

Objective: agents can operate `/api/dse/*` without a browser session, and every overnight run is recorded in Khala.

Deliverables:

- Worker secrets:
  - `OA_DSE_ADMIN_SECRET`
  - `OA_E2E_JWT_PRIVATE_JWK` (mint ops-admin JWT for Khala)
- Admin auth mode for DSE endpoints:
  - accept `Authorization: Bearer <OA_DSE_ADMIN_SECRET>` as an alternative to WorkOS session cookies
  - applies to: compile/promote/canary/trace-export + new ops endpoints
- Khala tables:
  - `dseOpsRuns`: `{ runId, startedAtMs, endedAtMs?, commitSha, baseUrl, actor, status, signatureIds, notes?, links... }`
  - `dseOpsRunEvents`: `{ runId, tsMs, phase, level, message, json? }`
- Worker endpoints (admin-secret gated):
  - `POST /api/dse/ops/run/start`
  - `POST /api/dse/ops/run/event`
  - `POST /api/dse/ops/run/finish`

Exit criteria:

- A headless script can start a run, append events, and finish a run, and the data is visible in Khala.

### Phase 2: Programmatic Runner (One Command) + Baseline Checks

Objective: one command runs the overnight loop steps deterministically and logs everything to Khala.

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

### Phase 3: Dataset Ingestion (Khala) For SelectTool

Objective: agents can seed/update a real dataset in Khala without manual UI.

Deliverables:

- A canonical dataset file committed to the repo (stable ids):
  - Suggested: `docs/autopilot/testing/fixtures/dse-selecttool.dataset.v1.jsonl`
  - Each row includes: `exampleId`, `inputJson`, `expectedJson`, `split`, `tags`
- A programmatic import path:
  - Option A (preferred): a Worker endpoint `POST /api/dse/examples/import` (admin-secret gated) that upserts into `dseExamples`.
  - Option B: the overnight script calls Khala directly with a privileged token (harder in prod).
- Dataset rules:
  - at least ~30 examples
  - at least 10 holdout examples (`split="holdout"`)
  - tags include `overnight`, `selecttool`, dataset version

Exit criteria:

- Re-running the import is idempotent (upsert by `signatureId+exampleId`).
- `POST /api/dse/compile` against prod uses the Khala dataset.

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

Objective: agents can run the full loop in prod, safely, with all outcomes recorded in Khala.

Deliverables:

- Overnight script performs:
  1. compile (prod): `POST /api/dse/compile`
  2. start canary (prod): `POST /api/dse/canary/start`
  3. generate traffic to reach `minSamples` quickly:
     - run `packages/effuse-test` prod tests that exercise `/autopilot` messages
     - optionally add a dedicated “signature exerciser” endpoint for SelectTool to generate samples deterministically
  4. monitor canary counters via Khala queries (`dseCanaries`) until:
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

- Store E2E summary in Khala (`dseOpsRuns` + events):
  - pass/fail
  - failing test ids
  - request ids (`x-oa-request-id`) when available

Exit criteria:

- By morning, Khala contains:
  - dataset rows, compile report, artifact, canary history, and either promotion or rollback history
  - plus a single `dseOpsRuns` record linking them all.

### Phase 6: Read-Only Visualization

Objective: display the Khala-stored loop evidence on web pages, without control surfaces.

Deliverables:

- Read-only pages that show:
  - recent ops runs (`dseOpsRuns`)
  - per-signature history (active pointer changes, canary history, compile reports)
  - datasets + example diffs
  - links to receipts + traces

Implemented pages (Effuse, Khala-backed):

- `/dse` ops runs list (`apps/web/src/effuse-pages/dseOpsRuns.ts`)
- `/dse/ops/:runId` ops run detail + events timeline (`apps/web/src/effuse-pages/dseOpsRunDetail.ts`)
- `/dse/signature/:signatureId` per-signature view (`apps/web/src/effuse-pages/dseSignature.ts`)
- `/dse/compile-report/:jobHash/:datasetHash/:signatureId` compile report JSON view (`apps/web/src/effuse-pages/dseCompileReport.ts`)
- `/dse/eval-report/:evalHash/:signatureId` eval report JSON view (`apps/web/src/effuse-pages/dseEvalReport.ts`)

Exit criteria:

- A non-operator can answer “what improved last night?” by reading web pages backed by Khala.

### Phase 7: Judge Rewards For Non-Discrete Outputs

Objective: enable overnight compile/eval on recap/summarization signatures where “exact match” is not meaningful.

Deliverables:

- Add a judge signature (and pin its artifact):
  - judge signature id: `@openagents/autopilot/judge/ThreadSummaryQuality.v1`
  - pinned judge artifact: `apps/web/src/effuse-host/dsePinnedArtifacts.ts`
- Add a judge-based reward bundle for recap/summarization signatures:
  - reward bundle: `reward_thread_summary_judge.v1` in `apps/web/src/effuse-host/dseJobs.ts`
  - applied to:
    - `@openagents/autopilot/canary/RecapThread.v1`
    - `@openagents/autopilot/rlm/SummarizeThread.v1`
- Store eval reports in Khala:
  - table: `dseEvalReports`
  - endpoint: `POST /api/dse/eval`
  - read-only pages for report inspection

Exit criteria:

- Recap/summarization compile/eval is possible with a pinned judge and a Khala-stored report that can be audited later.

### Phase 8: Trace Mining Scale-Up (Headless)

Objective: convert RLM traces into labeled examples in `dseExamples` without any UI.

Deliverables:

- A headless “receipt list” endpoint (ops-admin):
  - `GET /api/dse/receipts/list?signatureId=...&limit=...&requireRlmTrace=1&resultTag=Ok&strategyId=rlm_lite.v1`
- A trace export endpoint that can upsert examples with provenance:
  - `POST /api/dse/trace/export` writes to `dseExamples` and stores linkage metadata in `meta`
- A headless miner script:
  - `apps/web/scripts/dse-trace-mine.ts` (CLI)
  - uses Bearer admin-secret auth and calls: receipts list -> trace export

Exit criteria:

- We can generate new dataset rows from production traces programmatically and link back to receipts/blobs.

### Phase 9: Knobs + Compiler Search Spaces (RLM + Distilled Pipelines)

Objective: make the knobs that matter compiler-visible (don’t hand-tweak prompts) for long-context strategies.

Deliverables:

- Add search spaces to recap/summarization compile jobs:
  - controller instruction variants
  - chunking policy variants
  - sub-role selection variants
  - budget profile variants
- Keep everything auditable in compile reports:
  - `dseCompileReports` includes the job spec, candidates, and best selection

Exit criteria:

- RLM-lite strategy configs and distilled pipeline params can be compiled into artifacts and compared with eval-backed selection.

## Notes / Current Code Surface

Current endpoints and storage:

- Ops run recording:
  - `POST /api/dse/ops/run/start`
  - `POST /api/dse/ops/run/event`
  - `POST /api/dse/ops/run/finish`
- Dataset import:
  - `POST /api/dse/examples/import`
- Compile/eval:
  - `POST /api/dse/compile` (`apps/web/src/effuse-host/dseCompile.ts`)
  - `POST /api/dse/eval`
- Canary + promotion:
  - `POST /api/dse/canary/start`, `POST /api/dse/canary/stop`
  - `GET /api/dse/canary/status?signatureId=...`
  - `POST /api/dse/promote`
- Exercisers (Phase 5 traffic generation):
  - `POST /api/dse/exercise/thread/ensure`
  - `POST /api/dse/exercise/predict`
- Trace mining:
  - `GET /api/dse/receipts/list?...` (ops-admin)
  - `POST /api/dse/trace/export`
- Debug reads:
  - `GET /api/dse/receipt/:receiptId`
  - `GET /api/dse/blob/:receiptId/:blobId`
- Most admin endpoints live in `apps/web/src/effuse-host/dseAdmin.ts` and are auth-gated by either:
  - browser session cookies (WorkOS), or
  - `Authorization: Bearer <OA_DSE_ADMIN_SECRET>` (headless ops mode).
- Khala DSE tables/functions: `apps/web/khala/dse/*`
- Target signatures: `apps/autopilot-worker/src/dseCatalog.ts`
- Prod E2E suite: `packages/effuse-test/src/suites/apps-web.ts`

## Implementation Log

- 2026-02-10T08:19:24Z Phase 1: programmatic DSE ops auth + Khala ops run recording.
- Added Worker env typing + admin-secret auth helper:
  - `OA_DSE_ADMIN_SECRET` support (`Authorization: Bearer <OA_DSE_ADMIN_SECRET>`)
  - Worker-minted admin JWT (subject=`user_dse_admin`) via `OA_E2E_JWT_PRIVATE_JWK` for headless Khala access.
- Added Khala ops persistence:
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
  - `POST /api/dse/trace/export` (includes admin-only Khala reads for receipt/blob).
- Tests / verification:
  - `cd apps/web && npx khala codegen` (ok)
  - `cd apps/web && npm run lint` (ok)
  - `cd apps/web && npm test` (ok)

- 2026-02-10T04:54:49Z Phase 7: judge-based rewards for recap/summarization + Khala-stored eval reports.
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
- Added Khala eval reports:
  - table: `dseEvalReports` in `apps/web/khala/schema.ts`
  - queries/mutations: `apps/web/khala/dse/evalReports.ts`
  - tests: `apps/web/tests/khala/dse-eval-reports.test.ts`
- Added Worker endpoint:
  - `POST /api/dse/eval` (stores a stable `evalHash` + full JSON report into Khala)
  - test: `apps/web/tests/worker/dse-eval-endpoint.test.ts` asserts judge pin info is present in stored report JSON
- Added read-only visualization for eval reports:
  - signature page now lists eval reports
  - new `/dse/eval-report/:evalHash/:signatureId` detail page
  - test: `apps/web/tests/worker/routes.test.ts`
- Tests / verification:
  - `cd apps/web && npx khala codegen` (ok)
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
  - `docs/autopilot/testing/fixtures/dse-selecttool.dataset.v1.jsonl` (30+ examples, 10+ holdout)
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
- Extended the canonical overnight runner to perform the Phase 5 loop and log every step to Khala:
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

- 2026-02-10T10:13:11Z Phase 6: read-only visualization pages (Effuse) backed by Khala.
- Added admin-only `/dse/*` pages (read-only):
  - `/dse` ops runs list
  - `/dse/ops/:runId` ops run detail + events timeline
  - `/dse/signature/:signatureId` per-signature view (active pointer/history, canary status/history, compile reports, dataset examples, receipts)
  - `/dse/compile-report/:jobHash/:datasetHash/:signatureId` compile report JSON view
- Added Khala read queries needed for the pages:
  - `dse.opsRuns.listRuns`, `dse.opsRuns.getRun`, `dse.opsRuns.listRunEvents` (admin-only)
  - `dse.active.listActiveHistory`
  - `dse.canary.listCanaryHistory`
  - `dse.receipts.listPredictReceiptsBySignatureIdAdmin` (admin-only list)
- Made receipt/blob debug endpoints usable for ops admin sessions (and admin-secret mode):
  - `/api/dse/receipt/:receiptId` uses admin query when session subject is `user_dse_admin`
  - `/api/dse/blob/:receiptId/:blobId` uses admin query + `getTextAdmin` when session subject is `user_dse_admin`
- Fixed Worker asset routing false-positives for dotted signature ids (e.g. `SelectTool.v1`) so `/dse/signature/...` reaches SSR.
- Tests / verification:
  - `cd apps/web && npx khala codegen` (ok)
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

- 2026-02-10T12:51:12Z Docs: added a concrete “secret bootstrap/rotation” section for ops-admin + prod E2E (`OA_DSE_ADMIN_SECRET`, `OA_E2E_JWT_PRIVATE_JWK`, `OA_E2E_BYPASS_SECRET`) and clarified that `npm run wrangler:secrets` skips missing keys.
- Updated:
  - `docs/autopilot/runbooks/OVERNIGHT_SELF_IMPROVEMENT_PLAN.md`

- 2026-02-10T12:56:01Z Docs/Ops: updated env example files so the overnight loop required secrets are discoverable and can be copied into non-committed `.env.production` / `.env.local`.
- Updated:
  - `apps/web/.env.production.example`
  - `apps/web/.env.local.example`

- 2026-02-10T13:03:12Z Docs/Ops: added a dedicated local secrets file pattern for ops-admin testing (`.env.dse.local`) and documented copying the example before starting `wrangler dev`.
- Updated:
  - `apps/web/.gitignore` (ignore `.env.dse.local`)
  - `apps/web/.env.dse.local.example`
  - `docs/autopilot/runbooks/OVERNIGHT_SELF_IMPROVEMENT_PLAN.md`

- 2026-02-10T13:05:15Z Ops: updated repo root `.gitignore` to allow committing `.env*.example` files (while still ignoring real `.env*` secrets), so `.env.dse.local.example` is trackable.
- Updated:
  - `.gitignore`

- 2026-02-10T13:21:05Z Ops: wired up production so headless ops-admin calls actually work end-to-end.
- Added missing Worker secret:
  - `OA_DSE_ADMIN_SECRET` (Cloudflare Worker `autopilot-web`)
- Deployed to production:
  - `cd apps/web && npm run deploy:worker`
  - `cd apps/web && npm run deploy:khala` (required because Khala prod was missing `dse/opsRuns:*` functions)
- Verified:
  - created + finished a smoke ops run in prod: `opsrun_smoke_e9780d37b613406cbc113f92f6b7f2ee`

- 2026-02-10T12:48:56Z Ops: ran a local headless ops-admin smoke attempt and confirmed a concrete failure mode:
  - Khala rejects ops-admin JWTs with `InvalidAuthHeader` when `OA_E2E_JWT_PRIVATE_JWK` does not match the JWKS Khala trusts (`apps/web/khala/auth.config.ts` -> `https://openagents.com/api/auth/e2e/jwks`).
- Hardened the overnight runner: if `/api/dse/ops/run/start` fails, the CLI now still emits a machine-readable JSON summary (previously it threw and emitted no summary).
- Updated:
  - `apps/web/scripts/dse-overnight-lib.ts`
  - `apps/web/tests/scripts/dse-overnight.test.ts`
  - `docs/autopilot/runbooks/OVERNIGHT_SELF_IMPROVEMENT_PLAN.md` (local ops-admin notes + recommended `wrangler dev --env-file ...` pattern)
- Tests / verification:
  - `cd apps/web && npm run lint` (ok)
  - `cd apps/web && npm test` (ok)

- 2026-02-10T12:26:37Z Docs: clarified local prerequisites for running the overnight loop against `http://localhost:3000` (start `npm run dev`, secret alignment notes).
- Updated:
  - `docs/autopilot/runbooks/OVERNIGHT_SELF_IMPROVEMENT_PLAN.md`

- 2026-02-10T12:24:42Z Ops: propagate `x-oa-request-id` into overnight runner HTTP errors (so failures are correlatable from the CLI summary + ops run events).
- Updated:
  - `apps/web/scripts/dse-overnight-lib.ts`
  - `apps/web/tests/scripts/dse-overnight.test.ts`
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

- 2026-02-10T11:45:19Z Doc maintenance: reconciled this plan with the implemented system + added headless run instructions.
- Updated plan status to reflect Phases 1–9 are implemented in code.
- Updated secrets guidance for admin-secret ops mode:
  - `OA_DSE_ADMIN_SECRET` (client-side Bearer authorization)
  - `OA_E2E_JWT_PRIVATE_JWK` (Worker-side minting for Khala ops-admin JWT)
- Clarified prod E2E bypass secrets:
  - Worker secret: `OA_E2E_BYPASS_SECRET`
  - Runner env: `EFFUSE_TEST_E2E_BYPASS_SECRET` (must match)
- Added a “How To Run (Headless, One Command)” section with canonical CLI examples and read-only result pages.

- 2026-02-10T11:49:02Z Doc maintenance: filled in Phases 7–9 sections and corrected “Non-Goals” to match the implemented judge/eval capability.

- 2026-02-10T11:53:16Z Docs: updated operator-facing guidance to match the new collapsed `dse.signature` UI on `/autopilot`.
- Updated:
  - `docs/autopilot/runbooks/DSE_PLAYBOOK.md`
  - `docs/autopilot/runbooks/SELF_IMPROVE_RUNBOOK.md`
  - `docs/autopilot/dse/rlm-trace-mining.md`

- 2026-02-10T11:56:25Z Docs: updated stream testing doc status + commands to match current implementation.
- Updated:
  - `docs/autopilot/testing/STREAM_TESTING.md`

- 2026-02-10T11:58:43Z Doc maintenance: updated Phase 6 + code surface section to match shipped read-only pages and the current Worker API surface.

- 2026-02-10T12:01:48Z Docs: updated `docs/autopilot/runbooks/DSE_PLAYBOOK.md` to include the canonical headless overnight runner and link back to this runbook.

- 2026-02-10T12:04:12Z Ops: updated the Worker secret sync script to include DSE/RLM overnight secrets.
- Updated:
  - `apps/web/scripts/sync-wrangler-secrets.sh`

- 2026-02-10T12:06:43Z Docs: updated `docs/autopilot/runbooks/SELF_IMPROVE_PLAN.md` to reflect that the staged plan is now implemented and the canonical execution runbook is this doc.

- 2026-02-10T12:10:22Z Docs: updated `docs/autopilot/testing/PROD_E2E_TESTING.md` to reference the overnight runner + `npm run wrangler:secrets` for syncing required prod E2E secrets.

- 2026-02-10T12:13:09Z Ops: made the overnight runner CLI resilient to being invoked from any working directory (auto-`chdir` to repo root).
- Updated:
  - `apps/web/scripts/dse-overnight.ts`

- 2026-02-10T12:17:41Z Ops: fixed the overnight runner to respect `--e2e-grep` (and added a unit test).
- Updated:
  - `apps/web/scripts/dse-overnight-lib.ts`
  - `apps/web/tests/scripts/dse-overnight.test.ts`
- 2026-02-10T11:22:35Z Phase 9: compiler-visible knobs for RLM-lite compilation (controller/chunking/roles/budgets) with Khala-stored compile reports (`2941dfa0c`).
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

- 2026-02-10T14:03:37Z Ops: hardened the overnight runner so failures are actionable from CLI output (not just Khala).
- Fixed `runOvernight` to include the `phase5` object in stdout JSON (it was only stored in Khala via `/ops/run/finish` previously).
- Wrapped fetch failures in the overnight runner with path + timeout context:
  - e.g. `dse_admin_fetch_failed path=/api/dse/compile timeoutMs=... err=...`
- Added tests:
  - `apps/web/tests/scripts/dse-overnight.test.ts` asserts `phase5` is present and timeouts include `path=...` and `timeoutMs=...`.
- Prod debug: re-ran the headless overnight loop against `https://openagents.com` (no E2E) and confirmed prod compile was hanging:
  - ops run id: `opsrun_9a1d8b92-a8ad-4af3-83f5-b2e57759eb5f`
  - failure: `dse_admin_fetch_failed path=/api/dse/compile timeoutMs=600000 err=The operation timed out.`
  - direct `curl` to `/api/dse/compile` also received 0 bytes for 900s (client timeout).
- Fix: added an OpenRouter LM hard timeout + per-request circuit breaker so we can fall back to Workers AI instead of stalling indefinitely:
  - `apps/web/src/effuse-host/dse.ts`:
    - OpenRouter fetch timeout (default 15s)
    - once OpenRouter fails, disable it for the remainder of the request (compile/eval can issue many subcalls)
- Tests / verification:
  - `cd apps/web && npm run lint` (ok)
  - `cd apps/web && npm test` (ok)

- 2026-02-10T14:56:01Z Prod unblock: made Phase 5 “compile -> canary -> traffic -> promote” complete end-to-end in production via the headless runner.
- Fixed canary gating bugs:
  - `/api/dse/canary/start` now computes `datasetHash` including example `tags`, matching `/api/dse/compile` (previously omitted tags -> `compile_report_not_found`).
  - `/api/dse/canary/start` now seeds an explicit baseline control artifact from `signature.defaults.params` when no active pointer exists (previously `control_missing` blocked first canary).
  - test coverage: `apps/web/tests/worker/dse-admin-jobhash-gating.test.ts` asserts canary/promote look up compile reports with the tag-inclusive `datasetHash`.
- Made the ops “traffic generator” usable within Bun’s implicit ~5m HTTP timeout:
  - `/api/dse/exercise/predict` now runs predicts with bounded parallelism (`concurrency <= 4`) instead of fully serial.
- Made prod compile feasible and deterministic:
  - Switched DSE endpoints default Workers AI model to `@cf/openai/gpt-oss-20b` (lower latency, still 128k context):
    - `apps/web/src/effuse-host/dseCompile.ts`
    - `apps/web/src/effuse-host/dseAdmin.ts`
  - Increased offline eval/compile throughput: `packages/dse/src/eval/evaluate.ts` now evaluates examples with `concurrency: 4` (was 1).
- Deployed Worker to production multiple times during this iteration (`cd apps/web && npm run deploy:worker`) to validate fixes immediately.
- Verified prod compile succeeds:
  - request id: `dbg_compile_selecttool_prod_4`
  - compiled_id: `sha256:862f69e8a655c716e8eac0fe22fcfbdcf304702a8c729fa3a91e67cd2a9ee61a`
  - datasetHash: `sha256:57e3ad2d003681f24eda6cf416da26d3299ea57c6bc324505ad878ce99b5d286`
  - subsequent compiles return cached (`existed: true`).
- Verified full prod overnight loop (no E2E) succeeds:
  - ops run id: `opsrun_4796ab2c-1544-4f17-9d88-500d171c454e`
  - canary clean at `minSamples=20` (`errorRate=0`)
  - promoted control -> canary:
    - from: `sha256:414e0b59c4b40c53169ac5a178e18544adaedb79b0870d77622f2af7ef473737`
    - to: `sha256:862f69e8a655c716e8eac0fe22fcfbdcf304702a8c729fa3a91e67cd2a9ee61a`
- Tests / verification:
  - `cd packages/dse && bun test && bun run typecheck` (ok)
  - `cd apps/web && npm run lint && npm test` (ok)
