# RLM Trace Mining and Distillation

- **Status:** Draft (process doc)
- **Last updated:** 2026-02-10
- **Conflict rules:**
  - If terminology conflicts: `docs/GLOSSARY.md` wins.
  - If behavior conflicts with code: code wins.

Goal: treat RLM runs as an *exploration pass* for long-context problems, then distill the repeating tactics into explicit, typed behavior (signatures/modules/graphs) that DSE can compile and ship.

Related docs:

- Context rot and other failures: `docs/autopilot/context-failures.md`
- RLM integration plan: `docs/autopilot/rlm-synergies.md`
- DSE spec: `docs/autopilot/dse.md`
- Effect-only RLM/DSE design: `packages/dse/docs/EFFECT_ONLY_DSE_RLM_GEPA_MIPRO_DESIGN.md`

---

## 1) What counts as a "trace" in OpenAgents

For trace mining we care about *actionable structure*, not internal monologue.

Minimum trace streams:

- **Tool receipts**: tool name, params hash, output hash, latency, side effects.
- **Predict receipts** (DSE): signature id, compiled_id, prompt hashes, budgets, outcome (ok/error).
- **RLM iteration events** (when present): per-iteration actions, blob/span accesses, sub-LM calls, derived-variable writes.
- **Session replay**: `ReplayBundle` today; target `REPLAY.jsonl` v1 per `crates/dsrs/docs/REPLAY.md`.

For FRLM runs (`pylon rlm`), traces are also stored locally:

- `pylon rlm --log` (default true) persists trace events to a local SQLite DB.

---

## 2) What we must log to make traces useful

Without these fields, trace mining collapses into vibes:

- **Scope keys**: `threadId`, `runId`, `signatureId`, `compiled_id` (when applicable).
- **Strategy id**: direct vs RLM (and RLM variant).
- **Budgets**: limits + per-run usage counters (time, lm calls, tool calls, output chars, iterations, subcalls).
- **Context pressure snapshots:** what was the pressure estimate and what inputs drove it.
- **Evidence access**:
  - blob/span identifiers (BlobRef or SpanRef)
  - how much was surfaced into token space (preview sizes)
- **Progress signals**:
  - "new evidence" events per iteration (did we read anything new)
  - stuck/thrash detection outcomes (if triggered)

Implementation note (DSE RLM-lite):

- RLM-lite traces are persisted as a BlobStore JSON document with format `openagents.dse.rlm_trace` and include:
  - `signatureId`, `receiptId`, `strategyId`
  - an `Input` event (encoded signature input JSON)
  - per-iteration `action` + `observation` events, including the `Final` output payload
- This is the minimum needed to export candidate labeled examples from traces.

---

## 3) Workflow: RLM explore -> distill -> compile -> ship

### 3.1 Choose a long-context workload

Pick a task where direct prompting is unstable:

- repo-scale Q&A with evidence requirements
- log analysis
- "needle in haystack" retrieval

### 3.2 Run an exploratory RLM pass (verbose, budgeted)

Two practical entry points today:

1. **FRLM via Pylon** (fanout + trace DB):

```bash
pylon rlm "Explain why this build fails" --file ./output/build.log --budget 2000 --fanout 10
pylon rlm history --limit 20
```

2. **RLM engine (local)**:

- For algorithmic guidance and pitfalls (especially symbolic recursion), read `crates/rlm/docs/METHODS.md`.

### 3.3 Review traces and extract repeating tactics

Look for repeated patterns like:

- initial situating: sample, inspect, characterize the context
- a chunking policy that works reliably for the domain
- repeated evidence ops: grep/read_lines/symbols
- per-chunk extraction and a final synthesis pass
- early stop conditions when confidence is high

The output of this step should be a short list of "tactics" described as:

- preconditions (when to apply)
- operations sequence (tools + data flow)
- stop/verify criteria
- budgets required

### 3.4 Distill into typed building blocks

Distillation targets (in order of preference):

- A **Signature** (decision point) when it gates action or routing.
- A **Module** or small **graph IR** when the behavior is a repeatable pipeline.
- A compiler-visible **param surface** when the best approach varies (instruction blocks, chunking knobs, role selection, budgets).

Avoid the anti-pattern: copying "the best trace" into a prompt string.

### 3.5 Compile and compare

Use DSE eval/compile to compare:

- distilled pipeline (fast path)
- RLM exploratory pass (fallback)
- direct predict (baseline)

Promote only if the distilled behavior is measurably better on holdout (quality and/or cost/latency).

### 3.6 Export candidates from traces into `dseExamples` (Convex-first)

Goal: turn one good exploratory run (receipt + RLM trace) into a candidate labeled example row:

- `signatureId`
- `exampleId`
- `inputJson` (from RLM trace `Input`)
- `expectedJson` (from RLM trace `Final.output`)

#### Headless (many receipts): review -> export -> tagging

For scale-up, use the headless miner (Bearer `OA_DSE_ADMIN_SECRET`) which:

1. lists candidate predict receipts for a signature
2. exports candidate examples into `dseExamples`
3. tags exported rows (adds `trace_mined` by default)

```bash
OA_DSE_ADMIN_SECRET="..." \
  bun run apps/web/scripts/dse-trace-mine.ts \
    --base-url http://localhost:3000 \
    --signature-id "@openagents/autopilot/canary/RecapThread.v1" \
    --split train \
    --tag seed
```

Under the hood it calls:

- `GET /api/dse/receipts/list?...` (ops-admin only)
- `POST /api/dse/trace/export` (upserts into `dseExamples`)

#### Step 1: find the DSE predict receipt id

In the Autopilot UI, inspect the `dse.signature` chat card for the run and copy `receiptId`.

#### Step 2: export via the Worker endpoint

Use the admin endpoint (authed):

```bash
curl -X POST "http://localhost:8787/api/dse/trace/export" \
  -H "content-type: application/json" \
  -d '{"receiptId":"<RECEIPT_ID>","split":"train","tags":["seed"],"dryRun":false}'
```

Behavior:

- creates/updates `dseExamples` for the receipt’s `signatureId`
- default `exampleId` is `trace:<receiptId>` (override via `exampleId`)
- tags include `trace_export` and `strategy:<strategyId>` plus any user tags
- exported rows include structured linkage metadata in `dseExamples.meta`:
  - `kind=openagents.trace_export.v1`
  - `receiptId`, `threadId/runId`, `rlmTrace.blobId`, `strategyId`, `compiled_id`

#### Step 3: compile and promote

After exporting enough examples:

1. compile: `POST /api/dse/compile` (writes artifact + compile report)
2. promote: `POST /api/dse/promote` (holdout delta gate; writes active pointer; canary cleared)

Reference: `docs/autopilot/SELF_IMPROVE_PLAN.md` (Stage 5/6 endpoints).

---

## 4) Guardrails

- **Keep traces replayable**: bounded, structured events; stable ids; hash full payloads even if previews are truncated.
- **Do not treat RLM as a universal fix**: poisoning/confusion still require provenance and verification (`docs/autopilot/context-failures.md`).
- **Prefer symbolic recursion for large N**: do not require the controller LM to emit O(N) subcalls.
