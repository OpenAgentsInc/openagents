# Unified Roadmap: Context Rot -> RLM -> Trace Mining -> Distilled Agents

- **Status:** Proposed (execution roadmap)
- **Last updated:** 2026-02-09
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

