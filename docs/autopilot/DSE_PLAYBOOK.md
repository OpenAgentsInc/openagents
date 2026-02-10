# DSE Playbook (How To Use DSE + RLM-lite Today)

- **Status:** Draft (operational guide; matches current implementation)
- **Last updated:** 2026-02-10
- **Spec / intent:** `docs/autopilot/dse.md`
- **End-to-end roadmap:** `docs/autopilot/RLM_UNIFIED_ROADMAP.md`
- **If this doc conflicts with code behavior:** code wins

## What this is

DSE is the part of Autopilot that turns “agent behavior” from a pile of prompts into something you can **define, measure, and ship safely**. Instead of hand-editing strings and hoping things get better, you describe each behavior as a **Signature**: a named, versioned step with a clear input and output shape. When Autopilot runs a Signature, it produces the same kind of artifact you expect from real software: receipts, budgets, and traceable evidence of what happened.

What this enables is a disciplined improvement loop. You can run a Signature on a dataset, score it with a metric, and then “compile” a better version by choosing better instructions, better examples, or a better strategy. The result is a **compiled artifact** with a stable ID that you can pin, roll out, canary, and roll back—so improvements are **auditable** and **reversible**, not mystery changes that drift over time.

DSE also makes long-context work safer. When conversations, logs, or repos get too big, quality can quietly collapse (“context rot”). DSE supports an alternate strategy called **RLM-lite** that keeps large context out of the model’s token window and instead works in a bounded loop with strict budgets and explicit evidence handles. In the UI, you’ll be able to see when this happened, why it triggered, what it looked at, and what limits it hit—so “agent work” stays debuggable and trustworthy.
This playbook explains how to use what’s implemented today: how to run Signatures, read receipts and traces, export examples from real runs, compile improvements offline, and promote artifacts safely into production. The goal is simple: **ship agent behavior like software**—versioned, tested, observable, and improving over time without surprises.

## Why RLM-lite?

Because this is the **smallest, safest version of RLM that we can actually ship in production right now**.

“Full” RLM (as a concept) implies a very broad class of things: open-ended reasoning loops, dynamic control flow, arbitrary code execution, unbounded memory growth, and models that decide *how* to reason as much as *what* to say. That’s powerful—but it’s also exactly how you end up with systems that are hard to audit, hard to bound, and impossible to replay in a Cloudflare Worker.

**RLM-lite** is a deliberately constrained subset:

* no arbitrary code execution,
* no unbounded loops,
* no hidden scratchpads,
* no silent context growth.

Instead, the model can only act through a **small, explicit action DSL**, and every step is executed by a kernel that enforces **hard budgets**, **deterministic traces**, and **schema-checked outputs**. Long context lives in variable space (blobs and handles), not in the token window, and every access is recorded. If something goes wrong, the system fails closed and you can see exactly why.

The “lite” is a feature, not a limitation. It gives you the core benefit people actually want from RLM—**avoiding context rot while working over large inputs**—without turning Autopilot into a black box. Full RLM ideas can still exist on the research side, but what runs in production is the version that is auditable, replayable, and boring in all the right ways.

## What we have implemented

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

### 5.1 DSE Debug Panel: Run A Canary Recap (direct vs rlm_lite)

We now ship a **Phase D canary surface** in the `/autopilot` UI so operators can run the same recap signature under
either strategy and see the receipts/traces end-to-end.

Where:

- `/autopilot` bottom-right controls → **DSE Debug**

How to use it:

1. Pick **Strategy**:
   - `direct.v1` (single-call predict, blob previews inlined)
   - `rlm_lite.v1` (bounded RLM loop, blob previews omitted; controller uses RLM ops)
2. Pick a **Budget** profile:
   - `small / medium / long` (these map to `params.budgets` and include RLM-specific counters)
3. Click **Run recap (canary)**.

What you should see:

- A user command message (audit trail): `/dse recap strategy=... budget=...`
- An assistant message with a `dse.signature` debug card for:
  - signature: `@openagents/autopilot/canary/RecapThread.v1`
  - `strategyId`, budgets (limits + usage), prompt render stats, context pressure
  - `rlmTrace` handle when `rlm_lite.v1` runs
- Links on the card:
  - **receipt**: `GET /api/dse/receipt/:receiptId`
  - **trace**: `GET /api/dse/blob/:receiptId/:blobId` (raw blob text; for RLM traces it is JSON)

Why this exists:

- It makes RLM-lite **observable** (you can see the strategy + budgets + evidence handles).
- It makes RLM-lite **debuggable** (you can open the trace and see the kernel actions/observations).
- It creates an operator-friendly path to compare `direct` vs `rlm_lite` without changing signature code.

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
