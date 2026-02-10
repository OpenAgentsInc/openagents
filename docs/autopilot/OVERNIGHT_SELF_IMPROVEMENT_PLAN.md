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
