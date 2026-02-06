# DSE / ds-effect: Declarative Self-Improving Effect (Spec)

Date: 2026-02-06  
Status: Draft / exploratory

This doc proposes a TypeScript package, `packages/ds-effect/`, that brings the **DSPy / DSRs** model (Signatures, Modules, Optimizers, Artifacts) into the **Effect** ecosystem, so `apps/web` and `apps/autopilot-worker` can be built as *typed Effect programs* while still being eligible for **self-improvement** (policy compilation) over time.

The intent is architectural clarity, not an immediate implementation.

---

## Why This Exists

We are migrating `apps/web` toward Effect (see `docs/autopilot/effect-migration-web.md`). As we do that, we risk re-creating the same failure mode DSPy was built to avoid:

- logic encoded as ad hoc prompts spread across the codebase
- improvements applied by “hand tweaking prompts inline”
- little ability to measure, optimize, and promote better variants

DSPy (Python) and DSRs/dsrs (Rust) solve this with a compiler model:

- **Signatures** define *what* a step must do (typed IO + instruction)
- **Modules** compose steps
- **Optimizers** compile modules against metrics into **policy bundles**

`ds-effect` is a way to get those same moves in TypeScript, using Effect primitives (Schema, Layer, ManagedRuntime, spans/log annotations) instead of globals.

---

## Core Idea (One Sentence)

Write agent behavior as **Effect programs over typed services**, but make the *LLM-facing parts* declarative and parameterized (Signatures), so we can **compile** improved instructions/demos/config into policy bundles without rewriting orchestration code.

---

## Where It Lives

`packages/ds-effect/` (TypeScript) is intended to be used by:

- `apps/web` (TanStack Start): loaders, serverFns, boundary adapters, telemetry
- `apps/autopilot-worker` (Cloudflare Worker): prompt+tool orchestration, blueprint state transitions

It should be Worker-compatible (no Node-only APIs in the runtime path).

---

## Relationship to DSPy (~/code/dspy)

DSPy’s key abstractions map cleanly onto Effect:

| DSPy (Python) | What it is | DSE / ds-effect (Effect TS) |
| --- | --- | --- |
| `Signature` (`dspy/signatures/signature.py`) | Typed IO + instruction + field metadata | `DseSignature` as a value: `{ id, instruction, input: Schema, output: Schema, fieldMeta }` |
| `Predict` (`dspy/predict/predict.py`) | A module that formats + calls the LM + parses to fields | `DsePredict` = `Input -> Effect<Env, PredictError, Output>` |
| `Module` (`dspy/primitives/module.py`) | Composable program unit with callbacks/history | `DseModule` = composable Effect programs with explicit parameter registration + tracing spans |
| `Teleprompter.compile(...)` (`dspy/teleprompt/*`) | Optimizer that mutates student program prompts/demos | `DseOptimizer.compile(...)` producing a policy bundle (ideally immutable + versioned) |
| `dspy.settings` | Global config for LM/adapter/trace | Effect `Layer` + `ManagedRuntime` (no global mutation) |

Design implication: in DSE we should **avoid “global mutable settings”** and instead treat LM/config/telemetry/policy as services in the environment (like `apps/web/src/effect/*` already does).

---

## Relationship to DSRs / dsrs (~/code/dsrs and crates/dsrs)

DSRs/dsrs formalizes the compiler layer in Rust:

- Signatures are explicit typed contracts (macros + IR)
- Modules expose a parameter tree (`Optimizable.parameters()`) so optimizers can update nested predictors
- Compilers emit a `CompiledModuleManifest` and trace spans (see `crates/dsrs/docs/COMPILER-CONTRACT.md`)

`ds-effect` should align with dsrs, not fork the world.

### Architectural Positioning

There are two viable ways to relate `ds-effect` to dsrs:

1. **Runtime-in-TS, compile-in-Rust (recommended first)**
   - `ds-effect` defines signatures and runs predictors in TS (Effect).
   - Optimization happens out-of-process using dsrs (Rust) as the compiler/optimizer engine.
   - Output is a **policy bundle** that TS loads at runtime.

2. **Runtime-in-TS, compile-in-TS (later, optional)**
   - Re-implement a subset of optimizers (e.g., random search / simple instruction proposer) in TS.
   - Still keep artifact formats compatible with dsrs so promotion/eval pipelines remain shared.

Recommendation: start with (1) so we reuse dsrs optimizers (MIPROv2/GEPA/COPRO) and evaluation machinery, then consider (2) only for “fast local iteration” workflows.

### Shared Artifact Contract (make it portable)

To interoperate, `ds-effect` policy output should be compatible with dsrs manifests:

- `compiled_id`: deterministic hash of canonical policy JSON
- `optimizer`: `"MIPROv2" | "GEPA" | "COPRO" | ...`
- `trainset_id`: hash of training set / labeled receipts
- `scorecard`: proxy/truth scores, iterations, cost
- `instruction`: optimized instruction text
- `demo_count` + serialized demos (few-shot examples)

This should be stored in a “policy bundle store” (DB or KV) and loaded by the TS runtime per signature id.

### Trace Compatibility

Effect already supports spans and structured logging. If we annotate spans with dsrs-compatible attributes, TS executions can participate in the same observability story:

- `signature_name`, `compiled_id`
- LM model, token usage, latency
- tool calls (when present) as child spans/events

Even if we never build a full “DAG trace” in TS, a span tree is enough to unify debugging and to generate replay/training data later.

---

## Proposed ds-effect Concepts

### 1) Signature as a value (Schema-first)

A signature is not a class; it’s a value that can be hashed, versioned, and converted to JSON schema.

Minimum fields:

- `id`: stable identifier (include version, e.g. `"@openagents/autopilot/UserHandle.v1"`)
- `instruction`: default instruction text
- `input`: `Schema.Schema<Input>`
- `output`: `Schema.Schema<Output>`
- `fieldMeta`: descriptions/prefix/format constraints (if needed for prompt formatting)

This mirrors:

- DSPy’s Pydantic field metadata (`InputField/OutputField`)
- dsrs’s `FieldSpec` / IR approach

### 2) Policy is separate from code

We should treat “what the optimizer changes” as data:

- instruction override
- demos (few-shot examples)
- LM config knobs (temperature, max tokens, etc.)
- optional tool affordances

So the running program is: `(SignatureDef, Policy) -> Prompt -> LM -> Output`.

This is the critical decoupling that makes cloud deployment sane: improve the policy without redeploying worker code.

### 3) Parameter tree must be explicit

Self-improvement requires the optimizer to find and update “parameters” inside composed modules.

DSPy uses runtime reflection (`Module.named_predictors()`); dsrs uses an explicit `Optimizable.parameters()` tree.

In TS we should prefer explicitness:

- `DseModule` exposes `parameters(): Record<string, DseOptimizable>`
- predictors are “optimizable leaves”

This avoids fragile TS reflection and makes it obvious which parts are learnable.

### 4) Adapter vs runtime boundary (same rule as dsrs)

Keep the same invariant as OpenAgents tooling:

- adapters do formatting/parsing only
- retries/timeouts/guardrails are runtime policy (Effect operators)

This keeps behavior auditable and avoids hidden “magic” inside adapters.

---

## What ds-effect Might Look Like (Sketch)

This is illustrative only (don’t treat as API commitment).

```ts
import { Effect, Layer, Schema } from "effect"

// Signature definition (value-level)
const UserHandle = Dse.Signature.make({
  id: "@openagents/autopilot/UserHandle.v1",
  instruction: "Extract the user's preferred handle. Do not ask for PII.",
  input: Schema.Struct({ message: Schema.String }),
  output: Schema.Struct({ handle: Schema.String }),
})

// Predictor (Effect program)
const predictUserHandle = Dse.Predict.make(UserHandle).pipe(
  Effect.withSpan("sig:UserHandle"),
)

// Module composition
const program = Effect.gen(function* () {
  const out = yield* predictUserHandle({ message: "Call me Chris." })
  return out.handle
})

// Runtime wiring (LM + policy store + telemetry)
const AppLive = Layer.mergeAll(LmLive, PolicyStoreLive, TelemetryLive)
ManagedRuntime.make(AppLive).runPromise(program)
```

Key properties:

- Schema is the source of truth for types + runtime validation
- the predictor reads policy for the signature id (defaults if absent)
- spans/log annotations include signature id + compiled policy id

---

## How It Fits Autopilot Specifically

`apps/autopilot-worker` today is a single monolithic system prompt + tool surface.

DSE lets us break “prompt-only logic” into stable signatures that can be optimized independently, for example:

- bootstrap stages:
  - `BootstrapSetUserHandleSignature`
  - `BootstrapSetAgentNameSignature`
  - `BootstrapSetAgentVibeSignature`
- tool-call repair / selection:
  - `ToolRepairSignature` (given failing tool call + tool schemas -> repaired tool call)
- response style enforcement:
  - `UserVisibleResponseSignature` (given internal state + tool results -> final user text)

These are all places where “we improve prompts over time” is valuable, but we want the orchestration to remain stable and testable.

---

## Implementation Plan (Phased)

1. **Phase 0: Policy-driven execution (no optimizer)**
   - `Signature` value type + Schema->JSON schema helpers
   - `Predict` that:
     - builds prompts from (SignatureDef + Policy)
     - parses outputs robustly into Schema
     - emits spans/log annotations
   - `PolicyStore` service (in-memory + DO/SQLite backing for worker)

2. **Phase 1: dsrs-backed compilation**
   - Define a shared “signature export” JSON:
     - id, instruction, JSON schema (input/output), demo encoding
   - Add a compiler endpoint / job that runs dsrs optimizers against training data
   - Produce `CompiledModuleManifest` + policy bundle JSON that TS can load

3. **Phase 2: Promotion gates + evaluation**
   - Adopt the same concepts as dsrs:
     - proxy metrics vs truth metrics
     - promotion gate (only promote if better and safe)
     - rollback story (policy bundle version pinning)

4. **Phase 3: Optional TS-native optimizers**
   - Only if needed for “quick local iteration” loops.

---

## Hard Problems / Open Questions

- **Robust parsing**: Effect Schema validates *after* parsing, but we still need a tolerant “jsonish” parser for LLM output. DSRs is exploring BAML’s parser (`~/code/dsrs/CURRENT_SPEC.md`). Do we:
  - adopt a JS tolerant parser (jsonrepair/json5/markdown-fence stripping), or
  - reuse BAML via WASM, or
  - enforce stricter model-side structured output where available?

- **Canonical hashing**: dsrs hashes canonical JSON for ids. We need the same canonicalization rules in TS for `compiled_id` stability.

- **Tracing semantics**: do we need a DAG like dsrs, or are spans sufficient for Autopilot needs?

- **Worker constraints**: Cloudflare Worker limits may constrain “compiler-in-worker” fantasies; assume compilation is out-of-band.

- **Tool schemas**: AI SDK expects JSON schema; we should generate it from Effect Schema to avoid drift.

---

## Next Steps (Concrete)

- Create `docs/autopilot/ds-effect.md` (this file) as the architectural intent.
- If we implement:
  - start with `Phase 0` in `packages/ds-effect/`
  - wire one tiny signature end-to-end in `apps/autopilot-worker` (e.g., “user-visible response”) to validate the shape
  - only then decide whether compilation lives in Rust (dsrs) or TS

