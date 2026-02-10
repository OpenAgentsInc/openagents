# Overnight Self Improvement Plan (DSE + RLM-lite, Convex-First, Prod-Verified)

- **Status:** Proposed (operator + implementation plan)
- **Last updated:** 2026-02-10
- **Primary docs to align with (do not duplicate):**
  - Operator workflow: `docs/autopilot/DSE_PLAYBOOK.md`
  - System plan + data model: `docs/autopilot/SELF_IMPROVE_PLAN.md`
  - RLM program + phases: `docs/autopilot/RLM_UNIFIED_ROADMAP.md`
  - Prod E2E harness: `docs/autopilot/PROD_E2E_TESTING.md`
  - UI determinism via transcripts: `docs/autopilot/STREAM_TESTING.md`
  - Deep spec/intent: `docs/autopilot/dse.md`
- **If this doc conflicts with code behavior:** code wins

This is a single, end-to-end overnight plan that:

1. **Implements** the missing pieces needed for a real DSE improvement loop (Convex-backed, replayable).
2. **Runs the loop in production** (including prod E2E) with safe canary + rollback posture.
3. Ensures **all relevant data is stored in Convex** so we can later build web pages that show:
   - what ran,
   - what we changed,
   - why we promoted,
   - and what happened under canary.

## Definition Of Done (By Morning)

We consider “overnight self-improve” successful when:

1. A target Signature has:
   - a non-trivial dataset in Convex (`dseExamples`) with a holdout split,
   - at least one compile report in Convex (`dseCompileReports`),
   - at least one compiled artifact stored (`dseArtifacts`),
   - and either a canary rollout or a promotion recorded with history (`dseCanaries` / `dseActiveArtifactHistory`).
2. Prod E2E passes the Autopilot + DSE observability test:
   - `apps-web.prod.autopilot.dse-canary-recap-shows-debug-card-and-trace`
   - suite: `packages/effuse-test/src/suites/apps-web.ts`
3. We can open a production receipt and trace from the UI:
   - `GET /api/dse/receipt/:receiptId`
   - `GET /api/dse/blob/:receiptId/:blobId`
4. There is a single “DSE Ops/Inspector” surface (MVP) to view all of this from the browser (admin-gated).

## Scope: What We Improve Overnight

We do two things in parallel, because they exercise different parts of the system.

### A) Long-context observability + RLM-lite (product surface)

- Canary signature: `@openagents/autopilot/canary/RecapThread.v1`
  - Purpose: make RLM-lite visible, debuggable, replayable in chat UX.
  - Operator surface: `/autopilot` → DSE Debug controls (Phase D).
  - This is primarily about **strategy visibility + traces**, not “compiling the best recap”.

### B) A “real” self-improve loop with compile → canary → promote (decision surface)

- Primary improvement target: `@openagents/autopilot/blueprint/SelectTool.v1`
  - Output is small and deterministic (easy to label + score).
  - It affects UX directly (less wrong/missed blueprint updates).
  - It’s a good first signature to prove “ship behavior like software”.

## Data Persistence: Where Everything Lives In Convex (Source Of Truth)

This plan assumes we keep all “what happened” data in Convex so it’s easy to build web pages later.

Tables (see `apps/web/convex/schema.ts`):

- **Runs and streamed UI parts**
  - `threads`, `messages`, `messageParts`, `runs`
- **DSE predict receipts (for every Signature execution)**
  - `receipts` with `kind="dse.predict"` plus metadata:
    - `receiptId`, `signatureId`, `compiled_id`
  - Stored by: `apps/web/convex/dse/receipts.ts`
- **RLM-lite blobs + variable-space state (thread/run scoped)**
  - `dseBlobs` (trace JSON is stored here as text)
  - `dseVarSpace`
  - Backed by: `apps/web/convex/dse/blobs.ts`, `apps/web/convex/dse/varSpace.ts`
- **Datasets**
  - `dseExamples` (global dataset store)
  - Backed by: `apps/web/convex/dse/examples.ts`
- **Compiled artifacts + active pointer**
  - `dseArtifacts`, `dseActiveArtifacts`, `dseActiveArtifactHistory`
  - Backed by: `apps/web/convex/dse/artifacts.ts`, `apps/web/convex/dse/active.ts`
- **Compile reports**
  - `dseCompileReports`
  - Backed by: `apps/web/convex/dse/compileReports.ts`
- **Canary rollouts**
  - `dseCanaries`, `dseCanaryHistory`
  - Backed by: `apps/web/convex/dse/canary.ts`
  - Auto-stop counts are incremented from predict receipts:
    - `apps/web/convex/dse/receipts.ts` (MVP: count-based auto-stop on high error rate)

Debug read endpoints (Worker → Convex → browser):

- Receipt JSON: `GET /api/dse/receipt/:receiptId` (`apps/web/src/effuse-host/dseAdmin.ts`)
- Blob text (trace JSON): `GET /api/dse/blob/:receiptId/:blobId` (`apps/web/src/effuse-host/dseAdmin.ts`)

## Preconditions (Prod + Dev)

### A) Production E2E prerequisites

From `docs/autopilot/PROD_E2E_TESTING.md`:

- Worker secrets set:
  - `OA_E2E_BYPASS_SECRET`
  - `OA_E2E_JWT_PRIVATE_JWK`
- Run prod E2E with:

```bash
cd apps/web
EFFUSE_TEST_E2E_BYPASS_SECRET="..." \
  npm run test:e2e -- --base-url https://openagents.com --tag prod --grep "apps-web\\.prod\\.autopilot"
```

### B) RLM-lite prerequisites

- Worker must have:
  - `env.AI` bound (Cloudflare AI binding),
  - `OPENROUTER_API_KEY` configured (RLM-lite gating uses OpenRouter primary).
- RLM-lite UI/debug depends on being authenticated in prod (prelaunch gating).

### C) Baseline verification prerequisites

We do not start an overnight “improve loop” run with failing tests.

Minimum local checks:

```bash
cd packages/dse && bun test && bun run typecheck
cd apps/web && npm run lint && npm test
```

## Overnight Timeline (Concrete Steps)

This is written as an “execute it” plan. If some steps are already implemented, we verify and skip.

### 0) Start: Capture the baseline (30 minutes)

1. Verify local tests (see commands above).
2. Verify prod E2E smoke:
   - Run the prod suite (or at least the DSE recap visibility test).
3. In prod `/autopilot`:
   - Run “Run recap (canary)” with `strategy=direct.v1` then `strategy=rlm_lite.v1`.
   - Confirm the `dse.signature` card shows:
     - `signatureId`
     - `strategyId`
     - budgets (limits + usage)
     - and the trace link for `rlm_lite.v1`.
4. Record the baseline artifacts:
   - Keep the `receiptId` for at least one `direct` and one `rlm_lite` run.

### 1) Implement: DSE Ops/Inspector (1.5–2.5 hours)

Goal: one place in the web UI to view all Convex-stored DSE data without devtools.

Deliverables:

- A page/route (admin-gated) that can:
  - list known signatures (from `apps/autopilot-worker/src/dseCatalog.ts`)
  - show active pointer (`dseActiveArtifacts`) + history (`dseActiveArtifactHistory`)
  - show canary status + history (`dseCanaries`, `dseCanaryHistory`)
  - list compile reports (`dseCompileReports`) + open the JSON
  - list examples (`dseExamples`) + open raw `inputJson`/`expectedJson`
  - list recent receipts for a signature (via `receipts` index by `signatureId`) and open:
    - receipt JSON (`/api/dse/receipt/:receiptId`)
    - trace blob (`/api/dse/blob/:receiptId/:blobId`)

Notes:

- The read path should use Convex queries already implemented in `apps/web/convex/dse/*`.
- Keep payloads bounded in the UI; for large JSON show a “raw” toggle.

Verification:

- Add a Worker test or UI contract test that asserts the page renders deterministically for a fixture dataset.
- Ensure `apps/web` lint + tests stay green.

### 2) Implement: Make compile non-trivial for SelectTool (1–2 hours)

Right now `/api/dse/compile` runs an optimizer over an empty search space, which is useful for plumbing but not for improvement.

Goal: overnight, make `@openagents/autopilot/blueprint/SelectTool.v1` actually improvable.

Concrete changes:

- Add a search space for this signature:
  - instruction variants (2–6 candidates)
  - optional few-shot subset selection (if the signature has enough examples to choose from)
  - optional decode mode choice (`strict_json` vs `jsonish`) if needed for robustness
- Keep the reward deterministic at first:
  - `exact_json_match.v1` is acceptable for SelectTool because output is discrete.

Where:

- The compile job spec is currently hardcoded in:
  - `apps/web/src/effuse-host/dseCompile.ts`
  - and gating/promotion checks also reconstruct job spec in:
  - `apps/web/src/effuse-host/dseAdmin.ts`

Rule:

- The job spec function MUST be shared so compile and promote/canary gates use the same definition.
  - Either move it into a shared module or import from one place.

Verification:

- Add/extend a Worker test to prove:
  - compile produces multiple candidates (not 1),
  - best artifact differs from defaults when dataset supports it,
  - report is idempotent by (jobHash, datasetHash).

### 3) Build the dataset in Convex (1–2 hours)

Goal: at least ~30 examples for SelectTool with a holdout split.

Dataset construction rules:

- Use explicit `exampleId` strings (stable across runs).
- Ensure at least 10 examples are `split="holdout"` (do not tune on these).
- Tag everything:
  - `tags=["overnight", "selecttool", "v1"]`
- Keep examples minimal and realistic: short user messages, small blueprint hints.

Mechanics:

- Upsert examples via Convex:
  - `api.dse.examples.putExample` (auth required)
- Prefer a small script or admin UI form to add/edit examples; avoid one-off manual DB edits.

Verification:

- Add a Convex test that asserts deterministic ordering + split filtering (already exists; extend if needed).

### 4) Run compile in prod (30–60 minutes)

Goal: create a compile report + artifact in production Convex.

Steps:

1. Ensure prod is deployed with the new compile job spec.
2. Run:
   - `POST /api/dse/compile { signatureId: "@openagents/autopilot/blueprint/SelectTool.v1" }`
3. Confirm Convex now contains:
   - `dseCompileReports` row
   - `dseArtifacts` row for the resulting `compiled_id`

### 5) Canary in prod (60–120 minutes)

Goal: run a small rollout safely and record everything in Convex.

Steps:

1. Start canary:
   - `POST /api/dse/canary/start`
   - Use `rolloutPct=5` initially
   - Use `minSamples=50`, `maxErrorRate=0.2` (MVP defaults; adjust based on observed stability)
2. Let normal traffic exercise the signature.
3. Monitor:
   - `dseCanaries.okCount/errorCount` increments from predict receipts.
   - `dseCanaryHistory` shows start/update/auto_stop.

Stop conditions:

- If auto-stop triggers: it will delete the canary config and record `action="auto_stop"`.
- If we see regressions in behavior (qualitative), stop manually with:
  - `POST /api/dse/canary/stop`

### 6) Promote (optional, only if canary is clean) (30 minutes)

If the canary behaved and the compile report beat baseline holdout:

1. Promote:
   - `POST /api/dse/promote` with the candidate `compiled_id`
2. Confirm:
   - `dseActiveArtifacts` updated
   - `dseActiveArtifactHistory` recorded

Rollback is pointer-only:

- Use `api.dse.active.rollbackActive` (Convex) or add a small admin button in the inspector page.

### 7) Prod Verification (E2E + Logs) (30–60 minutes)

Run prod E2E suite:

```bash
cd apps/web
EFFUSE_TEST_E2E_BYPASS_SECRET="..." \
  npm run test:e2e -- --base-url https://openagents.com --tag prod --grep "apps-web\\.prod\\.autopilot"
```

If anything fails:

- Correlate by request id first:
  - `x-oa-request-id` and `oa_req=<id>`
  - See: `AGENTS.md` “Telemetry + Debugging (apps/web)”

## What This Unlocks (After Overnight)

Once this plan is executed, Autopilot operators can:

- See *which* strategy ran (direct vs RLM-lite) and open the trace.
- Export trace-derived examples into a dataset (for RLM workloads).
- Compile and promote/canary Signature behavior with:
  - full provenance in Convex,
  - deterministic job hashes,
  - and rollback-ready history.
- Build product pages that show:
  - “what improved last night”
  - backed by real Convex rows (not screenshots).

## Follow-ups (Not Required Overnight, But Next)

1. Add a judge-based reward for non-discrete outputs (recaps/summaries), with pinned judge artifacts.
2. Add a lightweight “labeling UI” (approve/correct expected outputs) so datasets can be built quickly without code edits.
3. Add an “overnight run summary” Convex row (one per session) that links:
   - commit sha, time window, signature ids touched,
   - compile report ids, promoted compiled ids,
   - prod E2E pass/fail summary.
4. Expand compile search spaces (Phase G):
   - chunking knobs, role selection, budget profiles (see `packages/dse/docs/EFFECT_ONLY_DSE_RLM_GEPA_MIPRO_DESIGN.md`).

