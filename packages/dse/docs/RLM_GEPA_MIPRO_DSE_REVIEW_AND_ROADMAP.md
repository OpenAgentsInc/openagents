# DSE vs RLM/GEPA/MIPRO: Implementation Review and Roadmap

Generated: 2026-02-08

This doc connects:

- The repo-wide docs index: `packages/dse/docs/RLM_GEPA_MIPRO_SUMMARY.md`
- What is actually implemented today in TypeScript/Effect DSE (`packages/dse/`)
- How that relates to the current **Convex-first Autopilot MVP** execution plane (`apps/web/`) vs the legacy DO-SQLite integration (`apps/autopilot-worker/`)
- What is actually implemented today in the Rust DSPy stack (`crates/dsrs/`, plus `crates/rlm/` + `crates/frlm/`)

If anything here conflicts with code behavior, code wins.

See also: `packages/dse/docs/EFFECT_ONLY_DSE_RLM_GEPA_MIPRO_DESIGN.md`

## MVP Execution Plane Note (Convex-First)

Autopilot MVP is **Convex-first**:

- No per-user Durable Objects / DO-SQLite for chat/user space in the MVP execution plane.
- Cloudflare Worker runs inference and enforces budgets/receipts, while Convex is the canonical state store.

References:

- Decision doc: `docs/autopilot/anon-chat-execution-plane.md`
- DO deprecation shims (410s): `apps/web/src/effuse-host/worker.ts`

`apps/autopilot-worker/` still exists (DO-SQLite + DSE wiring) and is useful as a reference integration, but it is **not** the MVP hot path.

## Quick Status (What We Have Today)

### DSE (TypeScript/Effect)

Implemented (library + tests) and integrated in at least one non-MVP surface (`apps/autopilot-worker/`):

- Typed `Signature` + Prompt IR + deterministic hashes
- `Predict(signature)` with policy resolution, schema decode, bounded repair, and predict receipts
- Execution budgets enforced in DirectPredict (`params.budgets.{maxTimeMs,maxLmCalls,maxOutputChars}`) with budget snapshots recorded in predict receipts (commit `42507656f`)
- `BlobStore` and Prompt IR context entries that can reference content-addressed blobs
- `PolicyRegistry` abstraction with an in-memory implementation (package) and a DO-SQLite implementation (`apps/autopilot-worker/`, legacy/non-MVP)
- Evaluation harness: datasets, metrics (deterministic + judge), reward signals (weighted aggregation), eval cache (in-memory)
- Compile loop (TS/Effect-native), but currently with **MVP optimizers**, not MIPROv2/GEPA

Not implemented yet (in DSE):

- RLM as an inference-time strategy (`RlmPredict`, VarSpace, recursion, sub-LM budgeting)
- GEPA optimizer (reflective evolution + Pareto frontier over per-example wins)
- MIPROv2 optimizer (multi-stage instruction proposal + demo bootstrapping + structured search)
- Runtime enforcement for several constraints declared in signatures/params (tool-call budgets, per-tool timeouts, signature timeoutMs, etc.)

### Rust DSPy stack (dsrs/rlm/frlm)

Implemented (separate from DSE):

- `crates/dsrs/` implements optimizers including **MIPROv2** and **GEPA**.
- `crates/rlm/` + `crates/frlm/` implement RLM/FRLM concepts and supporting signatures/tools.

Important constraint for `apps/web` + `packages/dse`: we should assume we **cannot** directly reuse Rust crate implementations inside the Effect/Workers runtime. Any production plan for DSE must be **TypeScript/Effect-native** (or behind a network boundary as a separate service).

## Where DSE Is Implemented (Code Map)

Core contracts:

- Signature: `packages/dse/src/signature.ts`
- Prompt IR: `packages/dse/src/promptIr.ts`
- Tool contracts: `packages/dse/src/tool.ts`
- Signature contract export: `packages/dse/src/signatureContract.ts`
- Deterministic hashing (schema/prompt/params/rendered): `packages/dse/src/hashes.ts`
- Canonical JSON (stable hashing input): `packages/dse/src/internal/canonicalJson.ts`

Runtime (Predict):

- LM client interface: `packages/dse/src/runtime/lm.ts`
- Policy registry interface + in-memory backend: `packages/dse/src/runtime/policyRegistry.ts`
- Blob store interface + in-memory backend: `packages/dse/src/runtime/blobStore.ts`
- Prompt rendering (params applied to IR, blob context loading, tool policy block): `packages/dse/src/runtime/render.ts`
- Output decode + bounded repair: `packages/dse/src/runtime/decode.ts`
- Predict implementation + predict receipts: `packages/dse/src/runtime/predict.ts`, `packages/dse/src/runtime/receipt.ts`

Artifacts + evaluation + compile:

- Compiled artifact schema: `packages/dse/src/compiledArtifact.ts`
- Dataset + hashing/sampling: `packages/dse/src/eval/dataset.ts`
- Metrics (deterministic + judge): `packages/dse/src/eval/metric.ts`
- Reward signals + aggregation: `packages/dse/src/eval/reward.ts`
- Evaluation runner (per-example + summary + cache keys): `packages/dse/src/eval/evaluate.ts`, `packages/dse/src/eval/cache.ts`
- Compile job spec + hashing: `packages/dse/src/compile/job.ts`
- Compile loop (current optimizers): `packages/dse/src/compile/compile.ts`

Legacy Autopilot Worker integration (Durable Object SQLite):

- DO tables + service layers (PolicyRegistry + BlobStore + ReceiptRecorder): `apps/autopilot-worker/src/dseServices.ts`
- In-repo signature catalog (Bootstrap + Blueprint tool selection signatures): `apps/autopilot-worker/src/dseCatalog.ts`
- Tool contract to `@effect/ai` tool conversion: `apps/autopilot-worker/src/effect/ai/toolkit.ts`
- End-to-end tests covering DSE introspection + artifact store/promote/rollback: `apps/autopilot-worker/tests/index.test.ts`

Autopilot MVP (`apps/web`, Convex-first):

- Execution plane: `apps/web/src/effuse-host/autopilot.ts` + `apps/web/convex/autopilot/*` (today this uses `@effect/ai` directly; DSE runtime integration is future work).
- Contract endpoints for UI introspection (exports DSE catalogs): `apps/web/src/effuse-host/contracts.ts`

## Where MIPRO/GEPA/RLM Are Implemented Today (Rust Reference Only)

- MIPROv2 optimizer: `crates/dsrs/src/optimizer/mipro.rs`
- GEPA optimizer: `crates/dsrs/src/optimizer/gepa.rs`
- Pareto frontier helpers (used by GEPA): `crates/dsrs/src/optimizer/pareto.rs`
- RLM crate (recursive execution engine + tools): `crates/rlm/`
- FRLM crate (federated conductor + signatures): `crates/frlm/`

These are useful for reading/understanding the intended semantics, but DSE’s implementation plan should not require importing or executing them in `apps/web`.

## Comparison Against The Existing Roadmap Docs

Relevant “plans and patterns” docs:

- DSE spec: `docs/autopilot/dse.md`
- DSE implementation roadmap: `docs/autopilot/AUTOPILOT_OPTIMIZATION_PLAN.md`
- Horizons patterns (mipro_v2 loop, RLM-style reward signals, budgets): `docs/autopilot/horizons-synergies.md`
- Microcode patterns (artifact pinning posture, blob side channel, multi-model roles): `docs/autopilot/microcode-synergies.md`
- RLM integration plan (two-bucket context + RLM-lite action DSL): `docs/autopilot/rlm-synergies.md`

What matches the roadmap already:

- Phase 0 “production spine” is implemented (Predict + PolicyRegistry + BlobStore + receipts + artifact format).
- Phase 1 eval harness is implemented (dataset + metrics + reward signals + caching + eval summary).
- Phase 2 compile loop is implemented, shaped like `mipro_v2` (candidate generation + evaluation + pick best), but the optimizer set is still minimal.

What is still roadmap-only:

- Phase 2.5 RLM inference strategy (swappable execution mode, two-bucket context, recursion, sub-model role, iteration receipts).
- Any real MIPROv2/GEPA implementation in TS DSE (beyond today’s grid/greedy MVP optimizers).
- Phase 3 budgets enforcement that matches the docs (tool calls, RLM iteration/sub-LM budgets, etc.). DirectPredict time/LM-call/output budgets are implemented (commit `42507656f`).

## Gap Analysis (DSE vs “Real” MIPROv2 / GEPA / RLM)

### MIPROv2 gap

What we have:

- `instruction_grid.v1`: evaluates a fixed set of instruction variants provided in `searchSpace`.
- `fewshot_greedy_forward.v1`: greedy adds few-shot example ids from a candidate pool.
- `joint_instruction_grid_then_fewshot_greedy_forward.v1`: runs the two above in sequence.

What’s missing vs MIPROv2 (as described in docs and implemented in `crates/dsrs/`):

- A meta-optimizer that *proposes* instruction candidates from traces (not only pre-enumerated variants).
- Demo bootstrapping from successful traces, beyond selecting from an existing authored pool.
- A structured search/scheduling policy (e.g. BO/SH-style exploration) rather than grid+greedy.

### GEPA gap

What we have:

- Per-example reward signals and a scalar reward.
- Judge-backed metrics that can be pinned to a specific judge artifact.

What’s missing:

- Pareto frontier tracking of “wins on examples” (diversity preservation).
- Reflection/mutation loop driven by text feedback (and usually trace information) to generate new candidates.
- A feedback evaluator API surface (today’s signals can emit `notes`, but we don’t store or structure “why it failed” well enough to drive reflection consistently).

### RLM gap

What we have:

- `BlobStore` and Prompt IR context entries that reference blobs.
- Prompt rendering loads blob text and truncates it into token space (bounded, but still “stuff into prompt”).

What’s missing:

- Two-bucket context execution: keep large context in var space and load only previews/slices into token space on demand.
- A swappable inference strategy (e.g. `DirectPredict` vs `RlmPredict`) that works for existing signatures.
- A deterministic, replayable “action kernel” (RLM-lite DSL) and iteration-level receipts.
- A `sub` model role (for recursion) and explicit budgets: `maxIterations`, `maxSubLmCalls`.

## Suggested Roadmap To Get RLM/GEPA/MIPRO Into DSE (TS/Effect)

The goal here is to extend the existing DSE spine, not rewrite it.

### 1) Add explicit “inference strategy” plumbing (RLM prerequisite)

Change scope:

- Introduce a strategy interface around prediction so we can plug in RLM without changing signatures.
- Keep the stable unit as `DseSignature<I, O>`.

Suggested shape:

- Add a `PredictStrategy` abstraction in `packages/dse/src/runtime/`.
- Implement `DirectPredict` using today’s `Predict.make` behavior.
- Implement `RlmPredict` as a new strategy.

### 2) Implement RLM-lite for DSE (Phase 2.5 from the roadmap docs)

Minimum viable DSE RLM should follow `docs/autopilot/rlm-synergies.md`:

- Add a `VarSpace` service (Convex-backed for the MVP execution plane; optional DO-backed later) that stores named variables pointing to `BlobRef`s (large inputs) and derived small JSON/text values.
- Add an RLM kernel with a *structured action DSL* (no arbitrary code at first).
- Action: `preview(blob, start, end)`.
- Action: `search(blob, query)` (regex/keyword).
- Action: `chunk(blob, size, stride)`.
- Action: `sub_lm(prompt, inputs...)` (role-based, budgeted).
- Action (optional): `tool_call(name, args)` wired through the existing tool contract surface.
- Add budgets and record them in iteration receipts: `maxIterations`, `maxSubLmCalls`, and (if tool actions are enabled) `maxToolCalls`.

Where this likely lands:

- DSE core: new runtime modules and receipt formats in `packages/dse/src/runtime/`.
- MVP Autopilot (`apps/web`): Convex tables/services for VarSpace + trace/iteration receipts (see `docs/autopilot/anon-chat-execution-plane.md` for the “Convex-first” posture).

### 3) Promote “MIPROv2-like” optimizers in DSE compile

Treat the existing compile loop as the scheduler (already shaped correctly) and add new optimizer ids:

- Extend `packages/dse/src/compile/job.ts` optimizer enum beyond grid/greedy.
- Add a meta-signature (in DSE catalog or a dedicated compiler module) that proposes instruction candidates.
- Meta-signature input: small summary of failures/successes from eval results.
- Meta-signature output: `[{ id, text }]` (strict JSON).
- Implement “propose → evaluate → select → repeat” with early stopping.
- Record: candidate list (or candidate hashes), eval summaries, selection policy config.

Pragmatic first step:

- Implement a “proposal” optimizer that generates `N` instruction variants via the LM and then runs the existing evaluation pipeline to pick best.
- Treat this as `mipro_like_instruction_proposal.v1` and avoid naming it “MIPROv2” until it actually matches the intended algorithmic contract.

### 4) Implement GEPA in DSE as a first-class optimizer

GEPA requires two additional ingredients that DSE does not yet make first-class:

- Per-example dominance tracking (Pareto frontier over example-level wins).
- Reflection feedback that is structured enough to mutate prompts deterministically.

Suggested implementation path:

- Add a `ParetoFrontier` module in DSE compile (port the core logic from `crates/dsrs/src/optimizer/pareto.rs` conceptually).
- Extend eval/example results (optionally) with bounded “why” fields.
- Why field: decode failure summaries.
- Why field: judge notes.
- Why field: tool failure counts.
- Why field (optional): bounded rendered-prompt hash and output hash already exist.
- Add a meta-optimizer loop.
- Step: sample a parent from the frontier.
- Step: ask an LM (via a pinned “reflect + propose” signature) to propose a mutated instruction.
- Step: evaluate the new candidate per-example.
- Step: insert into the frontier if it wins anywhere.
- Step: stop by budget.

### 5) Budgets and determinism (needed for RLM and for safe optimizers)

This is Phase 3 from `docs/autopilot/AUTOPILOT_OPTIMIZATION_PLAN.md` and should be treated as an enabler:

- Enforce timeouts and call budgets in `Predict` (and later `RlmPredict`) using signature constraints and params.
- Record budget consumption and early-stop reasons in receipts.

### 6) Do Not Depend On Rust For DSE (apps/web), Use It Only As Reading Material

Even though DSE compilation/runtime must be TS-only, we can optionally use Rust implementations as:

- Reading material for optimizer semantics (MIPROv2/GEPA) and RLM/FRLM concepts.
- A source of invariants we can encode in TS tests (without calling Rust code).

Concretely:

- Mirror a small subset of dsrs unit tests at the “contract level” in `packages/dse/test/` (fake LM, deterministic inputs), but keep the test logic entirely in TS.
- Keep optimizer ids/versioning explicit so we can evolve without breaking artifacts.

## Key DSE Implementation Commits (For Archaeology)

Recent DSE implementation work is concentrated in these commits (from `git log -- packages/dse`):

- `a3cddefef2bc902f9f3e61336b06c262f23d5c8c`: scaffold Effect-native signatures and predict
- `55f3fc2e0056ca99791228eb86f9511b0e355deb`: Phase 0 production spine
- `3e3d8065aeb104f4d4d4beb77f71ec458e82a9b5`: Phase 1 evaluation harness
- `1807d3e2b7d51b1101354bba70163de0661e3fa0`: Phase 2 compiler loop
- `fb1138c019892262c7fb2361001bbf1129db5ffb`: Autopilot DSE MVP slice + pin default artifact
