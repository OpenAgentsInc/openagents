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

Concretely, the Worker builds a plain chat prompt from recent messages and calls:

- `AiLanguageModel.streamText(...)` with `toolChoice: "none"` (tools disabled)
- flushes chunked `@effect/ai` stream parts into Convex `messageParts`

UI rendering then reconstructs the assistant message by replaying `messageParts`:

- decode/accumulate stream parts: `apps/web/src/effect/chat.ts`
- render text + tool cards: `apps/web/src/effuse-app/controllers/autopilotController.ts`, `apps/web/src/effuse-pages/autopilot.ts`

So: we do **not** yet have the “closed loop” where production runs are:

- executed as DSE `Predict(signature)` calls
- recorded as DSE receipts keyed by `signatureId` + `compiled_id`
- compiled into new artifacts and promoted/rolled back inside Convex-backed registries

## Frontend Integration Requirement (DSE Must Be Visible)

For Autopilot to be debuggable and to support “self-improve” workflows, DSE can’t be an invisible internal layer.

Requirement:

- **Whenever any DSE action runs, it must surface in the chat UI** as a typed component/card (signature call, tool call, compile run, promotion, budget stop, etc.).

This mirrors the existing posture where tool calls/results are rendered as chat cards (see `packages/effuse/src/toolParts/*`), but extended to DSE-specific actions.

### Proposed approach: DSE events as chat parts

Today the chat “wire” is Convex `messageParts.part` objects (currently `@effect/ai/Response` stream parts).

We should treat the `part` field as a union:

- `@effect/ai` stream parts (text deltas, finish, tool-call/result if enabled)
- **DSE action parts** (custom objects with `type: "dse.*"`), appended into the same `messageParts` stream

This keeps ordering deterministic and makes DSE activity replayable in the same way streaming text is.

### DSE chat-part schema (versioned, MVP)

We should standardize DSE parts as small, versioned JSON objects so both the Worker and UI can evolve safely.

Rules:

- `part.type` MUST start with `dse.` (namespaced).
- `part.v` MUST be a number (start at `1`).
- `part.id` MUST be stable for the lifetime of the action so the UI can update an existing card (same rule as tool cards update by `toolCallId` today).
- Parts MUST stay bounded. Any large payloads should be persisted separately (Convex table row or `BlobRef`) and referenced by id.

Minimal common shape:

```json
{
  "type": "dse.signature",
  "v": 1,
  "id": "dsepart_01HT...",
  "state": "start",
  "tsMs": 1760212345678
}
```

Signature action part (example):

```json
{
  "type": "dse.signature",
  "v": 1,
  "id": "dsepart_01HT...",
  "state": "ok",
  "tsMs": 1760212345890,
  "signatureId": "@openagents/autopilot/blueprint/SelectTool.v1",
  "compiled_id": "c_2f0d...",
  "timing": { "durationMs": 212 },
  "budget": {
    "limits": { "maxTimeMs": 2500, "maxLmCalls": 1, "maxOutputChars": 8000 },
    "usage": { "elapsedMs": 212, "lmCalls": 1, "outputChars": 1432 }
  },
  "receiptId": "rcpt_01HT..."
}
```

Tool action part (example):

```json
{
  "type": "dse.tool",
  "v": 1,
  "id": "dsepart_01HT...",
  "state": "error",
  "tsMs": 1760212346000,
  "toolName": "convex.mutation",
  "toolCallId": "toolcall_01HT...",
  "errorText": "Timeout after 2000ms"
}
```

Compile action part (example):

```json
{
  "type": "dse.compile",
  "v": 1,
  "id": "dsepart_01HT...",
  "state": "ok",
  "tsMs": 1760212349999,
  "signatureId": "@openagents/autopilot/blueprint/SelectTool.v1",
  "jobHash": "job_9e13...",
  "best": { "compiled_id": "c_8a1b...", "reward": 0.71 },
  "candidates": 24,
  "reportId": "compile_report_01HT..."
}
```

Promotion action part (example):

```json
{
  "type": "dse.promote",
  "v": 1,
  "id": "dsepart_01HT...",
  "state": "ok",
  "tsMs": 1760212351000,
  "signatureId": "@openagents/autopilot/blueprint/SelectTool.v1",
  "from": "c_old...",
  "to": "c_new...",
  "reason": "compile job job_9e13... improved reward 0.59 -> 0.71"
}
```

Budget stop part (example):

```json
{
  "type": "dse.budget_exceeded",
  "v": 1,
  "id": "dsepart_01HT...",
  "state": "error",
  "tsMs": 1760212352000,
  "message": "Stopped after exceeding maxLmCalls=1",
  "budget": {
    "limits": { "maxLmCalls": 1 },
    "usage": { "elapsedMs": 430, "lmCalls": 2, "outputChars": 0 }
  }
}
```

Note: `PredictReceiptV1` already has a stable, versioned format (`packages/dse/src/runtime/receipt.ts`). These chat parts should reference receipts by id rather than inlining the full receipt.

### New UI components we need

At minimum, add first-class renderers for:

- **Signature card** (`dse.signature.*`)
  - shows `signatureId`, `compiled_id`, status (running/ok/error), timing, budget snapshot
  - optionally shows input/output previews (bounded; large payloads via `BlobRef`)
  - links to a stored receipt (deep view) when available
- **Tool card** (`dse.tool.*`)
  - reuse existing tool card rendering (`renderToolPart`) when possible
  - always show `toolCallId`, tool name, and bounded input/output/error
- **Compile card** (`dse.compile.*`)
  - shows compile job hash, optimizer id, dataset id/hash, best reward, candidate count
  - links to stored compile report + resulting artifact id
- **Promotion card** (`dse.promote.*` / `dse.rollback.*`)
  - shows signatureId, from/to `compiled_id`, who/what initiated it, and rationale
- **Budget stop card** (`dse.budget_exceeded`)
  - clearly indicates “stopped due to budget” (not a silent failure)

Where this wiring lives:

- Wire decode/accumulation: `apps/web/src/effect/chat.ts` (extend `applyRemoteChunk`)
- Chat part typing: `apps/web/src/effect/chatProtocol.ts` (add typed `ChatPart` variants instead of relying on `{ type: string }`)
- Render mapping: `apps/web/src/effuse-app/controllers/autopilotController.ts` (extend `toRenderableParts`)
- Templates/components: either:
  - add `packages/effuse/src/dseParts/*` (preferred: reusable renderers like `renderSignaturePart`)
  - or render directly in `apps/web/src/effuse-pages/autopilot.ts` if we want to keep DSE UI local to the app

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

### 7) Frontend DSE surfaces (chat components + inspector pages)

The self-improve loop will not be operable without UI:

- Users/operators must be able to see which signatures/tools ran (and which policy `compiled_id` was used).
- When a new artifact is produced/promoted, it must be visible and attributable.

Minimum UI deliverables:

- Inline chat cards for DSE actions (signature/tool/compile/promote/budget stop).
- A minimal “DSE inspector” surface (can be read-only at first):
  - list artifacts per signature
  - show active pointer + history
  - show compile reports
  - show predict receipts (filter by signatureId/runId)

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

### Stage 2.5: Add DSE chat parts + UI components (required)

Add a DSE “action part” protocol and render it in chat.

Implementation sketch:

- Extend the Convex `messageParts.part` union to allow custom DSE parts (no schema change required since it is `v.any()`, but we should standardize a versioned shape).
- Update `apps/web/src/effect/chat.ts` to recognize `type: "dse.*"` parts and convert them into typed `ChatPart`s.
- Update `apps/web/src/effuse-app/controllers/autopilotController.ts` + `apps/web/src/effuse-pages/autopilot.ts` to render:
  - signature cards
  - tool cards (existing)
  - compile/promote/budget cards

Testable outcomes:

- storybook story showing each new DSE card type: `apps/web/src/storybook/stories/autopilot.ts`
- worker test that appends a `dse.signature.ok` part and asserts it renders into a non-empty assistant message model (snapshot/e2e style)

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
  - the chat stream includes a `dse.signature.*` part for the run (visible in UI)

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
