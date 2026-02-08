# Effect-Only Design: DSE + RLM + GEPA + MIPRO

Generated: 2026-02-08

This doc proposes an **Effect-only** (TypeScript + Effect) architecture for:

- DSE runtime (`Predict`, artifacts, policies, receipts, blobs)
- RLM as an inference strategy (two-bucket context + recursion + budgets)
- MIPRO-like and GEPA-like compilation loops (candidate generation + eval + promotion)

Constraints assumed:

- `apps/web` and `apps/autopilot-worker` run in Cloudflare Workers/DOs.
- DSE runtime and compiler loops must be **TypeScript/Effect-native**.
- Rust implementations in `crates/*` are **reference only** (do not depend on them at runtime).

Related docs:

- DSE spec: `docs/autopilot/dse.md`
- DSE roadmap: `docs/autopilot/AUTOPILOT_OPTIMIZATION_PLAN.md`
- Patterns to port: `docs/autopilot/horizons-synergies.md`, `docs/autopilot/microcode-synergies.md`, `docs/autopilot/rlm-synergies.md`

## Goals

- One stable contract surface: `DseSignature<I, O>` and tool contracts.
- Policy is shippable: compilation produces immutable artifacts with stable IDs; runtime pins and records `compiled_id`.
- Everything is auditable: every LM/tool call produces receipts and bounded traces.
- Long-context is handled explicitly: RLM mode avoids stuffing huge blobs into token space.
- Optimizers are explicit: compilation is a job with inputs (dataset/metric/search space/budget) and deterministic outputs (artifacts + reports).

## Non-Goals

- Shipping a Horizons-compatible YAML graph engine in the short term.
- Running arbitrary Python in Workers. RLM starts as RLM-lite (action DSL).
- “Learning in prod” where runtime mutates policies implicitly. Promotion is explicit.

## Architecture Overview

Think in three layers:

1. **Contracts** (pure data): signatures, tool contracts, prompt IR, params, compiled artifacts, datasets, metrics, reward signals.
2. **Runtime** (Effect services): LM client, tool executor, policy registry, blob store, varspace, budgets, receipt recorder, trace recorder.
3. **Compilation** (Effect programs): candidate generation + evaluation + selection + artifact emission + promotion.

`packages/dse/` owns (1), plus default implementations for (2) that are portable (in-memory, noop).
Apps (Workers + DOs) own production implementations of (2) (SQLite-backed registries, blob storage, etc.).

## Effect Services (Runtime)

### Required services

The runtime environment should include these Effect `Context.Tag` services:

- `LmClient`: `complete({ messages, modelId, temperature, ... }) -> { text, usage? }`
- `ToolExecutor`: execute a tool call against DSE tool contracts, with schema validation and receipts
- `PolicyRegistry`: get/set active compiled policy and load compiled artifacts
- `BlobStore`: content-addressed storage for large text blobs (Prompt IR context + RLM varspace)
- `ReceiptRecorder`: append-only receipt store (predict receipts, tool receipts, compile receipts)
- `TraceRecorder`: bounded event stream for replay/debug (per run, per iteration)
- `ExecutionBudget`: centralized counters/limits (time, LM calls, tool calls, output chars, RLM iterations, sub-LM calls)
- `Clock` and `Random`: deterministic testing and seeded sampling in compile/eval

Notes:

- Adapters do formatting/parsing only. Retries, timeouts, budgets, and receipts live in the runtime services.
- In Workers, services must be request-scoped (inject `env`/`ctx` through Layers).

### Budget service

`ExecutionBudget` should be the single guardrail API used by both runtime and compilation.

Proposed interface sketch:

```ts
export type Budget = {
  readonly start: (options: { readonly runId: string; readonly limits: BudgetLimits }) => Effect.Effect<void>
  readonly checkTime: Effect.Effect<void>
  readonly onLmCall: (meta: { readonly modelId?: string; readonly role?: string }) => Effect.Effect<void>
  readonly onToolCall: (meta: { readonly toolName: string }) => Effect.Effect<void>
  readonly onRlmIteration: Effect.Effect<void>
  readonly onSubLmCall: Effect.Effect<void>
  readonly onOutputChars: (n: number) => Effect.Effect<void>
  readonly snapshot: Effect.Effect<BudgetSnapshot>
}
```

Budget limits should be serializable and included in receipts.

## DSE Runtime: Predict

### DirectPredict (today)

DirectPredict is the current behavior in `packages/dse/src/runtime/predict.ts`:

1. Resolve active artifact from `PolicyRegistry` (or defaults).
2. Apply allowlisted params transforms to Prompt IR (instruction text, few-shot ids, tool policy).
3. Render prompt messages deterministically (load blob context, truncate).
4. Call `LmClient.complete`.
5. Decode output via Schema; optionally do bounded repair via a second LM call.
6. Record a predict receipt with stable hashes.

Effect-only improvements needed for parity with the roadmap:

- Enforce timeouts and budgets via `ExecutionBudget` and `SignatureConstraints`.
- Record tool receipts (when tools are used) and link them to the predict receipt/run id.

### PredictStrategy abstraction (needed for RLM)

Add an interface that can be selected per signature or per artifact:

- Strategy: `direct.v1` (DirectPredict)
- Strategy: `rlm_lite.v1` (RlmPredict)

Selection should be pinned inside the compiled artifact `params` (or an explicit `strategy` field) so runtime is deterministic.

## RLM (Effect-only)

RLM is an **inference-time strategy** that must work with existing signatures.

### Core data model

RLM needs “two buckets of context”:

- Token space: small prompt messages sent to the main LM.
- Variable space: durable store of large blobs and derived values.

Represent variable space as a service:

- `VarSpace`: `get(varName)`, `put(varName, valueRef)`, `list()`.
- Values should be references, not copies. Prefer `BlobRef` plus small JSON values.

VarSpace should be per-thread (Autopilot DO), keyed by `{ threadId, runId }` or `{ threadId }` depending on retention needs.

### RLM-lite action DSL (no arbitrary code)

Start with a structured JSON action format emitted by the LM:

- Action: `preview` (blob slice)
- Action: `search` (keyword/regex, returns spans/snippets)
- Action: `chunk` (plan chunk boundaries, returns chunk refs)
- Action: `sub_lm` (calls sub-model with a small prompt and optional blob slices)
- Action: `tool_call` (optional, behind tool policy allowlist)
- Action: `write_var` (store derived values/snippets)
- Action: `final` (produce the signature output as JSON)

The action runner is deterministic and emits a trace event for every action.

### RlmPredict flow

RlmPredict wraps a normal signature but changes how context is handled:

1. Store large inputs into `BlobStore` and bind them into `VarSpace` (for example `input_blob`, `context_blob`).
2. Build a controller prompt containing the signature id and output schema, variable names and tiny previews (not full blobs), and the available actions and budgets.
3. Loop for `maxIterations`: ask main LM for the next action(s) (strict JSON), execute actions via the RLM kernel (BlobStore + VarSpace + ToolExecutor + sub-LM), then append a bounded observation summary back into token space.
4. On `final`, decode to the signature output schema.
5. Emit per-iteration trace events and an overall predict receipt (including strategy id and budget snapshot).

Budgets:

- Every iteration calls `ExecutionBudget.onRlmIteration`.
- Every `sub_lm` calls `ExecutionBudget.onSubLmCall` and uses a separate model role config.
- Tool calls go through `ExecutionBudget.onToolCall`.

### Sub-model roles

Model roles must be explicit, effect-configurable, and artifact-pinnable:

- `main`: controller
- `sub`: recursion helper
- `judge`: evaluation metric
- `repair`: decode repair

In DSE, add `params.modelRoles` (or similar) to avoid overloading a single `modelId`.

## Evaluation (Effect-only)

The current DSE eval harness already matches the “RLM-style reward signals” idea:

- Reward bundle is a set of signals that each produce a score and notes.
- Weighted aggregation produces `reward` in `[0, 1]`.
- Judge metrics are pinned to a judge signature + compiled artifact.

Effect-only additions needed for GEPA/MIPRO-quality compile:

- Standardize a bounded “failure summary” record per example. Include decode errors, tool failures, judge notes, and optionally output hash and rendered prompt hash.
- Keep these summaries serializable so the compiler can feed them into “reflect/propose” signatures.

## Compilation (Effect-only)

Compilation is an explicit Effect program: it takes `CompileJobSpecV1` plus implementation services and emits a compiled artifact and report.

### Compiler environment

In addition to runtime services, compilation needs:

- `EvalCache` (persistent cache is optional; in-memory is fine for MVP)
- `DatasetStore` (optional; for referencing dataset ids by name)
- `ArtifactStore` (PolicyRegistry can be used, but consider separate “artifact write” with provenance)

### Candidate representation

Treat a candidate as a pure `DseParams` object plus provenance:

- `candidateId` (hash of params + parentId + optimizer id)
- `params`
- optional `parentId`
- optional `notes` (short, bounded)

### MIPRO-like optimizer (Effect-only)

Implement a MIPRO-like loop without claiming strict algorithmic equivalence yet:

1. Generate candidate instructions from a **proposal signature**.
2. Evaluate each candidate via the existing eval harness.
3. Select best by reward (tie-break by hash).
4. Optionally iterate with a second proposal round seeded by best + failure summaries.

Key point: proposal is just another DSE signature, pinned like everything else:

- Proposal signature: `@openagents/dse/compiler/ProposeInstructions.v1`
- Its compiled artifact is pinned and recorded in the compile report.

This keeps compilation self-hosted and auditable in Effect-only TS.

### GEPA-like optimizer (Effect-only)

Implement GEPA-like behavior in three parts:

1. A `ParetoFrontier` module that tracks per-example wins (coverage) and samples parents proportional to coverage.
2. A reflection/proposal signature that mutates prompts based on failure summaries and parent prompt text.
3. A compile loop that evaluates offspring and inserts them into the frontier if they win anywhere.

Do not require a full “trace” system to start. A bounded failure summary per example is sufficient for a first pass.

Multi-objective support:

- Keep reward as scalar for selection, but track additional dimensions (cost/latency/tool calls) in the report.
- Frontier dominance can be extended later to true multi-objective (vector reward).

### Promotion and rollback

Promotion should remain pointer-only via `PolicyRegistry.setActive(signatureId, compiledId)`.
Rollback should remain history-based (DO table already exists in `apps/autopilot-worker/src/dseServices.ts`).

## Storage (Workers/DO)

Minimum DO tables/services needed for the Effect-only end-state:

- `dse_artifacts`: immutable compiled artifacts keyed by `(signature_id, compiled_id)`
- `dse_active_artifacts`: pointer to active compiled id per signature
- `dse_active_artifact_history`: append-only history for rollback
- `dse_receipts`: append-only predict receipts
- `dse_blobs`: content-addressed text blobs

Additional tables for RLM:

- `dse_varspace`: `{ thread_id, run_id, var_name, value_kind, blob_id?, json?, created_at }`
- `dse_trace_events`: `{ thread_id, run_id, seq, json, created_at }` (bounded retention)

## Testing Strategy (Effect-only)

Unit tests (package-level):

- Fake `LmClient` layers for deterministic compile/eval tests (already used in `packages/dse/test/*.test.ts`).
- Property tests for hashing determinism (optional, but cheap).
- RLM kernel tests: parse/execute action DSL deterministically, budget counters increment correctly.

Integration tests (worker-level):

- Existing DO-backed artifact store/promote/rollback tests in `apps/autopilot-worker/tests/index.test.ts`.
- Add an RLM “smoke test” once VarSpace/trace endpoints exist: verify iteration receipts are recorded and budgets are enforced.

## Suggested Deliverables (Effect-only, in-order)

1. Add `ExecutionBudget` service and enforce budgets in DirectPredict.
2. Add `PredictStrategy` abstraction and preserve DirectPredict semantics.
3. Implement RLM-lite kernel + VarSpace + trace events + RlmPredict strategy.
4. Extend compile with proposal-based instruction candidate generation (MIPRO-like).
5. Add Pareto frontier + reflection/proposal loop (GEPA-like).
