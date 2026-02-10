# Unified Roadmap: Context Rot -> RLM -> Trace Mining -> Distilled Agents

- **Status:** Proposed (execution roadmap)
- **Last updated:** 2026-02-10
- **Conflict rules:**
  - If sequencing conflicts with the global roadmap: `docs/ROADMAP.md` wins.
  - If terminology conflicts: `docs/GLOSSARY.md` wins.
  - If implementation details conflict: code wins.

This document is the single end-to-end roadmap for implementing:

- context-rot-aware execution (avoid silent quality collapse on long contexts),
- RLM/FRLM long-context strategies (two-bucket context, budgets, traces),
- and the “agent discovery” loop (trace mining -> distillation -> compile -> ship).

It intentionally consolidates the intent already spread across:

- `docs/autopilot/context-failures.md`
- `docs/autopilot/rlm-synergies.md`
- `docs/autopilot/rlm-trace-mining.md`
- `docs/autopilot/AUTOPILOT_OPTIMIZATION_PLAN.md` (Phase 2.5)
- `docs/autopilot/SELF_IMPROVE_PLAN.md` (Stage 4.5)
- `docs/autopilot/dse.md`
- `packages/dse/docs/EFFECT_ONLY_DSE_RLM_GEPA_MIPRO_DESIGN.md`
- `crates/rlm/docs/METHODS.md` (symbolic recursion)

---

## 0) Constraints (non-negotiable for MVP)

From `docs/autopilot/spec.md` / `docs/autopilot/anon-chat-execution-plane.md`:

- **Convex-first** canonical state for MVP (threads/messages/messageParts + DSE registries).
- **Cloudflare Worker** hosts execution; **no containers**; small built-in tool surface.
- **Everything must be auditable**: schema validation, budgets, receipts, replayable traces.

---

## 1) What we are building (high-level)

### 1.1 Context failure posture

We explicitly handle three failures:

- **Context rot** (quality collapse past soft limits): primary RLM target.
- **Context poisoning** (untrusted/incorrect inputs): provenance/trust + verification, not “more tokens”.
- **Context confusion** (scope collisions): hard scoping + handles, not “better prompts”.

Canonical definitions: `docs/GLOSSARY.md` and `docs/autopilot/context-failures.md`.

### 1.2 RLM as an inference strategy

RLM is not a prompt template. It is a strategy for `Predict(signature)`:

- token space stays bounded,
- long context lives in variable/programmatic space (VarSpace + BlobRefs),
- execution proceeds via explicit ops + strict budgets.

Design reference: `docs/autopilot/rlm-synergies.md`, `packages/dse/docs/EFFECT_ONLY_DSE_RLM_GEPA_MIPRO_DESIGN.md`.

### 1.3 Trace mining -> distilled agents

RLM runs are exploratory. The goal is to:

1. run RLM when context pressure is high or novelty is high,
2. mine traces for repeating tactics,
3. distill those tactics into typed signatures/modules/graphs,
4. compile and promote them via DSE so the default path is fast and reliable.

Process reference: `docs/autopilot/rlm-trace-mining.md`.

---

## 2) Phased Implementation Plan

Each phase has “exit criteria” that should be testable.

### Phase A: Instrumentation and Signals (prerequisite)

Objective: we must be able to *detect* context rot risk and *audit* what happened.

Deliverables:

- Define and emit a **context pressure** signal on runs (even if heuristic v1).
  - Inputs: rendered prompt size/tokens (if available), number/size of retrieved snippets, number/size of tool logs, blob preview sizes, etc.
- Ensure receipts/traces carry:
  - scope keys (`threadId`, `runId`, `signatureId`, `compiled_id`)
  - strategy id (`direct.v1` vs `rlm_lite.v1` etc.)
  - budgets (limits + usage)
  - evidence access (BlobRefs / SpanRefs) + preview sizes (what entered token space)
- Decide where this lives in our trace substrate:
  - DSE receipts (`packages/dse/src/runtime/receipt.ts`)
  - Autopilot replay (`ReplayBundle` today; target `REPLAY.jsonl` v1: `crates/dsrs/docs/REPLAY.md`)

Exit criteria:

- Given a production trace, we can answer:
  - “Was this direct or RLM?”
  - “What budgets were hit?”
  - “Which blobs/spans were accessed and surfaced?”
  - “What was the context pressure and why?”

### Phase B: DSE PredictStrategy Abstraction (direct + RLM)

Objective: make RLM a swappable, artifact-pinnable execution strategy for signatures.

Deliverables:

- Implement an explicit `PredictStrategy` interface in `packages/dse/`.
  - `direct.v1`: current direct predict.
  - `rlm_lite.v1`: action DSL / kernel-driven RLM (no arbitrary code).
- Extend runtime budgets to include RLM counters:
  - iterations, sub-LM calls, tool calls (and per-tool timeouts).
- Pin strategy selection via DSE params/artifacts:
  - spec guidance is in `docs/autopilot/dse.md` (`params.strategy`).

Exit criteria:

- A single signature can be executed under both strategies with identical IO contracts and comparable receipts.

### Phase C: RLM-lite Kernel + VarSpace (Effect-only)

Objective: implement the core “two buckets” behavior safely in Workers.

Deliverables:

- Add `VarSpace` service:
  - values are small JSON or BlobRefs
  - scoped per thread/run (MVP: Convex-backed)
- Add `RlmKernel` that executes a **structured JSON action DSL**:
  - `preview`, `search`, `load`, `chunk`, `write_var`
  - `extract_over_chunks` (kernel-driven fanout; symbolic recursion helper)
  - `sub_lm` (role-based LLM call)
  - optional `tool_call` (behind existing tool policy allowlist)
  - `final` (schema decoded)
- Enforce strict budgets at the kernel boundary.
- Emit per-iteration trace events (deterministic, bounded).

Design constraints:

- Do not require the controller LM to emit O(N) subcalls for N chunks.
- Prefer **symbolic recursion** patterns (kernel/code drives traversal and uses the LM for extraction/synthesis).
  - Reference: `crates/rlm/docs/METHODS.md`.

Exit criteria:

- Long-context workloads can run with bounded token space and auditable per-iteration traces.

### Phase D: Autopilot Integration (routing + UI)

Objective: make RLM usable in the Autopilot product, not just as a library feature.

Deliverables:

- Add an RLM trigger rule/pipeline:
  - trigger on high context pressure, “thrash without new evidence”, or explicit “long-context” task types.
  - gate on model capability (avoid sending weak controller models into RLM loops).
- Persist RLM state appropriately:
  - VarSpace + blobs in Convex (MVP).
- UI: RLM activity must be visible:
  - show strategy id, budgets, iterations, and evidence handles in chat cards (aligned with `docs/autopilot/SELF_IMPROVE_PLAN.md` surfaces).

Exit criteria:

- Users/operators can see when RLM triggered and why, and can debug failures by trace.

### Phase E: Long-Context Datasets + Evaluation

Objective: measure whether RLM is worth it and when to prefer it.

Deliverables:

- Create 1-2 canonical long-context datasets:
  - log Q&A with evidence requirements,
  - repo “needle in haystack” / evidence sourcing tasks.
- Define metrics that balance quality and cost/latency:
  - exact match where possible,
  - evidence correctness (SpanRef/BlobRef citations),
  - and runtime cost (LM calls, tool calls, duration).
- Run eval comparing:
  - direct vs RLM-lite,
  - and (later) distilled pipelines vs RLM.

Exit criteria:

- We have a measurable policy: “When context pressure > X, RLM improves success rate or reduces thrash.”

### Phase F: Trace Mining -> Distillation -> Compile

Objective: convert slow exploratory RLM traces into fast, compiled behavior.

Deliverables:

- A trace export pipeline that can produce candidate labeled examples for `dseExamples`.
- A repeatable trace-review workflow (documented in `docs/autopilot/rlm-trace-mining.md`).
- At least one distilled “tactic” implemented as:
  - signature(s) + module/graph,
  - evaluated on holdout,
  - and promotable/rollbackable via the existing DSE loop (`docs/autopilot/SELF_IMPROVE_PLAN.md`).

Exit criteria:

- For at least one long-context workload:
  - default path uses a distilled pipeline,
  - RLM remains a fallback for novelty/high uncertainty.

### Phase G: Compile Knobs for RLM and Distilled Pipelines

Objective: optimize the pieces that matter (don’t hand-tweak prompts).

Deliverables:

- Add compiler-visible search spaces for:
  - controller instruction blocks,
  - chunking policy knobs,
  - role selection (`main` vs `sub`),
  - budget profiles.
- Add compilation loops that can propose/refine these knobs using failure summaries:
  - see `packages/dse/docs/EFFECT_ONLY_DSE_RLM_GEPA_MIPRO_DESIGN.md`.

Exit criteria:

- RLM strategy configs and distilled pipeline params are produced as compiled artifacts with measurable improvements.

### Phase H: Poisoning/Confusion Hardening (ongoing)

Objective: RLM does not solve poisoning/confusion; we must harden regardless.

Deliverables:

- Provenance-first evidence: SpanRefs/paths/line ranges whenever possible.
- Trust labeling + isolation rules:
  - keep untrusted content in variable space; only surface bounded excerpts with provenance.
- Verification posture for objective claims (tests/builds/grep over “LLM says so”).

Reference: `docs/autopilot/context-failures.md`.

Exit criteria:

- Long-context runs remain auditable and do not silently act on untrusted content.

---

## 3) Verification and Deployment (minimum)

Docs are not done until the harness is green for the touched surfaces.

- Workspace:
  - `cargo test`
- Web (apps/web):
  - `cd apps/web && npm test && npm run lint`
- DSE:
  - `cd packages/dse && bun test && bun run typecheck`

Production smoke guidance:

- `docs/autopilot/PROD_E2E_TESTING.md`

---

## Implementation Log

### 2026-02-10: Phase A (Instrumentation and Signals) — implemented in DSE

- Added prompt render observability:
  - prompt render stats (`PromptRenderStatsV1`) emitted by the renderer, including BlobRef evidence access + preview sizes
  - implementation: `packages/dse/src/runtime/render.ts`
- Added a heuristic v1 context pressure signal derived from rendered prompt chars:
  - implementation: `packages/dse/src/runtime/contextPressure.ts`
- Extended DSE predict receipts to include:
  - `runId` (defaults to `receiptId` for now)
  - `strategyId` (defaults to `direct.v1`)
  - `promptRenderStats` + `contextPressure`
  - implementation: `packages/dse/src/runtime/receipt.ts`, `packages/dse/src/runtime/predict.ts`
- Verified (TypeScript):
  - `cd packages/dse && bun test && bun run typecheck`

### 2026-02-10: Phase F (Trace mining -> distillation -> compile) — implemented in DSE + `apps/web`

- Made RLM-lite traces exportable into candidate labeled examples:
  - trace blob format now includes `signatureId` + `receiptId`, and emits an `Input` event (encoded input JSON)
  - implementation: `packages/dse/src/runtime/predict.ts`
- Added a trace export helper that converts (predict receipt + RLM trace blob) into a `dseExamples`-shaped candidate:
  - extracts `inputJson` from the trace `Input` event and `expectedJson` from the trace `Final.output`
  - implementation: `packages/dse/src/traceMining/exportExamples.ts`, `packages/dse/src/traceMining/rlmTrace.ts`
  - test: `packages/dse/test/traceExport.test.ts`
- Added a Convex-first trace export pipeline for operators:
  - Convex query to fetch a DSE predict receipt by `receiptId`: `apps/web/convex/dse/receipts.ts`
  - Worker admin endpoint `POST /api/dse/trace/export` writes/upserts the candidate into `dseExamples`: `apps/web/src/effuse-host/dseAdmin.ts`
  - Worker test: `apps/web/tests/worker/dse-trace-export.test.ts`
- Implemented and evaluated one distilled “tactic” as a pinned predict strategy:
  - new strategy `distilled.search_line_extract.v1`: deterministic search+parse fast path (0 LM calls) with RLM-lite fallback for novelty/high uncertainty
  - implemented in `Predict(signature)` dispatch: `packages/dse/src/runtime/predict.ts`
  - holdout-enabled dummy dataset splits (train vs holdout): `packages/dse/src/eval/longContextBench.ts`
  - test comparing direct vs distilled vs RLM on holdout: `packages/dse/test/distilledLongContextQa.test.ts`
- Documented the trace-review and export workflow:
  - `docs/autopilot/rlm-trace-mining.md`
- Verified (TypeScript):
  - `cd packages/dse && bun test && bun run typecheck`
  - `cd apps/web && npx convex codegen`
  - `cd apps/web && npm test && npm run lint`

### 2026-02-10: Phase B (PredictStrategy abstraction + RLM counters) — implemented in DSE

- Added `params.strategy` to DSE params so inference strategy is pinned in compiled artifacts:
  - `packages/dse/src/params.ts`
- Implemented strategy selection in `Predict(signature)`:
  - `direct.v1` runs the existing single-call predict (+ repair)
  - `rlm_lite.v1` is recognized but currently fails fast with a typed `PredictStrategyError` (Phase C will provide the RLM kernel)
  - `packages/dse/src/runtime/predict.ts`
- Stabilized TypeScript Effect inference for strategy dispatch:
  - `makePredict` now has an explicit return type and dispatch returns a single `Effect<O, PredictError, PredictEnv>` shape
  - prevents env/error types from degrading to `unknown` under `exactOptionalPropertyTypes`
  - `packages/dse/src/runtime/predict.ts`
- Extended execution budgets for upcoming RLM behavior:
  - new budget limits: `maxToolCalls`, `maxRlmIterations`, `maxSubLmCalls`
  - new usage counters + handle methods: `onToolCall`, `onRlmIteration`, `onSubLmCall`
  - `packages/dse/src/runtime/budget.ts`, `packages/dse/src/params.ts`
- Updated predict receipt schema to carry the expanded budget usage counters:
  - `packages/dse/src/runtime/receipt.ts`
- Verified (TypeScript):
  - `cd packages/dse && bun test && bun run typecheck`

### 2026-02-10: Phase C (RLM-lite Kernel + VarSpace) — implemented in DSE

- Added `VarSpace` service (small JSON values + BlobRefs, bounded in-memory implementation):
  - `packages/dse/src/runtime/varSpace.ts`
- Implemented the RLM-lite kernel with a structured JSON action DSL:
  - actions: `preview`, `search`, `load`, `chunk`, `write_var`, `extract_over_chunks`, `sub_lm`, optional `tool_call`, `final`
  - budgets enforced at the kernel boundary (`onRlmIteration`, `onSubLmCall`, `onToolCall`, `onLmCall`, `onOutputChars`)
  - `packages/dse/src/runtime/rlmKernel.ts`
- Implemented `rlm_lite.v1` strategy execution in `Predict(signature)`:
  - controller loop emits JSON actions, kernel executes them, `Final` is schema-decoded into signature output
  - requires pinned limits: `budgets.maxRlmIterations` and `budgets.maxSubLmCalls` (fail closed if missing)
  - `packages/dse/src/runtime/predict.ts`
- Ensured long-blob context stays in programmatic space for RLM controller:
  - renderer supports `blobContextMode: "metadata_only"` so blob context entries are not inlined into token space
  - `packages/dse/src/runtime/render.ts`
- Added per-iteration trace events:
  - RLM trace is serialized deterministically to a blob (`openagents.dse.rlm_trace`) and referenced from the predict receipt as `rlmTrace`
  - `packages/dse/src/runtime/predict.ts`, `packages/dse/src/runtime/receipt.ts`
- Added optional `ToolExecutor` service contract for `tool_call` actions (behind allowlist):
  - `packages/dse/src/runtime/toolExecutor.ts`
- Added an end-to-end test for `rlm_lite.v1`:
  - `packages/dse/test/rlmLite.test.ts`
- Exported the new runtime modules:
  - `packages/dse/src/index.ts`
- Verified (TypeScript):
  - `cd packages/dse && bun test && bun run typecheck`

### 2026-02-10: Phase D (Autopilot integration: routing + UI + Convex persistence) — implemented in `apps/web`

- Added Convex-backed persistence for RLM state (scoped by `threadId` + `runId`):
  - new tables: `dseBlobs`, `dseVarSpace`
  - schema: `apps/web/convex/schema.ts`
  - functions:
    - `apps/web/convex/dse/blobs.ts` (`putText`, `getText`)
    - `apps/web/convex/dse/varSpace.ts` (`getVar`, `putJson`, `putBlob`, `del`, `list`)
- Worker-side DSE environment now provides Convex-backed `BlobStore` + `VarSpace` for Autopilot runs:
  - new layers: `layerDseBlobStoreFromConvex`, `layerDseVarSpaceFromConvex`
  - updated: `layerDsePredictEnvForAutopilotRun`
  - implementation: `apps/web/src/effuse-host/dse.ts`
- Added an RLM-lite summarization signature (strategy pinned in params):
  - signature id: `@openagents/autopilot/rlm/SummarizeThread.v1`
  - defaults: `strategy.id = rlm_lite.v1` with strict RLM budgets (`maxRlmIterations`, `maxSubLmCalls`)
  - implementation: `apps/autopilot-worker/src/dseCatalog.ts`
- Integrated a gated RLM trigger into Autopilot chat:
  - trigger: explicit `/rlm` / recap phrasing OR high “older history” pressure
  - gate: requires `OPENROUTER_API_KEY` (avoid weak controller models in RLM loops)
  - behavior: pre-summarize older messages via DSE + RLM-lite, inject summary into the main system prompt, and emit an auditable `dse.signature` card with `strategyId`, budgets, and `rlmTrace`
  - implementation: `apps/web/src/effuse-host/autopilot.ts`
- UI: surfaced RLM/DSE observability fields in chat cards:
  - extended `dse.signature` part shape: `apps/web/src/effect/chatProtocol.ts`
  - mapping + rendering: `apps/web/src/effuse-app/controllers/autopilotChatParts.ts`, `apps/web/src/effuse-pages/autopilot.ts`
  - ensured the UI rerender key considers strategy/trace: `apps/web/src/effuse-app/controllers/autopilotController.ts`
- Fixed Autopilot Worker DSE env typing by providing `VarSpace`:
  - `apps/autopilot-worker/src/dseServices.ts`
- Verified (TypeScript):
  - `cd apps/web && npx convex codegen`
  - `cd packages/dse && bun test && bun run typecheck`
  - `cd apps/autopilot-worker && npm run typecheck`
  - `cd apps/web && npm test && npm run lint`

### 2026-02-10: Phase E (Long-context datasets + evaluation) — implemented in DSE

- Added canonical long-context bench signature + datasets (dummy but meaningful):
  - `@openagents/autopilot/eval/LongContextQa.v1` (answer + BlobRef evidence quote)
  - datasets:
    - `autopilot.long_context.log_qa.v1` (needle-in-preview vs needle-beyond-preview)
    - `autopilot.long_context.repo_needle.v1` (repo snapshot needle extraction)
  - implementation: `packages/dse/src/eval/longContextBench.ts`
- Extended prompt rendering so BlobRefs embedded in *signature input JSON* are rendered and counted:
  - adds an `Input blobs:` section that behaves like context blob rendering (`inline_preview` vs `metadata_only`)
  - ensures context pressure can be measured for “input-provided” blobs, not just prompt context entries
  - implementation: `packages/dse/src/runtime/render.ts`
- Extended eval + reward plumbing to score evidence correctness and cost:
  - eval captures per-example predict receipts and surfaces `predictMeta` (strategy, timing, context pressure, budget usage)
  - reward signals:
    - `evidence_quote_in_blob.v1` checks that `evidence.quote` is an exact substring of the cited blob text
    - `predict_cost.v1` adds a normalized penalty for duration / LM calls / tool calls
  - implementation: `packages/dse/src/eval/evaluate.ts`, `packages/dse/src/eval/reward.ts`
- Added an end-to-end Phase E test that demonstrates the policy we want to measure:
  - direct fails when the needle is beyond preview; RLM-lite succeeds via `Search` + `Final`
  - implementation: `packages/dse/test/longContextBench.test.ts`
- Verified (TypeScript):
  - `cd packages/dse && bun test && bun run typecheck`
