# Autopilot Optimization Plan (DSE-first, Horizons/Monty-inspired)

- **Status:** Proposed (implementation roadmap)
- **Last updated:** 2026-02-07
- **Primary input docs:** `docs/autopilot/dse/dse.md`, `docs/autopilot/synergies/horizons-synergies.md`, `docs/autopilot/synergies/monty-synergies.md`, `docs/autopilot/synergies/microcode-synergies.md`, `docs/autopilot/synergies/rlm-synergies.md`

This plan proposes a unified roadmap to implement **DSE** (“DSPy, but Effect TS”) and selectively adopt the best patterns from **Horizons** (graph execution, evaluation/optimization shape, budgets, memory, evented traces) and **Monty** (secure “code mode” with externals + snapshot/resume).

The bias is **Effect-first**: we prefer implementing the concepts as Effect services and typed contracts in this repo, and only “use Horizons/Monty directly” when it buys something we cannot reasonably build (or when we can integrate cleanly without violating our Workers/no-containers constraints).

---

## Goals (what “done” looks like)

- **DSE is real in production paths**: Autopilot runtime can run at least one DSE `Signature` end-to-end, with schema-validated IO, receipts, and a resolved/pinned `compiled_id`.
- **Optimization is disciplined**: we can run an offline compile loop (dataset + metric + optimizer) and produce immutable, hash-addressed artifacts that can be promoted/rolled back.
- **Evaluation is composable**: deterministic metrics are first-class; LLM judges are explicit and pinned; we can aggregate “reward signals” into a single score.
- **Execution is bounded**: budgets (time, steps, tool calls, LLM calls) are enforced per turn and per DSE run.
- **Replayability improves**: logs/receipts carry stable IDs (signatureId, compiled_id, prompt hash, tool receipts) sufficient to reproduce and debug.
- **Optional “code mode” is possible** (later): if we ever run LLM-authored code, it is capability-limited, observable, and checkpointable at I/O boundaries.

---

## Non-goals (for now)

- A full Horizons-compatible YAML engine.
- “Learning in prod” (DSE runtime must load artifacts; compilation is explicit and out-of-band).
- A big memory/RAG product. We’ll design the interfaces early but keep the MVP small.
- Depending on Rust crates for TS DSE compilation (DSE compile engine is TS/Effect-native by repo constraint).

---

## Unified mental model (DSE × Horizons × Monty × Microcode × RLM)

### DSE core (from `dse.md`)
- **Signature** = typed IO (Effect Schema) + **Prompt IR** + defaults + constraints.
- **Predict(signature)** = resolve active artifact → apply params → render IR → call LLM → decode/repair → emit receipts.
- **Compile(job)** = candidate generation → evaluate on dataset/holdout → choose best → emit artifact → promote via registry.

### Horizons patterns we should port (from `horizons-synergies.md`)
- **Explicit execution graph**: DAG of node types; shared exec state; input/output mappings.
- **Budgets** on every run: max supersteps, max LLM calls, max time.
- **Optimization loop shape** (mipro_v2-like): sampler → evaluator → early stopping → best candidate.
- **Evaluation/reward** (RLM-like): multiple weighted signals → aggregate to a reward in \([0,1]\) + report.
- **Memory** (Voyager-like): append-only items, scoped retrieval with recency bias and optional summarization.
- **Evented trace**: node start/finish, timing, tool/LLM calls; queryable/auditable.

### Monty patterns we should port (from `monty-synergies.md`)
- **Externally-defined capabilities only** (“externals” are the only side effects).
- **Pause at I/O** and checkpoint (serialize) then resume.
- **Typecheck/validate before run** (against allowed externals).
- **Hard resource limits**.

### Microcode patterns we should port (from `microcode-synergies.md`)
- **“Compiled artifact first” posture**: runtime resolves/pins an immutable artifact id and logs it (analogous to Microcode’s precompiled program pinning).
- **Config precedence + persistence**: a deterministic precedence chain (request overrides/env/durable config) with auditable resolution and stable hashes in receipts.
- **Large blob side channel**: store large pasted/code blobs once and reference them by id/hash in Prompt IR and receipts/replay.
- **Multi-model roles**: explicitly model primary vs sub/aux models (and extend to judge/repair/router roles) as policy knobs in artifacts.
- **Dynamic tool mounting**: treat MCP servers (and future providers) as `ToolProvider`s that can mount namespaced tools with schemas and receipt hooks.
- **Debug/trace toggles**: structured “trajectory” traces when enabled (bounded and redactable), not unstructured internal monologue in prod.

### RLM patterns we should port (from `rlm-synergies.md`)
- **Two-bucket context**: keep huge inputs in variable space (vars/blobs), load only previews/slices into token space.
- **Recursion via sub-LM**: budgeted sub-calls whose results land in variable space (not automatically in the main prompt).
- **Inference-time strategy**: RLM should be a swappable execution strategy that works with existing DSE signatures.
- **Budgets fit the model**: add `maxIterations` and `maxSubLmCalls` alongside time/tool/LLM-call budgets.

---

## Roadmap (phases and deliverables)

### Phase 0 — DSE “production spine” (minimum credible DSE)

**Objective:** ship the DSE runtime primitives needed for real usage and later optimization.

- **Signature + Prompt IR**
  - Implement `DseSignature<I,O>` backed by Effect Schema.
  - Implement Prompt IR v1 blocks (`System`, `Instruction`, `FewShot`, `ToolPolicy`, `OutputFormat`, `Context`, optional `Rubric`).
  - Implement deterministic normalization + hashing (`promptIrHash`, optional `renderedPromptHash`).

- **Predict(signature)**
  - Resolve active artifact (or defaults).
  - Apply params to IR via explicit allowlisted transforms (instruction variant, few-shot ids, tool policy restrictions, output-format tightening).
  - Render deterministically (provider adapter boundary).
  - Decode + bounded repair policy.
  - Emit receipts: `signatureId`, `compiled_id?`, hashes, timing, tool/LLM metadata.

- **Blob references for large context**
  - Add a Prompt IR `Context` entry type for blob references (id/hash/size/mime), backed by a `BlobStore` service.
  - Store large pasted/code artifacts once (DO SQLite/R2), reference by hash/id in prompts and receipts.

- **Artifact format + Registry**
  - Implement schema-validated `DseCompiledArtifactV1` load/store.
  - Implement `PolicyRegistry` interface with at least one backend suitable for Autopilot:
    - **Durable Object SQLite** registry (matches `dse.md` §10.3).
  - Promotion is pointer-only; artifacts are immutable.

**Horizons/Monty adoption in this phase:** concept-only.
- We **do not** implement a full DAG engine yet, but we do structure `Predict` so it can be a “node” later.
- We **do** design receipts so they can become an event stream (“trace”).

**Exit criteria**
- At least one in-repo signature runs in Autopilot and records `signatureId` + `compiled_id` consistently.
- Artifacts can be stored, resolved, promoted, and rolled back.

#### Phase 0 Implementation Log (2026-02-07)

- `packages/dse/`
  - Added deterministic hashing helpers (`promptIrHash`, `renderedPromptHash`, schema/params hashes) in `packages/dse/src/hashes.ts`.
  - Added schema-validated compiled artifact format `DseCompiledArtifactV1` in `packages/dse/src/compiledArtifact.ts`.
  - Added `BlobRef` + `BlobStore` (in-memory + DO-SQLite wiring on the worker side) and extended Prompt IR Context entries to support blob refs.
  - Upgraded `Predict` to:
    - resolve active artifact via `PolicyRegistry` (`getActive` + `getArtifact`)
    - apply allowlisted transforms (instruction, few-shot, tool policy)
    - decode with `strict_json` or `jsonish` + bounded re-ask repair
    - emit `PredictReceiptV1` receipts (hash-first) via `ReceiptRecorder`

- `apps/autopilot-worker/`
  - Implemented Durable Object SQLite-backed `PolicyRegistry` + `BlobStore` + `ReceiptRecorder` layers and wired them into the DSE signature used for Blueprint tool routing.
  - Added DO endpoints for artifact storage/promotion/rollback and receipt listing:
    - `.../dse/artifacts` (GET/POST)
    - `.../dse/active` (GET/POST/DELETE)
    - `.../dse/rollback` (POST)
    - `.../dse/receipts` (GET)
  - Auto-installs a default compiled artifact and sets an active pointer for Blueprint tool routing so Autopilot chat runs with a pinned `compiled_id` by default.

- Verification
  - `cd packages/dse && bun test && bun run typecheck`
  - `cd apps/autopilot-worker && npm test && npm run typecheck`

---

### Phase 1 — Evaluation harness (DSE eval) with “reward signals” (Horizons RLM shape)

**Objective:** make evaluation first-class and composable, enabling compile loops.

- **Dataset**
  - Define `Dataset<I, Y>` with stable `exampleId`, splits/tags, and deterministic iteration order.

- **Metrics**
  - Prefer deterministic metrics (`(pred: O, expected: Y) => MetricReport`).
  - Add explicit **judge signatures** for non-deterministic evaluation; judges are pinned artifacts and recorded in reports.

- **Reward signals (Horizons RLM idea, Effect-first)**
  - Introduce an evaluation layer that supports multiple signals:
    - format validity (schema decode success)
    - exact/partial match against `Y`
    - tool failure penalties
    - judge score (explicit signature)
  - Weighted aggregation yields a normalized reward \([0,1]\).
  - Serialize `EvalSummaryV1` (and optionally bounded per-example details).

- **Eval caching keys**
  - Cache by `(signatureId, compiled_id, datasetHash, metricVersion, exampleId)` where feasible.

**Exit criteria**
- `evaluate(signature, artifact, dataset, metricOrRewardBundle)` produces stable reports and can be run offline deterministically (seeded sampling recorded).

#### Phase 1 Implementation Log (2026-02-07)

- `packages/dse/`
  - Added Dataset primitives (stable `exampleId`, `split`/`tags`, deterministic ordering + hashing, deterministic sampling) in `packages/dse/src/eval/dataset.ts`.
  - Added Metrics:
    - deterministic metrics (pure scoring)
    - judge metrics backed by pinned DSE judge signatures + artifacts (recorded in metric reports)
    in `packages/dse/src/eval/metric.ts`.
  - Added Reward bundles and signals (format validity, metric signal, tool-failure penalty) + weighted aggregation in `packages/dse/src/eval/reward.ts`.
  - Added Eval cache keys + in-memory/noop cache layers in `packages/dse/src/eval/cache.ts` and wired caching into evaluation.
  - Added `Eval.evaluate(...)` in `packages/dse/src/eval/evaluate.ts` producing `EvalSummaryV1` with `datasetHash`, `(metricId, metricVersion)`, and selection hash (plus optional sampling seed).
  - Exported new eval modules via `packages/dse/src/index.ts`.

- Verification
  - `cd packages/dse && bun test && bun run typecheck`
  - `cd apps/autopilot-worker && npm test && npm run typecheck`

---

### Phase 2 — Compiler loop (DSE compile), shaped like Horizons `mipro_v2` (but TS/Effect-native)

**Objective:** turn eval into optimization; produce artifacts with provenance.

- **Compile job spec**
  - Implement `CompileJobSpecV1` and stable `compileJobHash`.
  - Search space definitions for:
    - instruction variants
    - few-shot example pools + k/selector
    - optional decode policy variants
    - optional model config variants

- **Optimizer interfaces (mipro_v2 shape)**
  - `VariantSampler`: propose candidate `DseParams` (plus provenance).
  - `Evaluator`: score candidates on holdout using Phase 1 eval.
  - `StoppingPolicy`: early stopping / successive halving.

- **MVP optimizers**
  - instruction grid search (variants)
  - greedy few-shot forward selection
  - optional successive halving across candidate sets

- **Artifact emission**
  - Emit `DseCompiledArtifactV1` with:
    - contract hashes, params hash, eval summary
    - optimizer id/config/iterations
    - provenance (git sha, dataset hash, metric id)

**Exit criteria**
- We can run compile offline and promote a better artifact, with a measurable improvement on holdout.

#### Phase 2 Implementation Log (2026-02-07)

- `packages/dse/`
  - Added Phase 2 compile job + hashing in `packages/dse/src/compile/job.ts` (`CompileJobSpecV1`, `compileJobHash`).
  - Implemented a TS/Effect-native compiler loop in `packages/dse/src/compile/compile.ts`:
    - instruction grid search optimizer (`instruction_grid.v1`)
    - greedy few-shot forward selection optimizer (`fewshot_greedy_forward.v1`)
    - joint optimizer (`joint_instruction_grid_then_fewshot_greedy_forward.v1`)
    - emits schema-validated `DseCompiledArtifactV1` with eval summary + provenance (dataset hash, metric id, searchSpaceHash).
  - Exported compile APIs via `packages/dse/src/index.ts`.
  - Added compile tests in `packages/dse/test/compile.test.ts`.

- Verification
  - `cd packages/dse && bun test && bun run typecheck`
  - `cd apps/autopilot-worker && npm test && npm run typecheck`

---

### Phase 2.5 — RLM-style inference for long-context tasks (two-bucket context)

**Objective:** mitigate context rot for huge inputs by adding an RLM execution strategy that works with existing signature contracts.

Context rot is a **quality** failure past soft limits (not just "context window exceeded"). See `docs/autopilot/reference/context-failures.md` for the taxonomy (rot vs poisoning vs confusion) and the telemetry we need to detect it.

- **RLM strategy (Effect-first)**
  - Add `RlmPredict(signature)` as a swappable inference-time strategy.
  - Persist a variable space (`VarSpace`) whose values are typed metadata + blob references (aligns with Phase 0 `BlobStore`).
  - Start with an RLM-lite “action DSL” kernel (deterministic, replayable). Keep arbitrary code execution behind a strict boundary (see Phase 6 + Monty model).
  - Prefer **symbolic recursion** for large contexts: kernel/code-driven fanout over chunks (do not require the controller LM to emit O(N) subcalls). Reference: `crates/rlm/docs/METHODS.md`.

- **Recursion and model roles**
  - Add explicit `sub` model role (in addition to main/judge/repair) and pin role selection in artifacts.
  - Expose recursion as a budgeted operation (`sub_lm(...)`) whose results land in variable space.

- **Budgets**
  - Enforce `maxIterations` (REPL turns) and `maxSubLmCalls` (recursive calls), in addition to Phase 3 budgets.

- **Receipts and replay**
  - Emit structured per-iteration trace events: actions executed, blobs accessed (by ref/hash), sub-LM calls, and derived-variable writes.
  - Treat traces as an input to distillation: mine repeating tactics and convert them into typed signatures/modules/graphs (see `docs/autopilot/dse/rlm-trace-mining.md`).

**Exit criteria**
- At least one long-context Autopilot workload (logs/codebase subset/evidence sourcing) runs reliably under strict budgets with auditable receipts.

---

### Phase 3 — Bounded execution budgets everywhere (Horizons budgets)

**Objective:** ensure every Autopilot/DSE run is safe, predictable, and cost-controlled.

- **Runtime budget service (Effect-first)**
  - Add a service (e.g. `ExecutionBudget`) that enforces:
    - `maxTimeMs`
    - `maxSteps` (supersteps)
    - `maxLlmCalls`
    - `maxToolCalls`
    - `maxOutputChars` (Microcode-style hard cap on emitted content)
    - optional `maxTokens` / `maxCost` when measurable
  - Thread budget context through:
    - `Predict`
    - tool execution
    - any future multi-step pipeline/graph

- **Receipt/trace integration**
  - Record budget consumption events so “why did we stop?” is auditably answered.

**Exit criteria**
- Budget exhaustion is a first-class, typed failure with clear receipts and safe termination.

---

### Phase 4 — “Graph-shaped” programs (Horizons DAG), without abandoning Effect

**Objective:** get the benefits of explicit DAG execution and traceability, but keep Effect as the implementation language.

Instead of adopting Horizons YAML, implement a **small TS graph IR** that compiles to Effect:

- **Graph IR (minimal)**
  - Node types:
    - `SignatureNode` (calls `Predict(signature)`)
    - `ToolNode` (calls tool runtime)
    - `TransformNode` (pure data mapping)
    - optional `JudgeNode` (evaluation signature)
  - Explicit edges + declared IO mapping (JSON path) between node state and node inputs/outputs.

- **Graph runner**
  - Executes DAG with:
    - budgets (Phase 3)
    - concurrency controls
    - per-node receipts (start/end, duration, hashes, failures)
  - “Verifier graphs” become a first-class reusable pattern: summarize trace → judge → reward signals.

**Why this matters for DSE**
- DSE “programs” (modules) remain Effect, but we gain:
  - explicit step boundaries (better receipts)
  - structured pipelines for evaluation and verification
  - a natural place to enforce budgets and capture traces

**Exit criteria**
- At least one multi-step Autopilot flow is expressed as a DAG and executed with full trace + budgets.

---

### Phase 5 — Memory service (Voyager-like), explicitly scoped (optional)

**Objective:** add long-term context without coupling it to DSE internals.

- **Memory interface (Effect service)**
  - `append(item)` (immutable, append-only)
  - `retrieve(query, k, filters)` (vector + recency blend)
  - optional `summarize(scope)` to compress memory
  - scope keys aligned to Autopilot model (user/thread; future org/project optional)

- **Backends**
  - Start with minimal (DO SQLite table or KV) + naive retrieval.
  - Add embeddings/vector store only when the product requires it.

**Exit criteria**
- One DSE signature can request memory context via `Memory` service, and retrieval is observable and receipt-backed.

---

### Phase 6 — Optional “code mode” (Monty-inspired), capability-limited and checkpointable (future)

**Objective:** enable a safe alternative to multi-round tool calling, where the model emits a small program that calls tools as functions.

**Important constraint:** Workers cannot run native Node addons directly in many deployments. So the first step is **design + boundary**, not immediate in-worker execution.

- **Unify capability surfaces**
  - Treat “tools” and “code externals” as the same registry:
    - same names
    - same schemas
    - same policies/budgets

- **Code execution interface**
  - A `CodeExecutor` Effect service that supports:
    - `plan`/`validate` (typecheck/contract-check)
    - `run` with strict externals allowlist
    - checkpoint at each external call (pause/resume model)
    - hard limits (time/memory/depth)

- **Implementation strategy (Effect-first)**
  - Start with a “stub executor” only in tests/dev that demonstrates the pause/resume contract without running Python.
  - If/when we adopt Monty:
    - run Monty in a separate host (small service) or a compatible environment and expose it via a narrow RPC
    - keep all side effects in our tool runtime (Monty only requests externals)
    - store snapshots/checkpoints in DO storage for durability

**Exit criteria**
- A proof-of-concept flow can execute a small “code plan” that calls tools, with receipts at each external boundary, and can resume from a checkpoint after an intentional abort.

---

## Cross-cutting design rules (to keep the system coherent)

- **Everything optimizable is explicit params**: no hidden prompt edits; compilation only touches allowlisted transforms.
- **Artifacts are immutable**: promotion is pointer-only; rollback is trivial.
- **No learning in prod**: runtime executes pinned artifacts; compilation is explicit/offline.
- **Budgets are enforced at runtime boundaries**: LLM calls, tool calls, graph steps, and (future) code execution.
- **Adapters only format/parse**: retries/repairs/timeouts live in runtime operators.
- **Receipts are stable and replayable**: always include `signatureId` and `compiled_id` when known; hash inputs/IR where safe.
- **Judges are just signatures**: pinned, auditable, and recorded in eval provenance.

---

## What we “use” from Horizons/Monty (pragmatic guidance)

### Horizons
- **Use the ideas, not the code (default)**:
  - DAG + budgets + traces
  - mipro_v2-shaped compile loop (sampler/evaluator/early stop)
  - RLM-shaped reward signals (weighted aggregation)
  - Voyager-shaped memory (scope + append + retrieve + recency bias)
- **When to integrate directly (rare)**:
  - only if we need a Rust-side graph runner for performance or compatibility, and it can be delivered as a stable service boundary without dragging runtime coupling into DSE.

### Monty
- **Use the model as the contract**:
  - externals-only side effects
  - pause/resume at I/O
  - typecheck/validate pre-run
  - strict limits
- **Integrate Monty later via a boundary**:
  - avoid coupling DSE to a specific interpreter
  - keep capability surface in our tool registry

---

## Suggested immediate next actions (high-signal worklist)

- **Implement Phase 0** in `packages/dse/` (or wherever DSE is landing) and wire a single signature in Autopilot behind a feature flag.
- **Define Phase 1 eval** interfaces and one “reward bundle” metric that combines format validity + deterministic match + optional judge.
- **Implement Phase 2 compile MVP** (instruction variants + greedy few-shot) and prove “artifact improves metric” on a tiny dataset.
- **Add Phase 3 budgets** to `Predict` and tool execution, and record budget consumption in receipts.
