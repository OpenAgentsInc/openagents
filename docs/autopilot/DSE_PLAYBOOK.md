# DSE Playbook (How To Use DSE + RLM-lite Today)

- **Status:** Draft (operational guide; matches current implementation)
- **Last updated:** 2026-02-10
- **Spec / intent:** `docs/autopilot/dse.md`
- **End-to-end roadmap:** `docs/autopilot/RLM_UNIFIED_ROADMAP.md`
- **If this doc conflicts with code behavior:** code wins

This is the plain-language guide for using what we have implemented now:

- DSE `Predict(signature)` with artifact-pinned strategies (`direct.v1`, `rlm_lite.v1`, distilled tactics),
- receipts + replayable traces (including RLM iteration traces),
- a trace -> dataset export path,
- compile -> promote -> canary operator loops,
- and Phase H hardening (provenance-first observations + reduced prompt injection surface).

## 1) Mental Model

DSE is the compiler/runtime layer for "agent behavior" in an Effect-first codebase.

You define behavior as a **Signature**:

- stable `signatureId` like `@openagents/<domain>/<Name>.vN`
- typed input/output schemas (Effect Schema)
- a structured prompt IR
- default params (strategy, decode policy, budgets, tool policy)

At runtime, `Predict(signature)`:

1. looks up the **active compiled artifact** (optional),
2. executes the signature using the artifact params (or signature defaults),
3. emits a **predict receipt** (hashes, timing, budgets, strategy, context pressure),
4. and for RLM-lite, emits an **rlmTrace** blob containing per-iteration actions and observations.

Over time, operators use traces + examples to compile better artifacts and roll them out safely.

## 2) Key Objects You Will See

- `params.strategy.id`
  - `"direct.v1"`: single LLM call (+ optional repair).
  - `"rlm_lite.v1"`: bounded controller loop executing a structured action DSL against variable-space context.
  - `"distilled.search_line_extract.v1"`: a deterministic long-context "needle" tactic with fallback.
- **Budgets** (`params.budgets`)
  - `maxTimeMs`, `maxLmCalls`, `maxToolCalls`, `maxOutputChars`
  - RLM-specific: `maxRlmIterations`, `maxSubLmCalls`
  - RLM-lite fails closed if `maxRlmIterations`/`maxSubLmCalls` are missing.
- **BlobStore** + `BlobRef`
  - Large text lives outside token space and is referenced by blob handles.
- **VarSpace**
  - Per-run key/value store used by RLM-lite.
  - Values are small JSON or `BlobRef` handles.
- **Predict receipt** (`openagents.dse.predict_receipt`)
  - stable hashes (`paramsHash`, schema hashes, prompt IR hash, rendered prompt hash when available)
  - `strategyId`, budgets (limits + usage), context pressure
  - `rlmTrace` handle when RLM-lite ran
- **RLM trace** (`openagents.dse.rlm_trace`)
  - deterministic JSON blob with events: input, actions, observations, final output.

## 3) What RLM-lite Actually Does

RLM-lite exists to avoid context rot by keeping long context in variable space.

- Token space: bounded messages + bounded observations.
- Variable space: blobs, chunk lists, extracted JSON, sub-model outputs as BlobRefs.

### 3.1 Kernel-driven (symbolic) recursion

The controller does not emit O(N) subcalls for N chunks.
Instead, it can use `ExtractOverChunks`, where the kernel:

1. iterates over chunk BlobRefs
2. calls a sub-model per chunk (budgeted)
3. writes outputs into VarSpace

This is the "symbolic recursion" pattern (code drives traversal; the model extracts/synthesizes).

### 3.2 Actions available (current)

The controller emits one JSON action per iteration:

- `Preview` (bounded excerpt)
- `Search` (bounded match snippets)
- `Load` (load a blob into VarSpace)
- `Chunk` (split a target into chunk blobs)
- `WriteVar` (write small JSON or a BlobRef)
- `ExtractOverChunks` (kernel-driven fanout)
- `SubLm` (role-based sub-model call; output stored as a blob handle)
- `ToolCall` (optional; allowlisted and budgeted)
- `Final` (schema-decoded output)

## 4) Phase H: Poisoning / Confusion Hardening (What Changed)

RLM does not "solve poisoning". Phase H makes the RLM-lite path more auditable and reduces prompt-injection surface:

1. **Provenance-first observations**
   - `PreviewResult` and `SearchResult` include a SpanRef-like `span` object:
     - `source` (which blob/var the excerpt came from)
     - `startChar`/`endChar` offsets and `totalChars`
     - optional `startLine`/`endLine` when cheap to compute
2. **Trust + origin labeling**
   - observations include `trust` (currently conservative: everything is treated as `untrusted`)
   - observations include `origin` hints (e.g. `tool`, `lm`, `unknown`)
3. **Stop re-injecting untrusted text**
   - RLM controller state no longer includes raw observation text/snippets, only sizes + spans.
   - the controller still sees the observation once (bounded), but it is not duplicated every iteration.
4. **Verification posture**
   - tool calls are recorded and labeled in observations without inlining tool output.

Interpretation: a "fact" is not trusted because the controller said it. It is trusted because we can point to a specific span/tool receipt.

## 5) Using This In Autopilot (End-User UX)

Autopilot uses DSE under the hood for certain steps (not everything is DSE yet).

Today, the most visible use is long-context summarization:

1. Ensure the environment supports the required model capability (RLM controller quality matters).
   - RLM-lite triggering is gated in Autopilot when `OPENROUTER_API_KEY` is not configured.
2. In `/autopilot`, request a recap of older messages (or use `/rlm` if supported in the UI).
3. Autopilot will run an RLM-lite DSE signature to summarize older history and inject a bounded summary into the main prompt.
4. In the chat UI, look for a `dse.signature` card:
   - it should show `signatureId`, `compiled_id`, `strategyId`, budgets/usage, and `rlmTrace`.

If something goes wrong, the receipt + trace should answer:

- did we run `direct` or `rlm_lite`?
- what budgets were hit?
- what blobs/spans were accessed?
- how many iterations/subcalls were used?

## 6) Operator Workflow: Trace -> Example -> Compile -> Promote/Canary

This is the core "self-improve" loop.

Prereqs:

- You must be authenticated (these endpoints rely on the browser session cookie).
- Some endpoints require model access (`env.AI` binding and `OPENROUTER_API_KEY`) because they run eval/compile.

### 6.1 Export a labeled example from an RLM trace

Goal: turn a good RLM run into a candidate dataset row (`dseExamples`).

- Find a `receiptId` for a run that includes `rlmTrace`.
  - In Autopilot, the `dse.signature` card is the easiest place to copy `receiptId` and see whether `rlmTrace` exists.
- Call the admin endpoint (auth required):

```js
await fetch("/api/dse/trace/export", {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({
    receiptId: "<PASTE_RECEIPT_ID>",
    split: "holdout",
    tags: ["rlm", "trace_export"]
  })
}).then(r => r.json())
```

This derives:

- `inputJson` from the trace `Input` event
- `expectedJson` from the trace `Final.output`

and upserts it into Convex `dseExamples`.

### 6.2 Compile an artifact for a signature

Compile is "run evaluation loops and choose better params".

Current worker endpoint (auth required):

```js
await fetch("/api/dse/compile", {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ signatureId: "<SIGNATURE_ID>" })
}).then(r => r.json())
```

Notes:

- `/api/dse/compile` requires model access because it runs an eval loop:
  - `env.AI` must be bound for the Worker
  - `OPENROUTER_API_KEY` must be set

This creates (or reuses) a compile report keyed by `(signatureId, jobHash, datasetHash)` and stores:

- a compiled artifact (`compiled_id`)
- a compile report (including holdout reward)

Note: the compile engine supports richer search spaces and optimizers (including strategy + RLM knob selection),
but the current `/api/dse/compile` endpoint is intentionally minimal. See Phase G in:

- `docs/autopilot/RLM_UNIFIED_ROADMAP.md`
- `packages/dse/docs/EFFECT_ONLY_DSE_RLM_GEPA_MIPRO_DESIGN.md`

### 6.3 Promote a compiled artifact

Promote makes a compiled artifact active for that signature (runtime will start using it).

```js
await fetch("/api/dse/promote", {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({
    signatureId: "<SIGNATURE_ID>",
    compiled_id: "<COMPILED_ID>",
    minHoldoutDelta: 0.05,
    requireHoldout: true
  })
}).then(r => r.json())
```

### 6.4 Start a canary rollout

Canary runs a candidate artifact for a percentage of traffic without changing signature code.

```js
await fetch("/api/dse/canary/start", {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({
    signatureId: "<SIGNATURE_ID>",
    canary_compiled_id: "<COMPILED_ID>",
    rolloutPct: 10,
    minHoldoutDelta: 0.05,
    requireHoldout: true,
    minSamples: 50,
    maxErrorRate: 0.2,
    reason: "try rlm_lite strategy for high-context cases"
  })
}).then(r => r.json())
```

Notes:

- Canary start also requires model access (`env.AI` + `OPENROUTER_API_KEY`) because it re-evaluates the control artifact on holdout to compute the baseline reward.

Stop it:

```js
await fetch("/api/dse/canary/stop", {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ signatureId: "<SIGNATURE_ID>", reason: "rollback" })
}).then(r => r.json())
```

## 7) Developer Workflow: When To Use direct vs RLM-lite

Use `direct.v1` when:

- context pressure is low
- the task is simple and the evidence fits in token space
- you want the lowest latency path

Use `rlm_lite.v1` when:

- context pressure is high (large logs, repo snapshots, long thread history)
- you need explicit evidence extraction with bounded token space
- you want per-iteration traces and stable budgets

Use distilled strategies when:

- you have a repeated workload pattern (e.g. "needle in haystack" line extraction)
- you want deterministic fast paths with RLM fallback for novelty

## 8) Where To Look Next

- Unified long-context roadmap: `docs/autopilot/RLM_UNIFIED_ROADMAP.md`
- Context failure definitions: `docs/autopilot/context-failures.md`
- Trace mining workflow: `docs/autopilot/rlm-trace-mining.md`
- DSE spec (deep details): `docs/autopilot/dse.md`
- RLM design notes + constraints: `docs/autopilot/rlm-synergies.md`
