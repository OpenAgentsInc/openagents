# DSE / dse: DSPy, but Effect TS (Full Spec)

- **Status:** Draft (intended design; not fully implemented)
- **Last updated:** 2026-02-10
- **Source of truth (eventual):** `packages/dse/`
- **If this doc conflicts with code behavior:** code wins

This spec defines `dse` (aka “DSE”): a TypeScript + Effect library for **declarative, self-improving LM programs**. It is explicitly inspired by:

- **DSPy** (Python): signatures + programs + “compile” via eval loops (`~/code/dspy/`)
- **dsrs / DSRs** (Rust DSPy rewrite): typed signatures, optimizers, eval + manifest posture (`~/code/dsrs/` and `openagents/crates/dsrs/`)

The goal is not “Effect wrappers around an LLM client”. The goal is to make the *LLM-facing parts*:

- declarative (Signatures)
- typed (Schema IO)
- optimizable (explicit parameters + optimizers)
- measurable (datasets + metrics)
- shippable (compiled artifacts with deterministic IDs)
- production-safe (runtime loads artifacts; compilation is explicit and auditable)

For a plain-language "how to use what is implemented today" guide (operator workflow + Autopilot UX + RLM-lite),
see: `docs/autopilot/runbooks/DSE_PLAYBOOK.md`.

Notable Phase D UX that exists today:

- `/autopilot` includes a **DSE Debug** panel that can run a canary recap signature under `direct.v1` or `rlm_lite.v1`,
  and surfaces receipt/trace links (documented in `docs/autopilot/runbooks/DSE_PLAYBOOK.md` §5.1).

---

## 0) Context and Motivation

Autopilot is moving toward Effect as the default application architecture (see `packages/effuse/docs/effect-migration-web.md`, `docs/autopilot/reference/effect-telemetry-service.md`).

Effect solves wiring, reliability, observability, and testability. It does **not** automatically solve the “DSPy problem”:

- prompts and “agent behavior” drift across the codebase
- improvements happen via hand edits
- no disciplined eval loop exists to justify changes
- no artifact registry exists to pin and roll back behavior

`dse` is the missing compiler-layer counterpart for an Effect-first codebase.

---

## 1) Big Decisions (Scope of This Spec)

If we built “DSPy, but Effect TS”, the big decisions are:

1. **Core IR**: what is the canonical representation of “a prompt/program surface” that can be rewritten safely?
2. **Optimizable surface**: what parameters are allowed to change under compilation, and how do we represent them?
3. **Run and evaluate**: how do we execute programs and measure them (datasets, metrics, judges, caching)?
4. **Serialize and ship artifacts**: what are the on-disk/on-wire formats, how do we hash/version them, and how does runtime load them?

This spec answers those four.

**Hard constraint for this repo:** compilation and runtime are **TypeScript-only** and **Effect-native**. We do not depend on a Rust compiler/optimizer for DSE.

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

- a compiler (TypeScript, Effect-native) can optimize without importing app code
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

`dse` SHOULD ship helpers to:

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
- `Telemetry` (logs/spans/events; see `docs/autopilot/reference/effect-telemetry-service.md`)
- `ReceiptRecorder` (canonical hashes, tool calls, timings)
- `WorkersEnv` (Cloudflare bindings/env access, request-scoped when needed)
- `WorkersExecutionContext` (Cloudflare `waitUntil`, `passThroughOnException`, etc.)
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

### 4.3.1 Inference Strategy (Direct vs RLM)

`Predict(signature)` SHOULD be policy-driven, not hardcoded to a single execution strategy.

At minimum, DSE needs:

- `direct.v1`: today's "single LM call" predict (plus optional repair).
- `rlm_lite.v1`: an RLM-style strategy for long contexts (two-bucket context, VarSpace + BlobRefs, action DSL / kernel ops, iteration budgets).

Strategy selection MUST be pinned inside the compiled artifact (or derived deterministically from it) so runs are replayable and auditable. This is the contract that makes "RLM as an inference-time strategy" possible without changing signatures.

See `docs/autopilot/synergies/rlm-synergies.md` and `packages/dse/docs/EFFECT_ONLY_DSE_RLM_GEPA_MIPRO_DESIGN.md`.

### 4.4 Adapter boundary rule (match OpenAgents invariant)

Formatting/parsing is “adapter work”. Validation/retry/timeouts/receipts are “runtime work”.

- Prompt renderer/decoder MUST NOT implement retries.
- Retries/repair loops MUST be explicit `Effect` operators with bounded time.

### 4.5 Cloudflare Workers Entry Points (Fetch, Routers)

Cloudflare Workers has a recurring constraint:

- bindings/env (`env`) and execution context (`ctx`) are available **at request time**

Effect wants Layers “up-front”, so in practice you need a small boundary adapter that:

- builds request-scoped infra from `env`
- provides it via Layers/ManagedRuntime
- runs your Effect program and returns a `Response`

Two patterns that work well:

#### 4.5.1 `@effect/platform` HttpServer router (`toWebHandler`)

This is a clean option when your Worker is primarily an HTTP server:

```ts
import * as Http from "@effect/platform/HttpServer"
import { Effect } from "effect"
import * as S from "@effect/schema/Schema"

const HttpLive = Http.router.empty.pipe(
  Http.router.get("/", Http.response.text("Hello World")),
  Http.router.get(
    "/todo/:id",
    Effect.gen(function* ($) {
      const { id } = yield* $(
        Http.router.schemaPathParams(S.struct({ id: S.NumberFromString }))
      )
      return yield* $(Http.response.text(`Todo ${id}`))
    })
  ),
  Http.router.all("*", Http.response.empty({ status: 404 })),
  Http.router.catchAll((e) => {
    console.log(e)
    return Http.response.empty({ status: 400 })
  }),
  Http.app.toWebHandler
)

export default {
  async fetch(request: Request) {
    return await HttpLive(request)
  }
}
```

Where DSE fits:

- route handlers are Effects, so they can call `Predict(signature)` modules directly
- DSE’s `PolicyRegistry` and `ReceiptRecorder` can be provided as request-scoped services

#### 4.5.2 `effect-cf` Workers bridge (`Workers.serve`)

If you want the thinnest possible adapter, `effect-cf` provides a pragmatic `serve()` that converts:

`(request, env, ctx) => Effect<Response>` into an `ExportedHandler.fetch`.

DSE should provide an equivalent helper that additionally:

- provides `WorkersEnv` / `WorkersExecutionContext`
- installs `ConfigProvider` from `env`
- uses a `ManagedRuntime` so layers can be composed cleanly

### 4.6 Request-Scoped Env and ManagedRuntime (The “Runtime Wrapper”)

Problem statement (Cloudflare constraint vs Effect preference):

- Cloudflare gives bindings at request time.
- Effect dependency graphs want Layers composed once (composition root).

In Worker apps, the pragmatic solution is to build a request-scoped runtime:

- build infra layer from `env` (KV, D1, R2, DO stubs, etc.)
- build app layer by providing that infra
- create a `ManagedRuntime`
- run the Effect program with that runtime

This pattern also maps cleanly onto framework boundaries (React Router loaders/actions, Remix, TanStack Start serverFns/loaders): you return a `Promise`, but internally you run Effects.

Illustrative wrapper (adapted from the pattern you shared):

```ts
import { ConfigProvider, Effect, Layer, ManagedRuntime } from "effect"

export const makeInfraLive = (env: Env) =>
  Layer.mergeAll(
    /* KV.layer(...env.KV...), D1.layer(env.DB), ... */
  )

export const makeFetchRuntime =
  <R, E>(
    makeLiveLayer: (deps: { env: Env; infra: Layer.Layer<any> }) => Layer.Layer<R, E, never>
  ) =>
  <A, E2>(body: (args: { request: Request; env: Env; ctx: ExecutionContext }) => Effect.Effect<A, E2, R>) =>
  async (args: { request: Request; env: Env; ctx: ExecutionContext }): Promise<A> => {
    const infra = makeInfraLive(args.env)
    const live = makeLiveLayer({ env: args.env, infra })
    const runtime = ManagedRuntime.make(live)

    const program = body(args).pipe(
      // Request-scoped config from bindings.
      // Alternatively: Layer.setConfigProvider(ConfigProvider.fromJson(args.env))
      Effect.withConfigProvider(ConfigProvider.fromJson(args.env))
    )

    return runtime.runPromise(program)
  }
```

Tradeoffs and rules:

- Creating a runtime per request is usually fine when your “infra” is just wrappers over CF bindings (KV/D1/etc).
- For heavy resources, avoid re-creating them per request. In Workers you typically:
  - create them once in module scope when possible, or
  - create them once per Durable Object instance (see §4.7), or
  - cache the runtime/layer after the first request (env is stable within a worker instance).

#### Note: `cloudflare:workers` env import

Cloudflare also supports `import { env } from "cloudflare:workers"` (so you can access bindings outside the fetch signature).

This can simplify runtime construction (you can build layers at module scope), but we SHOULD still model env as an injectable service (`WorkersEnv`) for testability and explicitness.

### 4.7 Durable Objects: Long-Lived Runtime + Request Context

Durable Objects change the equation because `env` is available in the constructor, and stateful resources (like DO SQLite) naturally live there.

Recommended pattern:

- build a long-lived `ManagedRuntime` once per DO instance in the constructor
- for each request/message:
  - provide request-scoped context (requestId, route, user/thread ids, etc.)
  - run DSE modules inside the runtime

This fits Autopilot specifically:

- the Autopilot chat thread is already a Durable Object
- the DO is a natural composition root for:
  - artifact registry tables (see §10.3)
  - telemetry sinks
  - request correlation ids
  - DSE `PolicyRegistry` + `ReceiptRecorder`

`effect-cf` includes an `EffectDurableObject` base class that wraps DO storage and returns `Effect<Response>` from `fetch()`. Even if we keep using `AIChatAgent`, the architectural move is the same: the DO holds the runtime, and request handlers run effects inside it.

### 4.8 Cloudflare Service Layers (effect-cf) and Control-Plane API (cloudflare-typescript)

To keep DSE focused, Cloudflare integrations SHOULD live as separate “infra layers” that apps provide, not inside `dse` core.

Recommendations:

- For runtime bindings (KV, D1, R2, Queues, DOs), prefer `effect-cf` patterns (reference: `~/code/effect-cf/`):
  - namespaced modules
  - `make()` + `layer()` + `Tag` + service accessors
  - Schema validation and typed errors at boundaries

- For Cloudflare control-plane APIs (account/zone/workers management), we likely want Effect-wrapped services around the `cloudflare` TypeScript client (repo: `~/code/cloudflare-typescript/`):
  - `CloudflareApi` as a `Context.Tag`
  - each method returns `Effect` via `Effect.tryPromise`
  - map errors into `Schema.TaggedError` with `Schema.Defect` for underlying causes
  - validate important responses with Schema (don’t trust the network boundary)

This gives us the same benefits as DSE itself: typed contracts, retry/timeout policy at the runtime boundary, and traceable receipts.

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

### 5.3.1 Context entries and BlobRefs (token-space hygiene)

`ContextEntry` SHOULD support both:

- small, inline JSON values (safe to render directly)
- **BlobRef-backed** entries for large text (stored outside token space)

Prompt rendering MUST dereference BlobRefs via a BlobStore and enforce strict preview/truncation rules. This is foundational for:

- avoiding context rot by default (don't stuff huge blobs into token space)
- making RLM strategies viable (variable space holds the long context)

Reference implementation in code: `packages/dse/src/promptIr.ts`, `packages/dse/src/runtime/blobStore.ts`, `packages/dse/src/runtime/render.ts`.

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

### 5.6 Compilation-Safe Transforms (What the Compiler May Rewrite)

Because prompt IR is structured, compilation can safely apply transforms without brittle string surgery.

Allowed transforms MUST be explicitly enumerated and bounded. MVP transforms:

- Replace `InstructionBlock` text (choose an instruction variant).
- Select a subset of `FewShotBlock.examples` by id (few-shot selection).
- Tighten `OutputFormatBlock` (e.g., add “JSON only”, forbid extra keys).
- Modify `ToolPolicyBlock` if and only if the signature declares it tool-aware (e.g., restrict allowed tools).
- Insert optional rubric/scoring hints only for explicit judge/eval signatures.

Anything not in the allowlist above is a breaking change and MUST require:

- a prompt IR version bump, or
- an explicit “transform capability” declared by the signature.

### 5.7 Normalization and Hashing

`PromptIR` MUST have a canonical normalized JSON form so we can compute stable hashes:

- `promptIrHash`: `sha256(canonicalJson(normalizePromptIr(promptIr)))`
- `renderedPromptHash` (optional): hash of final provider messages after params application and rendering

Normalization rules (v1):

- preserve block order as authored (blocks are semantically ordered)
- within each object, keys are sorted during canonical JSON serialization (see §9.3)
- remove runtime-only fields (timestamps, request ids, etc.) before hashing
- few-shot examples are referenced by stable ids and/or content hashes, never by “pretty-printed” text

This is the foundation of:

- eval caching keys
- artifact determinism (`compiled_id`)
- reproducible debugging (“what prompt did we run?”)

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

### 6.1.1 Params Schema (JSON Shape, v1)

`DseParams` MUST be fully serializable, and its canonical JSON MUST be included in artifact hashing.

Illustrative v1 shape:

```ts
type DseParamsV1 = {
  readonly paramsVersion: 1

  // Inference strategy selection (pinned by compiled artifact)
  readonly strategy?: {
    readonly id: string // e.g. "direct.v1" | "rlm_lite.v1"
    readonly config?: unknown
  }

  // Instruction selection
  readonly instruction?: {
    readonly variantId?: string
    readonly text?: string // optional literal override; prefer variantId
  }

  // Few-shot selection
  readonly fewShot?: {
    readonly exampleIds: ReadonlyArray<string>
    readonly k?: number
    readonly selector?: { readonly id: string; readonly config?: unknown }
  }

  // Model knobs
  readonly model?: {
    readonly modelId?: string
    readonly temperature?: number
    readonly topP?: number
    readonly maxTokens?: number
  }

  // Role-based model overrides (artifact-pinnable). When present, these override `model` for that role.
  // This is how we keep RLM controller vs sub-LM calls explicit and tunable.
  readonly modelRoles?: {
    readonly main?: DseParamsV1["model"]
    readonly sub?: DseParamsV1["model"]
    readonly repair?: DseParamsV1["model"]
    readonly judge?: DseParamsV1["model"]
  }

  // Decode/repair knobs
  readonly decode?: {
    readonly mode: "strict_json" | "jsonish"
    readonly maxRepairs?: number
    readonly repairStrategy?: "reask_same_model" | "reask_repair_model" | "repair_signature"
    readonly repairModelId?: string
  }

  // Tool policy knobs (only if signature declares tool awareness)
  readonly tools?: {
    readonly allowedToolNames?: ReadonlyArray<string>
    readonly maxToolCalls?: number
    readonly timeoutMsByToolName?: Record<string, number>
  }

  // RLM-lite strategy knobs (artifact-pinnable).
  // These do not change the signature IO contract; they change how the RLM controller operates.
  readonly rlmLite?: {
    readonly controllerInstructions?: string
    readonly extractionSystem?: string
    readonly chunkDefaults?: {
      readonly chunkChars: number
      readonly overlapChars?: number
      readonly maxChunks?: number
    }
    readonly subRole?: "sub" | "main"
  }

  // Execution budgets (artifact-pinnable). These are enforced at runtime and recorded in receipts.
  // RLM-lite fails closed unless `maxRlmIterations` and `maxSubLmCalls` are explicitly pinned.
  readonly budgets?: {
    readonly maxTimeMs?: number
    readonly maxLmCalls?: number
    readonly maxToolCalls?: number
    readonly maxRlmIterations?: number
    readonly maxSubLmCalls?: number
    readonly maxOutputChars?: number
  }
}
```

Notes:

- `paramsVersion` MUST be present to make forward compatibility explicit.
- When both `variantId` and `text` exist, runtime MUST define precedence (recommend: `text` wins).
- Unknown fields MUST be preserved for hashing (don’t drop them silently), but runtime MAY ignore them for forward compatibility.

### 6.1.2 Search Space Representation

Optimizers need a serializable search space definition, e.g.:

- instruction variants list
- few-shot pool reference (ids + hashes)
- which knobs are in scope (model/decode/tools)
- RLM-lite knob variants (controller/chunking/roles/budget profiles)

That search space SHOULD live in the compile job spec (`CompileJobSpecV1.searchSpace`) so:

- compilation is reproducible from artifacts
- we can compute a stable `compileJobHash`

Concrete (implemented) search space fields live in `packages/dse/src/compile/job.ts` and include:

- `strategyVariants` (e.g. `direct.v1` vs `rlm_lite.v1` vs distilled tactics)
- `instructionVariants`
- `fewShot` (pool + kMax)
- `rlmControllerInstructionVariants`
- `rlmChunkingPolicyVariants`
- `rlmSubRoleVariants` (`sub` vs `main`)
- `budgetProfiles`

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

### 7.3.1 Eval Report Schema (Summary)

`EvalReport` SHOULD be serializable so compilation outputs can embed evidence.

Illustrative summary shape:

```ts
type EvalSummaryV1 = {
  readonly evalVersion: 1

  readonly datasetId: string
  readonly metricId: string

  readonly n: number
  readonly meanScore: number
  readonly p50Score?: number
  readonly p95Score?: number

  readonly failures?: {
    readonly decodeFailures?: number
    readonly toolFailures?: number
    readonly otherFailures?: number
  }

  // Optional performance aggregates
  readonly latencyMs?: { readonly p50?: number; readonly p95?: number }
  readonly tokens?: { readonly prompt?: number; readonly completion?: number; readonly total?: number }
}
```

For MVP, storing only `EvalSummaryV1` is sufficient. Later we can optionally store per-example results (careful: size).

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

### 8.5 Compilation Workflow and Engines

Compilation is a workflow that turns:

- a `SignatureContractExportV1`
- a `CompileJobSpecV1` (search space + dataset + metric + optimizer config)

into:

- a `CompiledArtifact` (see §9.4)
- and a registry update (set “active artifact” for a signature)

Recommended workflow:

1. Export signature contracts from the codebase (deterministic JSON).
2. Define compile jobs (datasets + metrics + search spaces).
3. Run optimizers offline (CI, developer machine, or a dedicated compile service).
4. Store artifacts in the registry.
5. Promote artifacts via an explicit “set active” step (promotion gates can live here).
6. Runtime loads active artifacts; receipts include `compiled_id`.

Engine options:

1. **TS-native compilation (the only supported engine)**
   - Compilation runs in TypeScript and is implemented as Effect programs.
   - Start with simple grid search / greedy selection, then expand.
   - Must emit deterministic artifact formats so runtime behavior is pin-able and auditable.

Autopilot-worker constraints strongly suggest compilation is out-of-band; runtime should only *load and execute*.

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

### 9.4 Artifact Schema (JSON, v1)

Define a single, shippable artifact format that runtime can load without code changes.

```ts
type DseCompiledArtifactV1 = {
  readonly format: "openagents.dse.compiled_artifact"
  readonly formatVersion: 1

  readonly signatureId: string
  readonly compiled_id: string // sha256:<hex>
  readonly createdAt: string // ISO 8601

  readonly hashes: {
    readonly inputSchemaHash: string
    readonly outputSchemaHash: string
    readonly promptIrHash: string
    readonly paramsHash: string
  }

  readonly params: unknown // canonicalized DseParamsV1

  readonly eval: unknown // EvalSummaryV1 (at minimum)

  readonly optimizer: {
    readonly id: string // e.g. "instruction_grid.v1"
    readonly config?: unknown
    readonly iterations?: number
  }

  readonly provenance: {
    readonly compilerVersion?: string
    readonly gitSha?: string
    readonly datasetId?: string
    readonly datasetHash?: string
    readonly metricId?: string
    readonly searchSpaceHash?: string
  }

  // Optional dsrs-style compatibility declaration.
  readonly compatibility?: {
    readonly requiredTools?: ReadonlyArray<string>
    readonly requiredLanes?: ReadonlyArray<string>
    readonly privacyModesAllowed?: ReadonlyArray<string>
  }
}
```

Runtime MUST treat artifacts as immutable. A “promotion” changes only the active pointer in the registry.

### 9.5 Artifact Example (Abbreviated)

```json
{
  "format": "openagents.dse.compiled_artifact",
  "formatVersion": 1,
  "signatureId": "@openagents/autopilot/IssueTriage.v1",
  "compiled_id": "sha256:7e2b...c1",
  "createdAt": "2026-02-06T08:30:00Z",
  "hashes": {
    "inputSchemaHash": "sha256:...",
    "outputSchemaHash": "sha256:...",
    "promptIrHash": "sha256:...",
    "paramsHash": "sha256:..."
  },
  "params": {
    "paramsVersion": 1,
    "instruction": { "variantId": "triage_rubric_v3" },
    "fewShot": { "exampleIds": ["ex_001", "ex_014", "ex_102"], "k": 3 },
    "model": { "temperature": 0.2, "maxTokens": 600 },
    "decode": { "mode": "jsonish", "maxRepairs": 1, "repairStrategy": "reask_same_model" }
  },
  "eval": { "evalVersion": 1, "datasetId": "triage.v2", "metricId": "triage_exact.v1", "n": 500, "meanScore": 0.86 },
  "optimizer": { "id": "mvp_joint_search.v1", "iterations": 42 },
  "provenance": { "gitSha": "abc123", "datasetHash": "sha256:..." }
}
```

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

### 10.3 Suggested Durable Object SQLite Layout (Autopilot Worker)

For `apps/autopilot-worker`, the simplest registry is local to the user’s Durable Object (same place transcript + blueprint state live).

Suggested tables:

```sql
create table if not exists dse_artifacts (
  signature_id text not null,
  compiled_id  text not null,
  json         text not null,
  created_at   integer not null,
  primary key (signature_id, compiled_id)
);

create table if not exists dse_active_artifacts (
  signature_id text primary key,
  compiled_id  text not null,
  updated_at   integer not null
);
```

Notes:

- Store full artifact JSON; runtime validates with a schema before use.
- `dse_active_artifacts` is the only mutable “pointer” table; artifacts themselves are immutable rows.

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

### Status (implemented)

As of 2026-02-07, the MVP slice is implemented:

- `packages/dse/` ships Signature, Predict, Eval, Compile, and compiled artifact schemas.
- Autopilot worker uses DSE for Blueprint tool routing:
  - Signature: `@openagents/autopilot/blueprint/SelectTool.v1` in `apps/autopilot-worker/src/dseCatalog.ts`
  - DO SQLite registry + receipts: `apps/autopilot-worker/src/dseServices.ts`
  - Default artifact auto-install (pins `compiled_id`): `apps/autopilot-worker/src/dseServices.ts`, `apps/autopilot-worker/src/server.ts`

---

## 16) Package Layout (Current)

Implemented as `packages/dse/`:

```
packages/dse/
  src/
    compiledArtifact.ts
    hashes.ts
    params.ts
    promptIr.ts
    signature.ts
    runtime/
    eval/
    compile/
    internal/
  test/
```

CLI shape (open decision):

- library-only first (scripts call `compile(...)`)
- or Bun-first CLI + library (`bun run dse compile ...`)

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
5. Compilation runtime (local CLI vs CI job vs dedicated compile service)
6. Workers runtime strategy (per-request runtime vs cached runtime vs per-DO runtime; `env` param vs `cloudflare:workers`)

---

## 19) Next Steps

1. Implement Phase 3 budgets (see `AUTOPILOT_OPTIMIZATION_PLAN.md`) and thread them through:
   - `Predict` (LLM calls + repair loops)
   - tool execution
   - receipts/trace events
2. Add a minimal “graph runner” (Phase 4) so Autopilot multi-step flows can be executed as explicit nodes with per-node receipts.
3. Add a compile runner (script/CLI) that runs `Compile.compile(...)` offline and promotes artifacts into the worker registry via `/dse/artifacts` + `/dse/active`.
4. Add deterministic `TestLM` + golden prompt snapshots to lock down prompt rendering and compilation transforms.
