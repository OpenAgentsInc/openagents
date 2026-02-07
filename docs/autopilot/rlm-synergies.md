# RLMs (“Recursive Language Models”) and OpenAgents Autopilot / Effect / DSE: Synergies and Integration Plan

- **Source analyzed:** “SPy is the easiest way to use RLMs” (Isaac Miller, 2026-02-03) — excerpt provided in chat
- **Related docs:** `docs/autopilot/dse.md`, `docs/autopilot/AUTOPILOT_OPTIMIZATION_PLAN.md`, `docs/autopilot/monty-synergies.md`, `docs/autopilot/microcode-synergies.md`

This doc translates the RLM pattern described in the article into **DSE + Autopilot** terms and proposes an Effect-first integration plan.

---

## 1) RLM recap (what matters for us)

### 1.1 Two buckets of context

RLMs split “what the model can access” into two spaces:

- **Variable space**: a persistent REPL-like environment holding large inputs as variables.
- **Token space**: a small, dynamic slice of information loaded from variable space into the prompt when needed.

The key move is: **do not stuff long context into the model prompt**. Instead, load long inputs into a persistent environment and provide only:
- variable names (`context`, `other_inputs`, etc.)
- small previews (e.g. `context[:100]`)
- instructions for how to explore.

### 1.2 Recursion via sub-LLM calls

The “recursive” part is the main model’s ability to call a **sub-model** (subagent) from within the REPL (e.g. `sub_llm(prompt)`), write results back to variable space, and choose what to tokenize.

This is similar to agent “subtasks,” but with a crucial difference:
- results land in **manipulable variable space**, not automatically in the main prompt.

### 1.3 Budgets

RLMs need explicit leashes:

- `max_iterations`: number of REPL turns
- `max_llm_calls`: number of sub-LLM calls across the run
- `sub_lm`: a cheaper model role for subcalls

### 1.4 Signature compatibility (DSPy’s “killer feature”)

The article’s strongest product claim is modularity: **existing Signatures work unchanged**; RLM is an inference-time strategy you swap in.

This is exactly the posture DSE should aim for: **signatures are the stable contract; strategies are swappable execution policies**.

---

## 2) Why this aligns with DSE and what it unlocks

### 2.1 DSE already wants the right primitives

From `dse.md`, DSE already centers:
- typed `Signature` IO (Effect Schema)
- prompt IR + transforms
- artifacts + pinned `compiled_id`
- eval + compile loops

RLM adds a missing execution mode for a specific problem shape:
- **long-context reasoning without context rot**

### 2.2 It composes with our recent “BlobStore / BlobRef” direction

RLM “variable space” maps cleanly onto:
- a `BlobStore` service (store large inputs once)
- a `VarSpace`/`Workspace` service (named variables referencing blobs + derived results)

Microcode’s paste placeholder pattern and our Phase 0 “Blob references for large context” are the same underlying idea: **don’t duplicate large blobs in token space; reference them**.

---

## 3) Effect-first design: DSE RLM as an inference strategy

### 3.1 Introduce an explicit “inference strategy” interface (DSE)

DSE should treat “Predict” as policy-driven. Concretely:
- `DirectPredict` (today’s normal LLM call)
- `RlmPredict` (two-bucket context + REPL loop)
- later: `GraphPredict` (DAG runner), `ToolRouterPredict`, etc.

The stable unit remains the `Signature<I,O>`.

### 3.2 Define the RLM runtime services (Effect)

Minimum services to run an RLM step:

- **`VarSpace`**: persistent store of named variables (typed metadata + blob refs)
- **`BlobStore`**: immutable blob storage addressed by hash/id (DO SQLite/R2)
- **`RlmKernel`**: executes “REPL actions” and returns outputs + updated varspace
- **`LmClient` with roles**:
  - `main` (RLM controller)
  - `sub` (recursion helper)
  - (optional later) `judge`, `repair`
- **`ExecutionBudget`**: enforces `maxIterations`, `maxSubLmCalls`, `maxToolCalls`, time, output size, etc.
- **Receipts/trace**: each iteration is a structured event stream that can be replayed.

### 3.3 What is the “REPL language”?

The article assumes Python code in a sandbox. For us, the integration should be phased:

- **Phase A (RLM-lite, safest)**: a **structured action DSL** (no arbitrary code) with a small set of primitives:
  - `preview(var, slice)`
  - `search(blobRef, query)` (regex/keyword)
  - `load(blobRef, range)` / `chunk(blobRef, size)`
  - `json_parse(text)` / `extract(schema, text)`
  - `sub_lm(prompt, inputs...)`
  - `tool_call(name, args)`
  - `write_var(name, valueRef)`

  This captures the core “two bucket” and “recursion” benefits while staying deterministic and replayable.

- **Phase B (Python, Monty-inspired)**: allow a Python-like REPL only if we can guarantee:
  - capabilities are externals-only (see `monty-synergies.md`)
  - hard limits (time/memory/depth)
  - checkpoint/resume at external calls

In Workers, that likely means Python execution behind a narrow service boundary (or a compatible runtime), not “native Python” in-process.

---

## 4) How DSE compilation/eval should use RLM

### 4.1 Evaluation

RLM is an inference strategy, so evaluation should compare:
- `DirectPredict(signature)` vs `RlmPredict(signature)` on long-context datasets
- budget consumption vs quality (latency/cost vs reward)

### 4.2 What to optimize

The article notes DSPy compiles “tool descriptions + signature instructions” into an instruction block and optimizes that with MiPRO/GEPA/etc.

In DSE terms, we should make these explicit params/search spaces:
- the **RLM controller instruction block** (Prompt IR blocks for the controller)
- allowed action/tool set (capabilities)
- chunking policy knobs (chunk size, stride)
- `maxIterations`, `maxSubLmCalls`
- model-role selection (sub model id)

Compilation would output a `compiled_id` that pins:
- RLM strategy selection
- its policy knobs
- its allowed capabilities

---

## 5) Proposed integration plan (what to build in OpenAgents)

### Phase 0 (foundation — already in our plan)

- `BlobStore` + blob references in Prompt IR and receipts
- `ExecutionBudget` with `maxSteps/maxLlmCalls/maxToolCalls/maxOutputChars`

### Phase 1 (add RLM strategy, RLM-lite kernel)

- Add `RlmPredict` strategy for any `Signature<I,O>`:
  - put large inputs into `VarSpace` as blob refs
  - prompt the main model with variable names + previews + available actions
  - loop:
    - model emits action(s)
    - kernel executes actions
    - append bounded observation to token space
  - final: decode into `O` via Schema + repair policy
- Implement `sub_lm(...)` as a role-based LLM call with its own budget counter.
- Record iteration-level receipts suitable for replay.

### Phase 2 (datasets + eval for long-context tasks)

- Create one or two canonical “long context” datasets:
  - codebase subset selection (needle-in-haystack)
  - log Q&A / evidence sourcing
- Evaluate `DirectPredict` vs `RlmPredict` and promote when net utility is positive.

### Phase 3 (compile knobs specific to RLM)

- Add compiler search spaces for:
  - controller instruction variants
  - chunking policy knobs
  - sub-model selection
  - budget profiles
- Produce artifacts that pin “RLM strategy policy bundles”.

### Phase 4 (optional: Python execution / Monty-style kernel)

- Only after the RLM-lite kernel proves value:
  - define a `CodeKernel` interface compatible with Monty’s “externals-only” model
  - keep all side effects behind tool externals
  - require checkpoint/resume and strict limits

---

## 6) Where this plugs into Autopilot

Autopilot’s Durable Object is the natural home for RLM persistence:
- the `VarSpace` can be per-thread/per-user
- subagents can share it (as the article suggests)
- receipts and replay can correlate iterations to user turns

This yields a concrete Autopilot capability:
- “work across huge repos/logs without context rot,” while staying bounded and auditable.

---

## 7) Risks / guardrails

- **Cost blowups**: RLM can quietly do many subcalls; budgets must be enforced and recorded.
- **Non-determinism**: the kernel must be deterministic; randomness must be seeded and logged.
- **Privacy/retention**: blob storage + varspace must obey privacy modes and redaction/truncation policies.
- **Debugging**: prefer structured traces (“what actions ran”) over raw “internal monologue.”

