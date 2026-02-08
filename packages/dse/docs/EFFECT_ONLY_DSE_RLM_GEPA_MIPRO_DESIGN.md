# Effect-Only Design: DSE + RLM + GEPA + MIPRO

Generated: 2026-02-08

This doc proposes an **Effect-only** (TypeScript + Effect) architecture for:

- DSE runtime (`Predict`, artifacts, policies, receipts, blobs)
- RLM as an inference strategy (two-bucket context + recursion + budgets)
- MIPRO-like and GEPA-like compilation loops (candidate generation + eval + promotion)

Constraints assumed:

- Autopilot MVP is **Convex-first** in `apps/web` (no per-user Durable Objects / DO-SQLite execution plane; DO classes are deprecated shims).
- `apps/autopilot-worker` is a DO-SQLite-based reference integration (non-MVP).
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
Apps (Workers) own production implementations of (2) for the MVP, typically backed by **Convex** (canonical state) and blob storage (R2 or equivalent). Durable Objects / DO-SQLite can be reintroduced post-MVP as an execution-plane optimization, but are not assumed.

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

Status: **implemented (v1 subset)** in `packages/dse/src/runtime/budget.ts` as `ExecutionBudgetService`.

Current enforced limits:

- `maxTimeMs`
- `maxLmCalls`
- `maxOutputChars`

Current API surface:

```ts
export type ExecutionBudget = {
  readonly start: (options: {
    readonly runId: string
    readonly startedAtMs?: number | undefined
    readonly limits: { readonly maxTimeMs?: number; readonly maxLmCalls?: number; readonly maxOutputChars?: number }
  }) => Effect.Effect<BudgetHandle>
}

export type BudgetHandle = {
  readonly checkTime: () => Effect.Effect<void, BudgetExceededError>
  readonly onLmCall: () => Effect.Effect<void, BudgetExceededError>
  readonly onOutputChars: (n: number) => Effect.Effect<void, BudgetExceededError>
  readonly snapshot: () => Effect.Effect<BudgetSnapshotV1>
}
```

Planned extensions for RLM + tools:

- `onToolCall`, `onRlmIteration`, `onSubLmCall`
- tool-call limits and per-tool timeouts

Budget limits and budget usage snapshots should remain serializable and included in receipts.

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

- Enforce timeouts and budgets via `ExecutionBudget` and `SignatureConstraints` (budgets are implemented for DirectPredict; signature timeout/tool budgets still pending).
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

VarSpace should be per-thread (Convex thread in the MVP execution plane), keyed by `{ threadId, runId }` or `{ threadId }` depending on retention needs.

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
Rollback should remain history-based (append-only “active artifact history”), but in the MVP execution plane this history should live in **Convex**. (`apps/autopilot-worker/src/dseServices.ts` shows the same pattern in DO-SQLite, but that’s non-MVP.)

## Storage (Convex-first MVP)

Minimum Convex tables/services needed for the Effect-only end-state (names illustrative):

- `dseArtifacts`: immutable compiled artifacts keyed by `{ signatureId, compiled_id }`
- `dseActiveArtifacts`: pointer to active `compiled_id` per `signatureId`
- `dseActiveArtifactHistory`: append-only history for rollback
- `dseReceipts`: append-only predict receipts (and later tool/trace receipts) keyed by `runId` / `receiptId`
- `dseBlobs`: content-addressed blobs (or pointers to R2) keyed by `blobId` / hash

Additional tables for RLM:

- `dseVarSpace`: `{ threadId, runId?, varName, valueKind, blobId?, json?, createdAt }`
- `dseTraceEvents`: `{ threadId, runId, seq, json, createdAt }` (bounded retention)

Notes:

- Writes should be idempotent and bounded; follow the chunked/append-only posture in `docs/autopilot/anon-chat-execution-plane.md`.
- Post-MVP, a DO-SQLite execution plane can mirror the same logical schema (see the reference integration in `apps/autopilot-worker/src/dseServices.ts`).

## Testing Strategy (Effect-only)

Unit tests (package-level):

- Fake `LmClient` layers for deterministic compile/eval tests (already used in `packages/dse/test/*.test.ts`).
- Property tests for hashing determinism (optional, but cheap).
- RLM kernel tests: parse/execute action DSL deterministically, budget counters increment correctly.

Integration tests (worker-level):

- Existing Convex-first Worker + Convex tests in `apps/web/tests/worker/` (for example `apps/web/tests/worker/chat-streaming-convex.test.ts`).
- Add a DSE/RLM “smoke test” once Convex-backed VarSpace/trace endpoints exist: verify iteration receipts are recorded and budgets are enforced.

## Implementation Roadmap (Effect-only, Testable Steps)

Each step should land with:

- One or more tests that fail before the change and pass after.
- A clear verification command set (`packages/dse` unit tests, plus `apps/web` worker/Convex tests for MVP wiring changes).

### Step 0: Keep The Baseline Green (already true today)

- Scope: no behavior changes.
- Verification: `cd packages/dse && bun test && bun run typecheck`
- Verification: `cd apps/web && npm test && npm run lint`

### Step 1: Add Execution Budgets To DirectPredict

- Goal: enforce time/LM-call/output bounds in `Predict.make` (DirectPredict), and record budget snapshots in receipts.
- Code: add `packages/dse/src/runtime/budget.ts` (`ExecutionBudgetService`) with an in-memory implementation.
- Code: extend `packages/dse/src/params.ts` with optional budget knobs (for example `budgets.maxLmCalls`, `budgets.maxOutputChars`, `budgets.maxTimeMs`).
- Code: wire budget checks into `packages/dse/src/runtime/predict.ts` and record an optional `budget` field in `packages/dse/src/runtime/receipt.ts`.
- Tests: add `packages/dse/test/budget.test.ts` to assert budgets fail closed (for example `maxLmCalls=0` rejects, `maxOutputChars` rejects on oversized outputs).
- Tests: update `packages/dse/test/predict.test.ts` to provide an `ExecutionBudgetService` layer.
- Verification: `cd packages/dse && bun test && bun run typecheck`
- Status: implemented (commit `42507656f`).

### Step 2: Add TraceRecorder And Emit Predict Trace Events

- Goal: provide a bounded trace stream for debug/replay without relying on free-form logs.
- Code: add `packages/dse/src/runtime/trace.ts` (`TraceRecorderService`) with `noop` and in-memory implementations.
- Code: emit trace events from `packages/dse/src/runtime/predict.ts` (start, renderedPromptHash, LM response metadata, decode/repair attempts, end).
- Tests: add `packages/dse/test/trace.test.ts` verifying trace ordering and bounds (truncation/limits).
- Verification: `cd packages/dse && bun test && bun run typecheck`

### Step 3: Introduce PredictStrategy Selection (Direct vs RLM)

- Goal: make inference strategy swappable while keeping `DseSignature` stable.
- Code: add `packages/dse/src/runtime/predictStrategy.ts` (or similar) with strategy ids `direct.v1` and `rlm_lite.v1`.
- Code: keep existing `Predict.make` API, but resolve strategy from the active artifact params (default to `direct.v1`).
- Tests: add `packages/dse/test/strategy.test.ts` verifying default is `direct.v1` and selection is pinned by artifact params.
- Verification: `cd packages/dse && bun test && bun run typecheck`

### Step 4: Add VarSpace And RLM Action Schemas (No RlmPredict Yet)

- Goal: build the deterministic kernel foundation before wiring it into prediction.
- Code: add `packages/dse/src/runtime/varSpace.ts` (`VarSpaceService`) with an in-memory implementation.
- Code: add `packages/dse/src/runtime/rlm/action.ts` defining the action DSL with Effect Schema decode (preview/search/chunk/write_var/final).
- Code: add `packages/dse/src/runtime/rlm/kernel.ts` implementing deterministic action execution (no LM calls yet).
- Tests: add `packages/dse/test/rlmKernel.test.ts` for preview/search determinism and stable failure modes for invalid inputs.
- Verification: `cd packages/dse && bun test && bun run typecheck`

### Step 5: Implement RlmPredict Minimal Loop (preview + final)

- Goal: ship the smallest end-to-end RLM loop with strict budgets and trace events.
- Code: add `packages/dse/src/runtime/rlm/predict.ts` implementing `RlmPredict` with `maxIterations` and a strict JSON action contract.
- Code: reuse `BlobStore` + `VarSpace` + `TraceRecorder` + `ExecutionBudget`.
- Tests: add `packages/dse/test/rlmPredict.test.ts` with a fake `LmClient` that emits a `preview` action then a `final` action.
- Tests: verify iteration receipts/trace events are emitted and budget counters increment correctly.
- Verification: `cd packages/dse && bun test && bun run typecheck`

### Step 6: Expand RLM Actions (search, chunk, write_var) And Observation Summaries

- Goal: make RLM useful for long context without tool execution.
- Code: extend `packages/dse/src/runtime/rlm/kernel.ts` with bounded outputs and deterministic truncation rules.
- Code: implement bounded observation summaries appended to token space each iteration (avoid leaking full blob data).
- Tests: extend `packages/dse/test/rlmPredict.test.ts` to cover `search` and `chunk` flows.
- Verification: `cd packages/dse && bun test && bun run typecheck`

### Step 7: Add sub_lm And Explicit Model Roles

- Goal: enable recursion while keeping costs bounded and auditable.
- Code: extend `packages/dse/src/params.ts` to support role-based model selection (for example `modelRoles.main`, `modelRoles.sub`, `modelRoles.judge`, `modelRoles.repair`).
- Code: add an RLM action `sub_lm` that calls `LmClient` with the `sub` role configuration and records `onSubLmCall`.
- Tests: add a fake `LmClient` that distinguishes calls (main vs sub) and assert the right routing/budget counters.
- Verification: `cd packages/dse && bun test && bun run typecheck`

### Step 8: Convex Storage For VarSpace And Trace Events

- Goal: make RLM durable per-thread in the **Convex-first MVP** execution plane.
- Code: extend Convex schema + functions in `apps/web/convex/` to persist `VarSpace` and `TraceRecorder` (bounded, append-only).
- Code: add minimal Worker endpoints under `apps/web/src/effuse-host/` for reading trace events/varspace keys (bounded, debug-only).
- Tests: add a worker integration test under `apps/web/tests/worker/` that runs a tiny RLM loop and asserts trace events are persisted and budget failures return structured errors.
- Verification: `cd apps/web && npm test && npm run lint`

### Step 9: MIPRO-Like Instruction Proposal Optimizer (Effect-only)

- Goal: go beyond grid search by generating instruction variants via a pinned proposal signature.
- Code: extend `packages/dse/src/compile/job.ts` optimizer ids to include `instruction_propose.v1`.
- Code: implement proposal-based compilation in `packages/dse/src/compile/compile.ts` (or a new module under `packages/dse/src/compile/optimizers/`).
- Code: define a compiler-owned proposal signature (in `packages/dse/src/compile/signatures.ts`) and require its artifact to be pinned in the compile run report.
- Tests: add `packages/dse/test/compilePropose.test.ts` using a fake `LmClient` that returns deterministic candidate lists, and verify the best candidate is selected by reward.
- Verification: `cd packages/dse && bun test && bun run typecheck`

### Step 10: GEPA-Like Optimizer (Pareto Frontier + Reflect/Propose)

- Goal: implement a reflection-driven evolutionary loop with diversity preserved by coverage.
- Code: add `packages/dse/src/compile/pareto.ts` and a `gepa_lite.v1` optimizer implementation.
- Code: define a pinned reflect/propose signature (inputs: parent instruction + failure summaries; output: mutated instruction).
- Tests: add `packages/dse/test/compileGepaLite.test.ts` with a fake `LmClient` that proposes improving mutations; assert frontier retains multiple candidates and best reward improves within budget.
- Verification: `cd packages/dse && bun test && bun run typecheck`

### Step 11: Wire Compile/Promote Into A Real Surface (Worker + Web)

- Goal: make “compile -> artifact -> promote -> rollback” usable without manual DB pokes.
- Code (worker): add a compile-job endpoint that runs compile with explicit budgets and stores the resulting artifact; promotion stays explicit.
- Code (web): add a minimal UI to submit compile jobs, view reports, and promote/rollback artifacts.
- Tests (worker): integration tests asserting compile jobs are bounded and produce stored artifacts.
- Verification: `cd apps/autopilot-worker && npm test && npm run typecheck`
