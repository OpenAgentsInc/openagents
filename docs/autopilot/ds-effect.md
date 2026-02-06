# DSE / ds-effect: DSPy, but Effect TS (Full Spec)

- **Status:** Draft (intended design; not fully implemented)
- **Last updated:** 2026-02-06
- **Source of truth (eventual):** `packages/ds-effect/`
- **If this doc conflicts with code behavior:** code wins

This spec defines `ds-effect` (aka “DSE”): a TypeScript + Effect library for **declarative, self-improving LM programs**. It is explicitly inspired by:

- **DSPy** (Python): signatures + programs + “compile” via eval loops (`~/code/dspy/`)
- **dsrs / DSRs** (Rust DSPy rewrite): typed signatures, optimizers, eval + manifest posture (`~/code/dsrs/` and `openagents/crates/dsrs/`)

The goal is not “Effect wrappers around an LLM client”. The goal is to make the *LLM-facing parts*:

- declarative (Signatures)
- typed (Schema IO)
- optimizable (explicit parameters + optimizers)
- measurable (datasets + metrics)
- shippable (compiled artifacts with deterministic IDs)
- production-safe (runtime loads artifacts; compilation is explicit and auditable)

---

## 0) Context and Motivation

Autopilot is moving toward Effect as the default application architecture (see `docs/autopilot/effect-migration-web.md`, `docs/autopilot/effect-telemetry-service.md`).

Effect solves wiring, reliability, observability, and testability. It does **not** automatically solve the “DSPy problem”:

- prompts and “agent behavior” drift across the codebase
- improvements happen via hand edits
- no disciplined eval loop exists to justify changes
- no artifact registry exists to pin and roll back behavior

`ds-effect` is the missing compiler-layer counterpart for an Effect-first codebase.

---

## 1) Big Decisions (Scope of This Spec)

If we built “DSPy, but Effect TS”, the big decisions are:

1. **Core IR**: what is the canonical representation of “a prompt/program surface” that can be rewritten safely?
2. **Optimizable surface**: what parameters are allowed to change under compilation, and how do we represent them?
3. **Run and evaluate**: how do we execute programs and measure them (datasets, metrics, judges, caching)?
4. **Serialize and ship artifacts**: what are the on-disk/on-wire formats, how do we hash/version them, and how does runtime load them?

This spec answers those four.

---

## 2) Terminology

- **Signature**: typed IO contract + prompt IR + constraints. “What this step does.”
- **Module**: a composable Effect program `I -> Effect<R, E, O>`. “How steps compose.”
- **Prompt IR**: structured AST representing the model-facing prompt/messages and policy blocks.
- **Params / Policy**: serializable “knobs” that compilation can change (instructions, few-shot, model config, decode/repair policy, tool policy).
- **Compiled Artifact**: immutable bundle `signatureId + params + hashes + eval report + provenance`.
- **Registry**: store for compiled artifacts, plus an “active/pinned artifact” pointer per signature.

---

## 3) Core Abstraction: Signature as Schema + Contract

### 3.1 Requirements

A `Signature` MUST:

- have a durable stable id (include version): `@openagents/<domain>/<Name>.vN`
- define typed input and output via **Effect Schema**
- define a **Prompt IR** (not a raw string)
- define decode/repair policy as part of its contract defaults
- define constraints: timeout, retry, max tokens, tool allowance, safety tags

It MUST be possible to:

- export a signature deterministically (for compilation tooling)
- compute stable hashes for:
  - input/output schema encodings
  - prompt IR normalization
  - default params/policy

### 3.2 Type sketch

```ts
import { Schema } from "effect"

export type SignatureId = string

export type DseSignature<I, O> = {
  readonly id: SignatureId

  // IO contracts
  readonly input: Schema.Schema<I>
  readonly output: Schema.Schema<O>

  // Model-facing description as structured IR.
  readonly prompt: PromptIR<I, O>

  // Default policy and constraint surface (compiler may override via artifact).
  readonly defaults: {
    readonly params: DseParams
    readonly constraints: SignatureConstraints
  }
}
```

### 3.3 “Strict by default” posture

Like dsrs, DSE SHOULD default to strictness:

- decoded output MUST satisfy `outputSchema` or fail (with bounded repair if configured)
- tool calling MUST be schema-validated
- all runs MUST emit stable IDs in logs/receipts (signatureId, compiled_id)

Unlike dsrs, DSE MUST keep a first-class prompt AST so compilation can safely rewrite it.

### 3.4 Signature Export (Compiler Input Format)

Signatures MUST be exportable into a deterministic, language-agnostic JSON form so:

- a compiler (TS-native or dsrs-backed) can optimize without importing app code
- we can hash contracts (schema + prompt IR) to enforce compatibility
- compiled artifacts can be audited independently of runtime

Define a *contract export* format and version it explicitly.

#### `SignatureContractExportV1` (JSON)

```ts
type SignatureContractExportV1 = {
  readonly format: "openagents.dse.signature_contract"
  readonly formatVersion: 1

  readonly signatureId: string

  // JSON schema outputs for Effect Schema input/output.
  // The generator and its version MUST be recorded in provenance.
  readonly inputSchemaJson: unknown
  readonly outputSchemaJson: unknown

  // Normalized prompt IR JSON (see §5.7).
  readonly promptIr: unknown

  // Default policy/constraints (compiler can override in the artifact).
  readonly defaultParams: unknown
  readonly defaultConstraints: unknown

  // Optional: declaration of tool shapes/policies, if the signature is tool-aware.
  readonly toolContract?: {
    readonly allowedToolNames: ReadonlyArray<string>
    readonly toolSchemasByName?: Record<string, unknown>
  }
}
```

#### Compile-time inputs are separate from the signature

The signature contract describes *what the step is*.

Compilation needs additional inputs describing *what we are allowed to change and how we evaluate*. That should be separate, e.g.:

- instruction variants
- few-shot example pool
- dataset + metric selection
- optimizer choice + configuration

Define a compile job spec that references the signature id and contains those knobs.

```ts
type CompileJobSpecV1 = {
  readonly format: "openagents.dse.compile_job"
  readonly formatVersion: 1

  readonly signatureId: string
  readonly datasetId: string
  readonly metricId: string

  readonly searchSpace: unknown // instruction variants, few-shot pool refs, model knobs, etc.
  readonly optimizer: { readonly id: string; readonly config?: unknown }
}
```

`ds-effect` SHOULD ship helpers to:

- export `SignatureContractExportV1` from in-repo signature definitions
- compute `schemaHash` and `promptIrHash`
- generate a stable `compileJobHash`

---

## 4) Execution Model: Program Is an Effect Graph

### 4.1 Modules and environment

In DSE, a “program” is a composition of modules:

- `Module<I, O, R>` where `R` is the required environment
- implemented as `Effect.Effect<R, ModuleError, O>`

Wiring MUST happen through Effect services + Layers, not global mutable settings.

This matches existing app posture:

- `apps/web/src/effect/*` already builds a single `ManagedRuntime` and passes it via router context

### 4.2 Core services (environment)

At minimum the runtime environment SHOULD include:

- `LmClient` (the actual model provider)
- `PolicyRegistry` (resolve active artifact for a signature)
- `PromptRenderer` (Prompt IR -> provider messages)
- `OutputDecoder` (provider response -> `O`, with repair pipeline)
- `Telemetry` (logs/spans/events; see `docs/autopilot/effect-telemetry-service.md`)
- `ReceiptRecorder` (canonical hashes, tool calls, timings)
- `Clock` and `Random` (testability/determinism)
- optional `Cache` (eval caching; runtime caching if safe)

### 4.3 Predict module

`Predict(signature)` MUST:

1. resolve `activeArtifact` for `signature.id` (or fall back to defaults)
2. apply params to prompt IR (instruction variant, few-shot selection, tool policy block, output format block)
3. render IR deterministically
4. call the model provider with bounded constraints
5. decode output into `O` (Schema-validated; repair as policy)
6. emit telemetry + receipts including:
   - `signatureId`
   - `compiled_id` (or `compiled_id: null` if default policy)
   - prompt hash, output hash (when possible)
   - token usage and latency (when possible)

### 4.4 Adapter boundary rule (match OpenAgents invariant)

Formatting/parsing is “adapter work”. Validation/retry/timeouts/receipts are “runtime work”.

- Prompt renderer/decoder MUST NOT implement retries.
- Retries/repair loops MUST be explicit `Effect` operators with bounded time.

---

## 5) Represent Prompts as IR (Core IR)

This is the core “DSPy-ness”.

### 5.1 Prompt IR requirements

Prompt IR MUST:

- be structured and versioned (e.g., `PromptIR.version = 1`)
- contain explicit blocks for instruction, few-shot, tools, output format
- be normalizable (canonical JSON) to produce stable hashes
- be renderable deterministically to provider messages

### 5.2 Prompt IR blocks

The minimum set of blocks:

- `SystemBlock`
- `InstructionBlock`
- `FewShotBlock` (structured examples, not raw text)
- `ToolPolicyBlock`
- `OutputFormatBlock` (derived from `outputSchema`)

Common optional blocks:

- `ContextBlock` (retrieval results, blueprint context, request context)
- `RubricBlock` (for judge signatures / eval)

### 5.3 Type sketch

```ts
export type PromptIR<I, O> = {
  readonly version: 1
  readonly blocks: ReadonlyArray<PromptBlock<I, O>>
}

export type PromptBlock<I, O> =
  | { readonly _tag: "System"; readonly text: string }
  | { readonly _tag: "Instruction"; readonly text: string }
  | { readonly _tag: "FewShot"; readonly examples: ReadonlyArray<FewShotExample<I, O>> }
  | { readonly _tag: "ToolPolicy"; readonly policy: ToolPolicy }
  | { readonly _tag: "OutputFormat"; readonly format: OutputFormatSpec }
  | { readonly _tag: "Context"; readonly entries: ReadonlyArray<ContextEntry> }
```

### 5.4 Few-shot examples are structured

`FewShotBlock` MUST store examples structurally:

- example id
- input value (typed)
- output value (typed)
- content hash (canonical encoded form)

The renderer converts them to text/messages deterministically.

### 5.5 OutputFormat block derived from Schema

`OutputFormatBlock` MUST be derived from `outputSchema`, not hand-written.

We SHOULD generate JSON schema from Effect Schema and include:

- schema hash
- “return JSON only” guidance
- any additional formatting hints (e.g., “no extra keys”)

---

## 6) What’s Optimizable (Explicit Params)

### 6.1 Params definition

DSE MUST model all optimizable degrees of freedom as a serializable `Params` object. Examples:

- `instructionVariantId` (or instruction text, but IDs are better)
- `fewShotExampleIds: string[]`
- `exampleSelector` knobs (k, strategy)
- model settings:
  - `temperature`, `top_p`, `max_tokens`
- `scratchpadStyle` (only if allowed; default: disallow or keep minimal)
- decode/repair policy:
  - max repair attempts, repair strategy, jsonish parse options
- tool policy knobs:
  - allowed tools list
  - timeouts, budgets
- router thresholds (when routers exist)

### 6.2 Param tree is explicit

DSPy finds predictors via runtime reflection; dsrs exposes an explicit parameter tree via `Optimizable.parameters()`.

In TS we SHOULD copy dsrs’s explicitness:

- composite modules expose `parameters(): Record<string, Optimizable>`
- only those parameters are eligible for compilation updates

This makes “what is learnable” a deliberate decision.

### 6.3 Compiled module payload

The compiler output for a signature is:

```ts
type CompiledModule = {
  readonly signatureId: SignatureId
  readonly params: DseParams
  readonly promptIRHash: string
  readonly outputSchemaHash: string
  readonly eval: EvalSummary
  readonly provenance: Provenance
}
```

---

## 7) Evaluation Harness: Datasets + Metrics + Judges

DSPy’s superpower is the eval loop. DSE MUST make evaluation a first-class product surface.

### 7.1 Dataset

Define:

- `Dataset<I, Y>` where `Y` is ground truth if available
- stable `exampleId`
- optional tags/splits (`train`, `val`, `test`)

### 7.2 Metric

Metric SHOULD be deterministic where possible:

- `metric(predicted: O, expected: Y) -> number | MetricReport`

If a judge is needed, it MUST be explicit:

- judge is another signature/module, compiled and pinned independently
- judge results must record the judge artifact id to avoid circular drift

### 7.3 Evaluate API

Evaluation MUST:

- run with bounded concurrency
- be cacheable by `(signatureId, compiled_id, exampleId, metricVersion)`
- produce a report with:
  - aggregate score (mean) + distribution
  - failure taxonomy (decode failures, tool failures, metric mismatches)
  - performance summaries (latency/tokens/cost if available)

### 7.4 Determinism

For compilation to be meaningful:

- sampling must be seeded (RNG in environment)
- all runs should record the model id + config
- canonical hashes should key caches and artifacts

---

## 8) Optimizers: Start With Two (Then Expand)

You don’t need all of DSPy on day one. The MVP optimizers are:

1. **Few-shot selection optimizer**
2. **Instruction search optimizer**

### 8.1 Few-shot selection optimizer

Goal: choose `k` examples from a pool that maximize a metric.

MVP strategies:

- greedy forward selection
- bandit-style sampling

### 8.2 Instruction search optimizer

Goal: choose among instruction variants.

MVP strategies:

- grid search
- successive halving (early stopping)

### 8.3 Joint search (later)

After MVP:

- joint search over `(fewshot × instruction × decode policy × model config)`
- router policy tuning
- self-refine policies (carefully; must remain bounded and auditable)

### 8.4 “No learning in prod” rule

Runtime execution MUST load compiled artifacts and run them. It MUST NOT “learn” (change params) unless explicitly enabled behind a feature flag / experiment harness.

---

## 9) Compilation Outputs: Shipable Artifacts

This is the “dsrs-like” posture: versioned, deterministic, auditable artifacts.

### 9.1 Artifact contents

A compiled artifact MUST include:

- `signatureId`
- `compiled_id` (canonical hash of policy JSON)
- selected examples (ids + content hashes, optionally embedded content)
- chosen instruction variant (id + text)
- model settings
- decode/repair policy
- tool policy (if any)
- `promptIRHash`
- `outputSchemaHash`
- `evalSummary`
- `provenance`:
  - compiler/optimizer version
  - dataset version + hash
  - git sha(s)

### 9.2 dsrs compatibility

We SHOULD align the manifest with dsrs concepts (see `crates/dsrs/docs/COMPILER-CONTRACT.md`):

- optimizer id
- trainset hash/id
- scorecard (proxy/truth scores, iterations, cost)
- compatibility requirements (tools, lanes, privacy modes)

Even if the TS runtime doesn’t use every field initially, keeping it compatible prevents “two ecosystems” later.

### 9.3 Canonical hashing

`compiled_id` MUST be derived from canonical JSON serialization:

- object keys sorted
- stable encoding rules
- hash = `sha256(canonical_json(policy_bundle))`

This is required so receipts and replays can refer to stable IDs.

---

## 10) Artifact Registry: Load + Pin + Roll Back

### 10.1 Registry responsibilities

The registry MUST support:

- store/retrieve artifacts by `(signatureId, compiled_id)`
- resolve active artifact by `signatureId` (promotion/pinning)
- roll back to previous artifact

Backends to support (incrementally):

- Autopilot worker: Durable Object SQLite (natural fit; co-located with thread state)
- shared/global: KV/R2 (optional)
- UI surfacing: Convex mirror (optional; not canonical)

### 10.2 Runtime receipt integration

Every production run SHOULD log:

- `signatureId`
- `compiled_id`
- prompt hash
- output hash (when feasible)
- latency + token usage + tool calls

This matches OpenAgents’ “everything is logged and replayable” posture (see `crates/dsrs/docs/ARTIFACTS.md` and `crates/dsrs/docs/REPLAY.md` for the broader worldview).

---

## 11) Output Decoding and Repair (Schema + Policy)

Schema validation happens after parsing, but parsing is the hard part.

### 11.1 Decode pipeline

Recommended pipeline (policy-controlled):

1. strip markdown fences
2. strict JSON parse
3. tolerant “jsonish” parse
4. `Schema.decode` into `O`
5. bounded repair loop:
   - either re-ask model for corrected JSON
   - or run a dedicated “repair signature”

### 11.2 Parser choice (open)

dsrs is exploring BAML’s jsonish parser (`~/code/dsrs/CURRENT_SPEC.md`). TS options:

- JS tolerant parser (jsonrepair/json5 + fence stripping)
- BAML via WASM
- provider-enforced structured outputs (when available)

Whatever we choose, it MUST be explicit in the compiled artifact (decode/repair policy).

---

## 12) Tooling Integration (Tools, Receipts, Safety, Cost)

This is where we intentionally diverge from vanilla DSPy.

Signature contracts SHOULD be able to specify:

- `ToolPolicy`: allowed tools, timeouts, budgets, max calls
- `ReceiptPolicy`: required receipt fields, truncation policies
- `SafetyPolicy`: redaction rules, privacy mode
- `CostPolicy`: max spend per call (if measurable)

Compilation MAY tune tool routing policies, but only with:

- explicit search space
- deterministic eval harness
- promotion gates

---

## 13) Testing Story: Deterministic Fake LLM + Golden Prompts

Effect is strongest when tests are deterministic.

### 13.1 `TestLLM` layer

Provide a test LLM service that returns canned outputs keyed by:

- `promptHash` (and optionally `compiled_id`)
- or `(signatureId, exampleId)`

### 13.2 Golden prompt snapshots

Snapshot:

- normalized prompt IR
- rendered provider messages

This catches regressions in rendering and compilation transforms.

### 13.3 Golden eval snapshots (compiled artifacts)

For pinned artifacts, store expected eval summaries keyed by dataset hash + metric version.

---

## 14) What NOT to Copy From DSPy

- Don’t rely on runtime dynamism/reflection to discover learnable parameters.
- Don’t mutate global state for LM settings; use Effect env and layers.
- Don’t make string parsing the output contract; use schema + decode/repair.
- Don’t make everything LLM-judged by default; prefer deterministic metrics.
- Don’t “learn in prod” unless explicitly enabled and auditable.

---

## 15) MVP Slice (Small but Real)

The smallest credible DSE:

1. `Signature<I,O>` with Schema IO + Prompt IR
2. `Predict(signature)` module
3. `Dataset` + `Metric` + `evaluate(...)`
4. `compile(...)` that searches over:
   - instruction variants
   - few-shot selection
5. artifact registry + runtime loader + receipts

---

## 16) Package Layout (Proposed)

We’ll move this to `packages/ds-effect/` when ready.

```
packages/ds-effect/
  src/
    signature/
    prompt-ir/
    predict/
    policy/
    eval/
    optimize/
    registry/
  test/
```

CLI shape (open decision):

- library-only first (scripts call `compile(...)`)
- or Bun-first CLI + library (`bun run ds-effect compile ...`)

Recommendation: start library-only. Add a CLI once artifact formats and registry semantics stabilize.

---

## 17) Worked Example: “Issue Triage” Signature

Signature:

- Input: `{ userRequest, repoSummary?, recentErrors? }`
- Output: `{ category, priority, nextActions[], confidence }`

Dataset:

- curated triage cases with ground truth category/priority

Metric:

- exact match for category/priority
- partial credit for nextActions overlap

Compile:

- try instruction variants
- greedy few-shot selection
- emit artifact with eval summary + provenance

Runtime:

- Autopilot worker loads active artifact and runs predictor
- receipts include `signatureId` + `compiled_id`

---

## 18) Open Decisions to Lock Next

1. Parsing strategy (JS vs BAML/WASM vs strict structured outputs)
2. Canonicalization rules for hashing
3. Registry backend strategy (DO SQLite only vs shared store)
4. Trace model (spans-only vs DAG)
5. Compilation engine (dsrs-backed first vs TS-native)

---

## 19) Next Steps

1. Implement Phase 0 runtime pieces in `packages/ds-effect/`:
   - signature + prompt IR render/hash
   - `Predict` with decode/repair policy
   - artifact registry interface + DO SQLite impl for worker
2. Wire one tiny signature inside `apps/autopilot-worker` behind a feature flag.
3. Build eval harness + instruction search optimizer.
4. Decide whether compilation runs via dsrs (recommended first) or TS-native.
