# Autopilot Self-Improve Runbook (How I Will Improve DSE/RLM Safely)

- **Status:** Draft (operational runbook; intended to be executed by Codex/operators)
- **Last updated:** 2026-02-10
- **Legacy scope:** This runbook targets the former `apps/web` stack; treat command paths and endpoint references as historical unless mapped to current Laravel/runtime surfaces.
- **Plan/spec refs:** `docs/autopilot/runbooks/SELF_IMPROVE_PLAN.md`, `docs/autopilot/dse/dse.md`
- **How-to:** `docs/autopilot/runbooks/DSE_PLAYBOOK.md`
- **End-to-end roadmap:** `docs/autopilot/dse/RLM_UNIFIED_ROADMAP.md`
- **If this doc conflicts with code behavior:** code wins

This doc is the concrete process I will follow (and keep tightening) when improving DSE + RLM-lite in Autopilot.
The goal is boring, repeatable shipping: **measurable improvements, no surprises, easy rollback**.

## 0) What We Treat As “Real”

We trust improvements only when they are anchored by:

- **Receipts + traces** (replayable evidence of what ran, with budgets).
- **Datasets + metrics** (so we can compare before/after).
- **Tests** (unit/worker/UI/E2E) that make regressions obvious.

If any of these are missing, the work is not “improved”; it is “changed”.

## 1) Testing Surfaces (The Verification Harness)

This repo has multiple complementary test layers. The self-improve loop uses all of them:

- **DSE library tests (runtime/compiler contracts):**
  - `cd packages/dse && bun test && bun run typecheck`
- **Web worker + Khala MVP tests (contract + integration):**
  - `cd apps/web && npm test`
- **Web typecheck/lint (must be green for ship):**
  - `cd apps/web && npm run lint`
- **E2E browser tests (Effect-native runner):**
  - `cd apps/web && npm run test:e2e`
  - Against prod requires E2E bypass secret, see: `docs/autopilot/testing/PROD_E2E_TESTING.md`
- **Visual regression (storybook canvas screenshots):**
  - `cd apps/web && npm run test:visual`
  - Storybook docs: `docs/STORYBOOK.md`
- **Wire transcript fixtures (deterministic chat rendering):**
  - Contract: `docs/autopilot/testing/STREAM_TESTING.md`
  - Fixtures: `docs/autopilot/testing/fixtures/*.stream.v1.jsonl`

Rule: I don’t claim a behavior improvement unless the relevant layer is covered (at minimum: unit/worker + UI visibility).

## 2) The Self-Improve Loop (Runtime -> Dataset -> Compile -> Canary -> Promote)

This is the operational loop DSE is designed for:

1. **Observe**
   - Run a Signature and capture:
     - `signatureId`, `strategyId`, `compiled_id`
     - budgets (limits + usage)
     - receipts + (if RLM) `rlmTrace`
   - In UI, this is the `dse.signature` card (see `docs/autopilot/runbooks/DSE_PLAYBOOK.md`).
     - On `/autopilot`, signature cards are collapsed by default; click to expand to copy `receiptId` / open trace links.

2. **Export / Label**
   - Convert good runs and failures into `dseExamples` rows.
   - Prefer trace/receipt export paths when possible (they preserve the real input/output contracts).
   - Keep a real holdout split. Don’t tune on holdout.
   - Headless scale-up:
     - `apps/web/scripts/dse-trace-mine.ts` mines many receipts and exports/tag examples into Khala.
     - Reference: `docs/autopilot/dse/rlm-trace-mining.md`.

3. **Evaluate**
   - Compare current default (`direct.v1`) against `rlm_lite.v1` (or a candidate artifact) on the same dataset slice.
   - Track both **quality** and **cost**:
     - correctness / evidence correctness
     - LM calls, tool calls, iterations/subcalls, duration

4. **Compile**
   - Run compilation to generate candidate params and select the best deterministically.
   - This yields an immutable artifact id (`compiled_id`) + report.
   - Promotion is explicit (compile does not silently change production).

5. **Canary**
   - Roll out the candidate artifact to a small percent of traffic.
   - Watch receipts for error rate and budget blowups.
   - Stop immediately if it regresses (pointer-only rollback).

6. **Promote**
   - Point the signature to the new `compiled_id` (explicit registry update).
   - Keep history so rollback is always one mutation away.

7. **Lock it in**
   - Add/adjust tests and fixtures so the improvement cannot silently regress.

For the fully programmatic overnight loop (compile -> canary -> promote/rollback) with all logs stored in Khala,
use `apps/web/scripts/dse-overnight.ts` and follow `docs/autopilot/runbooks/OVERNIGHT_SELF_IMPROVEMENT_PLAN.md`.

## 3) How I’ll Apply This To RLM-lite Specifically

RLM-lite exists to prevent “context rot” by keeping long context in **variable space** and only bringing bounded excerpts into **token space**.
Because of that, RLM-lite improvements must be verified along three axes:

- **Budget safety:** iterations/subcalls/tool calls are enforced and visible.
- **Auditability:** traces show what was accessed (BlobRefs/SpanRefs), with bounded previews.
- **Behavior:** output is schema-valid and does not hallucinate evidence.

Concrete posture:

- When I change the RLM kernel/DSL, I add or update:
  - a unit test in DSE (budget/truncation/trace shape),
  - a worker test in `apps/web/tests/worker` (endpoint/receipt/trace behavior),
  - and an E2E assertion that the UI shows the debug card and that the trace is fetchable.

The canonical E2E “it’s real” test is:

- `apps-web.prod.autopilot.dse-canary-recap-shows-debug-card-and-trace`
  - implementation: `packages/effuse-test/src/suites/apps-web.ts`

## 4) “What I Will Do” For A Typical Improvement PR

This is the checklist I will follow when asked to improve Autopilot behavior via DSE/RLM:

1. **Pick the contract surface**
   - Identify the signature (or add one) and keep IO schemas small and stable.

2. **Reproduce and capture evidence**
   - Use the DSE Debug controls in `/autopilot` (or equivalent) to run both strategies.
   - Capture `receiptId` and (if RLM) the trace (expand the signature card to reveal receipt/trace links).

3. **Create or expand a dataset**
   - Add a small, high-signal dataset slice:
     - trace exports into `dseExamples` (preferred),
     - or deterministic fixtures under `docs/autopilot/testing/fixtures/` (for UI contracts).

4. **Change code**
   - Implement the improvement with strict budgets and bounded outputs.
   - Ensure strategy selection remains pinned (`params.strategy.id`) so replay is stable.

5. **Add tests at the right layer**
   - Unit test for the pure contract.
   - Worker test for endpoints/runtime wiring.
   - E2E/visual test only when the user-facing UI surface matters (it usually does).

6. **Run verification**
   - `cd packages/dse && bun test && bun run typecheck`
   - `cd apps/web && npm test && npm run lint`
   - Optionally: `cd apps/web && npm run test:e2e` (local) or prod E2E per `docs/autopilot/testing/PROD_E2E_TESTING.md`

7. **Ship safely**
   - If the change affects runtime behavior, prefer canary + rollback-ready promotion.
   - Write down the “how to debug” path in `docs/autopilot/runbooks/DSE_PLAYBOOK.md` when new fields/controls are added.

## 5) Common Failure Modes (And How This Runbook Prevents Them)

- “It worked once” but regresses:
  - Fix: dataset + tests + pinned artifact id.
- Latency/cost silently explodes:
  - Fix: budget enforcement + receipt counters + canary thresholds.
- RLM hides what it looked at:
  - Fix: trace with spans/BlobRefs, UI trace link, and bounded previews.
- Prompt injection / poisoning:
  - Fix: provenance-first observations and never trusting untrusted text without verification.
  - Reference: `docs/autopilot/reference/context-failures.md`
