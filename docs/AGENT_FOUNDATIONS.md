# Agent Foundations (OpenAgents)

- **Audience:** a new coding agent joining OpenAgents (Autopilot / Adjutant / dsrs / Pylon / Nexus)
- **Goal:** teach the concepts and implementation patterns that make OpenAgents **reliable, self-improving, and scalable**
- **Status:** Needs audit
- **Last verified:** (see commit)
- **Source of truth:** For terminology, **GLOSSARY.md wins**. For behavior, **code wins**.

> Start here, then branch:
> - **Vocabulary:** [GLOSSARY.md](GLOSSARY.md)
> - **Repo map:** [PROJECT_OVERVIEW.md](PROJECT_OVERVIEW.md)
> - **MVP gates / "what to ship next":** [ROADMAP.md](ROADMAP.md)
> - **Reality / what's wired:** [SYNTHESIS_EXECUTION.md](./SYNTHESIS_EXECUTION.md)
> - **Architecture decisions:** [docs/adr/](./docs/adr/) (ADRs for contracts, interfaces, invariants)
> - **DSPy core docs:** `crates/dsrs/docs/*` (ARCHITECTURE, SIGNATURES, TOOLS, METRICS, OPTIMIZERS, EVALUATION)
> - **Protocol surface:** [docs/protocol/PROTOCOL_SURFACE.md](./docs/protocol/PROTOCOL_SURFACE.md)

---

## 0) Mental model

OpenAgents is **not** “an LLM wrapper.” It’s a **runtime + compiler + (optional) market**:

- **Execution runtime**: runs tools/sandboxes/jobs, enforces schema + retries, and emits replayable receipts.
- **Compiler layer (dsrs / DSPy)**: turns agent behavior into **typed programs** (Signatures + Modules) that can be optimized via metrics into **policy bundles**.
- **RLM/FRLM**: execution modes for out-of-core reasoning over huge contexts (repo-scale + long sessions).
- **Market layer (NIP-90 / DVMs)**: treats compute and sandbox execution as purchasable services with verification and receipts.
- **Verification** (tests/builds): the anchor for correctness; everything else is optimization.

If you’re writing code in this repo, you’re usually doing one of:

1) adding a **capability** (tool / job schema / provider / lane),
2) adding a **policy** (signature / module / routing rule), or
3) adding a **measurement** (metrics / labels / counterfactual recording / eval).

---

## 1) The six DSPy primitives (as OpenAgents uses them)

> These terms are canonical in [GLOSSARY.md](GLOSSARY.md). If you see a mismatch here, treat it as a doc bug.

### 1.1 Signatures

**Definition:** typed input/output contracts for a model call.

**OpenAgents rule:** if it gates an action or decision, it must be a signature (or a signature-backed pipeline).

**Where they live**
- dsrs traits + runtime: `crates/dsrs/`
- Adjutant pipelines + app-facing decisions: `crates/adjutant/src/dspy/*`
- Specs may also exist in `crates/dsrs/docs/SIGNATURES.md` (but code is truth).

**Inputs must be**
- deterministic, runtime-supplied facts: task text, repo context slices, tool outputs, budgets, lane availability, etc.

**Outputs must be**
- machine-consumable: enums/booleans/arrays/JSON blobs, with strict parsing.
- no “just prose” fields used downstream as control signals.

**Decision signatures**
- must include `confidence: f32` and be **confidence-gated** (fallback to legacy rules below threshold).
- must be recorded with counterfactuals: DSPy output vs legacy output vs actual executed decision.

**Common signatures in this repo**
- planning: `SubtaskPlanningSignature`
- execution: `SubtaskExecutionSignature`
- synthesis: `ResultSynthesisSignature`
- routing: `ComplexityPipeline`, `DelegationPipeline`, `RlmTriggerPipeline`

**MVP-critical (canonical path)**
- `ToolCallSignature` (select tool + params) — spec exists, may not be fully wired yet
- `ToolResultSignature` (interpret tool output + emit `step_utility` in **-1..+1**) — spec exists, may not be fully wired yet

> Naming note (important):
> - **Canonical** learning label is `step_utility` in range **-1.0..+1.0** (from `ToolResultSignature`).
> - `ToolStepUtilitySignature` (if used) outputs `step_utility_norm` in range **0.0..1.0** and is a *judge/helper*, not the canonical label.

---

### 1.2 Modules

**Definition:** composable programs built from signatures + glue logic. A module is “workflow as code.”

**OpenAgents rule:** workflows are modules; prompts are implementation details.

**Implementation expectations**
- Modules implement dsrs `Module` and often `Optimizable`.
- Modules are testable:
  - unit tests: parsing, invariants, schema validity
  - integration tests: end-to-end loop where feasible
- Modules emit trace/callback events for:
  - HUD visibility
  - dataset collection (training examples)
  - replay artifacts

**Suggested modular boundaries**
- `RepoContextBuilderModule` — what to read / retrieve
- `PlannerModule` — plan (PlanIR)
- `StepExecutorModule` — tool steps (ToolCall → ToolExec → ToolResult)
- `VerifierModule` — run harness + compute deltas
- `LoopControllerModule` — stop/continue/escalate logic
- `RlmModule` / `FrlmConductor` — recursion modes

---

### 1.3 Tools

**Definition:** structured interaction with the world: file reads/edits, bash, git, sandbox runs, swarm jobs, payments.

**OpenAgents rule:** every tool call must be **metered, logged, and replayable**.

**Where tools live**
- RLM tools: `crates/rlm/src/tools/*`
- Adjutant tool registry: `crates/adjutant/src/tools.rs`
- Tool schema validation + receipts are runtime responsibilities (see `TOOLS.md`).

**Requirements**
- Tools have JSON schemas for args.
- Runtime validates schema **before** execution.
- Every tool call yields a receipt-level record:
  - tool name
  - params hash (canonical)
  - output hash (canonical)
  - latency
  - side effects (files written, commands run)
  - (when applicable) token cost / msats cost

**Failure behavior**
- bounded output (truncate for display; hashes are computed on full output)
- deterministic errors (no “maybe it worked”)
- timeouts, circuit breakers, and safe defaults (no infinite loops)

**OpenAgents twist**
- treat **compute** as a tool:
  - “inference provider” calls are metered and traced
  - “compute provider (DVM)” jobs are dispatched and receipted
- treat **wallet/treasury actions** as tools with explicit policy enforcement (budgets, approvals, reconciliation).

---

### 1.4 Adapters

**Definition (canonical):** serialization + parsing boundary for a provider format.

**OpenAgents rule:** signatures stay clean; adapters absorb provider-specific formatting and response parsing.

**Hard constraint (from Glossary):**
- **Adapters do NOT validate or retry.**
  Validation, retries, and tool execution policy belong to the **execution runtime** (or meta-operators like `Refine`).

**What adapters should do**
- render signature + examples into provider’s prompt/messages
- parse provider outputs back into typed fields
- normalize provider tool-call conventions (format differences)

**What must be recorded (telemetry)**
- parse error rate
- truncation events
- context-limit events
- provider response latency

**Where**
- chat formatting/parsing: `crates/dsrs/src/adapter/*`
- provider adapters: `crates/dsrs/src/lm/*` and related crates

---

### 1.5 Optimizers

**Definition:** compilation loops (MIPROv2 / COPRO / GEPA / Pareto) that improve a program (signatures/modules) to maximize metrics.

**OpenAgents rule:** optimize on **real sessions** and outcome labels, not toy examples.

**Implementation expectations**
- Optimizers consume:
  - collected traces/examples
  - outcome labels (verification deltas, repetition, cost)
- Prioritization:
  - “optimize worst signature first” (rolling accuracy / biggest impact)
- Auditability:
  - keep compile-run artifacts: candidates tried, scorecards, selected bundle
- Product discipline:
  - **policy bundles** are versioned artifacts you can pin/rollback/canary.

---

### 1.6 Metrics

**Definition:** what “better” means.

**OpenAgents rule:** format correctness is necessary; **outcome-coupled utility** is sufficient.

**Metric stack**
1) **Contract metrics** (cheap proxy)
   - JSON validity, required fields, enum values, confidence range, schema match
2) **Utility metrics** (core)
   - `verification_delta`, `step_utility`, repetition penalties, cost/latency penalties
3) **Fleet/product metrics**
   - verified success rate, cost per success, escalation rate, regression rate, APM/sAPM

**Avoid**
- optimizing only pretty outputs (Goodhart trap)

---

## 2) Open-endedness + self-improvement: why OMNI / OMNI-EPIC / DGM matter

You don’t need to “implement OMNI” to use the ideas. You need the mapping:

### 2.1 OMNI: learnable ≠ worthwhile

**Translation:** agents can thrash forever unless you define “worth doing.”

**OpenAgents mapping:** “interestingness” = **step utility under verification**
- did this step reduce failing tests?
- did it reduce uncertainty (new evidence)?
- did it avoid repetition?
- did it stay within budget?

**Directive**
- outcome labels (verification deltas + step utility) are not optional; they’re the difference between progress and theater.

### 2.2 OMNI-EPIC: code-defined environments expand the world

**Translation:** tools + job schemas define the reachable task space.

**Directive**
- treat every new tool or job schema as a “world expansion” and require:
  - deterministic inputs/outputs
  - receipts + hashing
  - verification path if objective (or explicit subjective tiering)

### 2.3 DGM: archives beat greedy hillclimbs

**Translation:** keep variants, evaluate empirically, and allow rollbacks.

**OpenAgents mapping**
- archive:
  - sessions + labeled examples
  - compile runs
  - policy bundles with metrics
- rollout discipline:
  - candidate → staged → shadow → promoted (rollback on regression)

---

## 3) RLM / FRLM: recursion as an execution mode

### 3.1 What RLM changes

RLM treats “context” as external state (repo + logs + indexes) accessed via **operations** (peek/grep/partition/map/summarize), not as raw tokens stuffed into a prompt.

In OpenAgents:
- RLM is an execution mode (not “a prompt template”).
- Context ops are tools and must be logged + metered like any other tool call.

### 3.2 Implementation requirements

When adding/altering RLM/FRLM:
- expose a sandboxed environment with:
  - file access helpers, chunking, grep, symbol tools
  - subcalls: `llm_query(lane, prompt, caps)`
- enforce budgets:
  - max recursion depth
  - max subcalls per tick
  - per-lane spend caps
  - stopping criteria (diminishing returns / repetition)
- log everything:
  - context ops as tool calls
  - fanout jobs as NIP-90 dispatch events (where applicable)

### 3.3 When to trigger RLM

RLM should trigger when:
- estimated context pressure is too high (repo/global reasoning)
- tasks are information-dense (need precision, not summary)
- verification loop indicates stuckness without new evidence (thrash signal)

---

## 4) Implementation checklists (what to do in code)

### 4.1 If you add a new decision point

✅ make it a **signature** (or signature-backed pipeline)
✅ include `confidence` and **confidence-gate** behavior
✅ record **counterfactuals** (DSPy vs legacy) in session records
✅ add post-hoc labeling from outcomes (success/failure + deltas + cost)
✅ wire into optimizer targeting (rolling accuracy + impact)

### 4.2 If you add a new tool

✅ register it in the canonical registry
✅ define an args schema and ensure runtime validates it
✅ emit receipt fields (params/output hashes, latency, side effects)
✅ enforce safe limits (timeout, bounded output, deterministic failure)
✅ add tests for schema + failure modes

### 4.3 If you add a new provider / lane

✅ implement provider integration + health detection
✅ add adapter formatting/parsing for that provider (no retries here)
✅ ensure cost accounting exists (tokens/latency/msats)
✅ make lane selection policy-driven (signature) and auditable
✅ add fallback strategy + circuit breaker behavior

### 4.4 If you “improve performance”

✅ don’t hand-tweak prompts in place
✅ move the behavior behind:
- a signature/module
- a measurable metric
- a policy bundle update (with rollback/canary path)

---

## 5) The flywheel we’re building

1) Autopilot runs tasks and emits **trajectories + receipts**
2) Sessions record decisions + counterfactuals + outcomes
3) Outcome feedback labels decisions and tool steps
4) dsrs compiles improved policies (MIPROv2 / GEPA / etc.)
5) policy bundles roll out via shadow/canary gates
6) FRLM can buy compute for fanout and distill competence down into cheaper lanes
7) repeat (capabilities compound without rewriting orchestration code)

---

## 6) Non-negotiable engineering invariants

- **Verification first**: green tests > persuasive narration
- **Typed contracts everywhere**: signatures and tools are structured, parseable, schema-valid
- **Everything is logged**: decisions, tool receipts, costs, counterfactuals
- **Optimize what matters**: outcome-coupled metrics, not formatting-only proxies
- **Avoid Goodhart**: pair proxies with truth checks and regression gates
- **Archives over greed**: keep policy variants; don’t overwrite history without a rollback path

---

## Appendix: What changed vs the prior spec

1) **Adapters**: clarified to match canonical definition
   - adapters serialize/parse only; **runtime** owns validation + retries.

2) **Step utility naming**: aligned with canonical terms
   - `step_utility` is **-1..+1** and belongs to `ToolResultSignature`.
   - `ToolStepUtilitySignature` (if present) should output `step_utility_norm` **0..1** as a helper/judge.

3) **Docs pointers**: added explicit “start here” links so this file doesn’t duplicate README/overview/roadmap.

If you need an agent to implement something from this doc, treat this file as onboarding + invariants, and put the concrete “do X in files Y/Z” into an issue or a crate-local doc near the implementation.
