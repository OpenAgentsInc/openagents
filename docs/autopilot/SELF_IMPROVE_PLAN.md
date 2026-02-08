# Autopilot Self-Improve Plan (Convex-First MVP)

- **Status:** Proposed (implementation plan)
- **Last updated:** 2026-02-08
- **Scope:** Effect-only (TypeScript + Effect) runtime + compiler loops; Convex-backed persistence for MVP
- **If this doc conflicts with code behavior:** code wins

This doc answers two questions for the current Autopilot MVP:

1. **Can the agent improve in a loop today?**
2. **What’s missing to make that loop real in production (Convex-first)?**

It is intentionally concrete: code pointers, required data models, and a step-by-step roadmap with testable increments.

## Convex-First MVP Constraint (Non-Negotiable)

For the MVP execution plane:

- **No per-user Durable Objects / DO-SQLite** for chat or “user space.”
- Cloudflare Worker runs inference/compute and writes bounded, chunked state into **Convex**.

References:

- Decision: `docs/autopilot/anon-chat-execution-plane.md`
- DO shims (deprecated 410): `apps/web/src/effuse-host/worker.ts`

`apps/autopilot-worker/` (DO-SQLite) is a reference integration, not the MVP hot path.

## What We Can Do Today (Reality Check)

### DSE library can already “improve in a loop” offline

In `packages/dse/` we have the core mechanics needed for a compile/eval loop:

- Typed signatures + prompt IR + deterministic hashes: `packages/dse/src/signature.ts`, `packages/dse/src/promptIr.ts`, `packages/dse/src/hashes.ts`
- Predict (policy resolution, decode/repair, receipts): `packages/dse/src/runtime/predict.ts`, `packages/dse/src/runtime/receipt.ts`
- Evaluation (datasets, metrics, reward signals, caching): `packages/dse/src/eval/*`
- Compile loop (MVP optimizers): `packages/dse/src/compile/compile.ts`
- Budgets enforced for DirectPredict (time / LM calls / output chars), recorded in receipts:
  - implementation: `packages/dse/src/runtime/budget.ts`
  - wiring: `packages/dse/src/runtime/predict.ts`
  - commit: `42507656f`

This means: given a dataset + metric + search space, we can **select better params** and emit an immutable `DseCompiledArtifactV1` (`packages/dse/src/compiledArtifact.ts`).

### Autopilot MVP does not yet execute through DSE

The live MVP execution plane in `apps/web/` currently runs chat using `@effect/ai` directly:

- Worker inference + chunked streaming to Convex: `apps/web/src/effuse-host/autopilot.ts`
- Canonical state in Convex: `apps/web/convex/autopilot/*` and schema `apps/web/convex/schema.ts`

So: we do **not** yet have the “closed loop” where production runs are:

- executed as DSE `Predict(signature)` calls
- recorded as DSE receipts keyed by `signatureId` + `compiled_id`
- compiled into new artifacts and promoted/rolled back inside Convex-backed registries

## What “Self-Improve in a Loop” Means (In This Repo)

At a minimum, a self-improvement loop has this shape:

1. **Observe:** capture inputs/outputs + metadata (hashes, timing, budgets, tool failures) for a specific contract surface.
2. **Label:** get an `expected` output or a judge score (human label, deterministic label, or pinned judge signature).
3. **Evaluate:** run the current policy on a dataset slice and score it.
4. **Compile:** generate candidate policy variants, evaluate them, pick the best deterministically.
5. **Promote:** set the active artifact pointer explicitly (no hidden runtime mutation).
6. **Monitor:** track rolling quality/cost; keep receipts for replay/debug.
7. **Rollback:** pointer-only rollback using history.

In DSE terms this is: `Signature` + `Dataset` + `RewardBundle` + `CompileJob` + `ArtifactRegistry`.

## What’s Missing For A Real Loop In The Convex-First MVP

This is the delta between “library can compile” and “Autopilot self-improves in prod.”

### 1) A production “optimization target” expressed as a Signature

We need at least one stable, typed decision surface in the MVP hot path.

Good first targets (small IO, easy labeling):

- Blueprint tool routing: `@openagents/autopilot/blueprint/SelectTool.v1` (defined in `apps/autopilot-worker/src/dseCatalog.ts`)
- Bootstrap extractors (handle/name/vibe): `apps/autopilot-worker/src/dseCatalog.ts`

These should be executed from `apps/web` (Worker) using `@openagentsinc/dse` `Predict.make(...)`, not only exist as contract exports.

### 2) Convex-backed Policy Registry + Artifact Store (MVP)

The DSE runtime expects a `PolicyRegistryService` (get active compiled id, load artifact).

MVP needs Convex tables + functions roughly equivalent to the DO-SQLite reference integration:

- `dseArtifacts`: `{ signatureId, compiled_id, json, createdAtMs, hashes... }`
- `dseActiveArtifacts`: `{ signatureId, compiled_id, updatedAtMs }`
- `dseActiveArtifactHistory`: append-only history for rollback

The DO-SQLite shape exists as reference only in `apps/autopilot-worker/src/dseServices.ts`.

### 3) A receipt stream that is usable as training data

For DSE to improve, we need:

- stable identifiers (`signatureId`, `compiled_id`, `receiptId` / `runId`)
- a way to derive examples from receipts and/or tool outcomes

DSE predict receipts already include hashes + budgets and can include errors:

- receipt format: `packages/dse/src/runtime/receipt.ts`
- predictor writes receipts on both Ok and Error now: `packages/dse/src/runtime/predict.ts`

MVP needs a Convex persistence story for DSE receipts (separate from chat message parts):

- simplest: store DSE receipts as `receipts.json` with a new `kind` discriminator (or a new `dseReceipts` table)
- store large payloads via `BlobRef` (R2 or equivalent) and keep Convex rows bounded

### 4) A labeling mechanism (otherwise “compile” has no target)

Options, from easiest to hardest:

- deterministic labels from tool outcomes (e.g. router signature: “tool called” vs “none”)
- human labeling UI (mark “correct tool” / “wrong tool”)
- pinned judge signatures (LLM judge) using DSE judge metrics (`packages/dse/src/eval/metric.ts`)

Without labels, you can’t justify promotion.

### 5) A compile runner that can execute safely (budgets + gating)

We need a place to run compile jobs that can call the LM, evaluate candidates, and write artifacts:

- Worker-triggered compile job that writes into Convex (preferred for keeping secrets in Worker)
- or Convex action that calls an external inference API (acceptable if secrets posture is correct)
- or offline/dev-only compile (good for the very first “prove it works” iteration)

Regardless of runner, compile must be:

- bounded (ExecutionBudget + explicit iteration caps)
- auditable (compile report + provenance + artifact hashes)
- safe to deploy (manual promote first; canary later)

### 6) Promotion/rollback controls (explicit, pointer-only)

We need explicit APIs to:

- list artifacts per signature
- promote: set active pointer
- rollback: set pointer to prior value

All of this should be Convex-backed for MVP.

## Minimal Roadmap (Testable Increments)

The goal is a smallest end-to-end vertical slice: **one signature improves on a dataset and can be promoted/rolled back**.

### Stage 0: Baseline Green (already)

- DSE unit tests: `cd packages/dse && bun test && bun run typecheck`
- Web worker + Convex tests: `cd apps/web && npm test && npm run lint`

### Stage 1: Budgeted, receiptful Predict (done)

- DirectPredict budgets + receipt snapshots are implemented (commit `42507656f`).

### Stage 2: Convex-backed DSE stores (artifacts + active pointer + receipts)

Add Convex tables + functions for:

- artifact store (`putArtifact`, `getArtifact`, `listArtifacts`)
- active pointer (`getActive`, `setActive`, `clearActive`) + history
- receipt append (`recordPredictReceipt`)

Testable outcomes:

- worker test proves “store → promote → rollback” works in Convex (analogous to `apps/autopilot-worker/tests/index.test.ts`, but in `apps/web/tests/worker/`)

### Stage 3: Run one DSE signature in the MVP hot path

Pick one signature (recommended: `@openagents/autopilot/blueprint/SelectTool.v1`) and execute it inside the `apps/web` Worker:

- provide `LmClientService` backed by the existing Workers AI binding
- provide `PolicyRegistryService` backed by Convex
- provide `ReceiptRecorderService` backed by Convex
- keep budgets on (`ExecutionBudgetService`)

Testable outcomes:

- worker test calls an endpoint that runs the signature and asserts:
  - output schema ok
  - receipt recorded with `signatureId` + resolved `compiled_id`

### Stage 4: Create a labeled dataset in Convex

Add `dseExamples` in Convex:

- `{ signatureId, exampleId, inputJson, expectedJson, split?, tags?, createdAtMs, source }`

Seed examples via:

- a small static dataset for the first signature (router examples are easy)
- optional “export receipts → label → import examples” dev workflow

Testable outcomes:

- convex query returns dataset deterministically sorted by `exampleId`

### Stage 5: Compile job runner (manual promotion)

Implement an admin-only compile endpoint:

- reads dataset
- runs `Compile.compile(...)`
- writes artifact + compile report
- does **not** auto-promote; promotion is manual

Testable outcomes:

- compile produces an artifact whose `compiled_id` matches `paramsHash`
- compile report persists and is reproducible (same inputs => same `jobHash`)

### Stage 6: Promotion gating + canary (optional, post-MVP)

Add:

- holdout split enforcement
- minimum improvement thresholds
- canary rollout (requires runtime selection beyond “single active pointer”)
- automatic rollback when regression detected

## Suggested “First Signature” For Self-Improve

`@openagents/autopilot/blueprint/SelectTool.v1` is ideal because:

- output is small and schema-validated (easy to score)
- labels are cheap (tool-called vs none, plus which tool)
- it directly affects UX (less spurious tool calls; correct updates)
- it’s already defined and has a few-shot pool (`apps/autopilot-worker/src/dseCatalog.ts`)

## Related Docs

- DSE spec (contract + compile posture): `docs/autopilot/dse.md`
- Autopilot optimization phases: `docs/autopilot/AUTOPILOT_OPTIMIZATION_PLAN.md`
- Convex-first execution plane: `docs/autopilot/anon-chat-execution-plane.md`
- RLM/GEPA/MIPRO plan (Effect-only, Convex-first aligned): `packages/dse/docs/EFFECT_ONLY_DSE_RLM_GEPA_MIPRO_DESIGN.md`
- DSE vs Rust reference optimizers review: `packages/dse/docs/RLM_GEPA_MIPRO_DSE_REVIEW_AND_ROADMAP.md`

