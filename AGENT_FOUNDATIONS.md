# Spec: OpenAgents Agent Foundations

**Audience:** a new coding agent joining OpenAgents (Autopilot/Adjutant/dsrs/Pylon/Nexus)
**Goal:** teach the agent the concepts and implementation patterns that make OpenAgents **self-improving, open-ended, and reliable**.

---

## 0) Mental model: what OpenAgents is building

OpenAgents is not “an LLM wrapper.” It is a **market + runtime + compiler** for autonomous agents:

* **Runtime** executes actions (tools, sandboxes, jobs) and records receipts.
* **DSPy (dsrs)** compiles agent behavior (signatures/modules) into better prompts/policies via metrics.
* **RLM/FRLM** gives agents out-of-core reasoning over massive contexts.
* **Marketplace (Pylon/Nexus/NIP-90)** turns compute into a purchasable tool with receipts and budgets.
* **Verification** (tests/builds) anchors correctness; everything else is optimization.

If you’re writing code in this repo, you’re usually adding one of:

1. a new **capability** (tool/job/back-end/lane),
2. a new **policy** (DSPy signature/module), or
3. a new **measurement** (metric, labels, counterfactuals).

---

## 1) The 6 DSPy core concepts and how OpenAgents implements them

### 1.1 Signatures

**Definition:** typed contracts for LM calls (inputs/outputs + optional instruction).
**OpenAgents rule:** *If it gates an action or a decision, it must be a signature.*

**Implementation requirements**

* Signatures live in `crates/adjutant/src/dspy/…` (and dsrs traits in `crates/dsrs/…`).
* Inputs are **only** deterministic data the runtime can supply (task text, repo context, tool outputs, budget, lane availability).
* Outputs are **strictly machine-consumable** fields (JSON/booleans/enums/arrays), not prose.
* Every signature should include `confidence: f32` when used for routing/override logic.

**Examples**

* Planning: `SubtaskPlanningSignature`
* Execution: `SubtaskExecutionSignature`
* Synthesis: `ResultSynthesisSignature`
* Decisions: complexity/delegation/RLM trigger signatures

**Must-add baseline signature**

* **ToolStepUtilitySignature**: produces `step_utility`, `should_continue`, `next_action_hint`, `confidence`.

---

### 1.2 Modules

**Definition:** composable graphs of signatures (and logic) that implement an agent workflow.
**OpenAgents rule:** *Workflows are modules; prompts are implementation details.*

**Implementation requirements**

* Modules implement dsrs `Module`/`Optimizable` (or wrap `Predict` units).
* Modules are versionable and testable (unit tests for parsing/format; integration tests for end-to-end).
* Modules must emit trace events for TrainingCollector + sessions.

**Recommended modular boundaries**

* `RepoContextBuilderModule` (what to read)
* `PlannerModule` (plan)
* `ExecutorModule` (execute tool steps)
* `VerifierModule` (run tests/builds + summarize)
* `LoopControllerModule` (stop/continue/escalate)
* `RlmModule` (out-of-core context logic)

---

### 1.3 Tools

**Definition:** structured interactions with the world: filesystem edits, bash, git, sandbox, compute jobs, payments.
**OpenAgents rule:** *All tools must be metered, logged, and replayable.*

**Implementation requirements**

* Tools are registered via a canonical `ToolRegistry`.
* Every tool call yields a **receipt**:

  * `tool_name`, `inputs_hash`, `outputs_hash`, `latency_ms`, `cost_tokens` (if applicable), `side_effects[]`.
* Tools must degrade safely:

  * timeouts, bounded output, deterministic errors, no infinite loops.

**OpenAgents twist**

* Treat “compute” as a tool:

  * `ComputeTool.submit(job_schema, payload, budget_cap) -> result + receipt`
* Treat “wallet” actions as tools with budget enforcement.

---

### 1.4 Adapters

**Definition:** provider boundaries: formatting + parsing + tool schema translation across Codex/Anthropic/Ollama/Cerebras/Pylon.
**OpenAgents rule:** *Signatures stay clean; adapters absorb provider mess.*

**Implementation requirements**

* Adapters:

  1. render prompts from signature,
  2. parse/repair outputs to typed structs,
  3. normalize tool-call conventions.
* Must record:

  * parse error rate, retry count, truncation events, context-limit events.

**Must-have adapter behavior**

* `SchemaRepairAdapter`: minimal JSON repair + retry policy
* hard cap on retries; failures fall back to legacy rules when applicable.

---

### 1.5 Optimizers

**Definition:** compilation loops that search prompt/demos/instruction variants to maximize metrics (MIPROv2 etc.).
**OpenAgents rule:** *Optimize on real sessions, not toy examples.*

**Implementation requirements**

* Optimizers run on **collected traces** + **outcome labels**.
* Use “optimize worst signature first” (rolling accuracy).
* Keep audit artifacts:

  * compile runs, candidates tried, scores, chosen policy bundle.

**Evolution direction**

* Move from “single latest compiled policy” to **policy bundles + archival selection** (see DGM relevance).

---

### 1.6 Metrics

**Definition:** what “better” means.
**OpenAgents rule:** *Format correctness is necessary; outcome-coupled utility is sufficient.*

**Implementation requirements**
Use a metric stack:

1. **Contract metrics** (cheap): JSON validity, required fields, enum values, confidence range
2. **Utility metrics** (core): verification delta, step utility, repetition penalty, cost penalty
3. **Product metrics** (fleet): success rate, cost per success, escalation rate, regression rate

**Must avoid**

* optimizing only “pretty outputs” (Goodhart risk)

---

## 2) How OMNI / OMNI-EPIC / DGM relate (and why we care)

### 2.1 OMNI: “learnable ≠ worthwhile”

OMNI’s lesson: a system can make progress forever on *boring* tasks unless it has a notion of **interestingness** / **worth doing**.

**OpenAgents mapping**

* Your version of “interestingness” is **step utility**:

  * does this step reduce failing tests?
  * does it narrow the search?
  * does it produce new actionable info?
  * is it redundant?

**Implementation directive**

* ToolStepUtilitySignature + outcome-coupled metrics are not optional—they are OMNI’s core idea translated to coding agents.

---

### 2.2 OMNI-EPIC: “code-defined environments unlock open-endedness”

OMNI-EPIC’s leap: once tasks/worlds are *coded*, the search space becomes unbounded.

**OpenAgents mapping**

* Your “environment” is: repo + tools + job schemas + verifier
* Expanding the tool surface and protocol job schemas expands the world.

**Implementation directive**

* Treat every new job schema / tool as a “world expansion.”
* Ensure it has:

  * deterministic inputs/outputs,
  * receipts,
  * verification path if objective.

---

### 2.3 DGM: “self-improvement + archive beats greedy hillclimb”

DGM’s lesson: keep an **archive** of variants; explore stepping stones; empirically validate improvements.

**OpenAgents mapping**

* Your current “archive” is: sessions + labeled examples + compile runs.
* Next step is to archive **policy bundles** and sometimes branch/rollback.

**Implementation directive**

* Never assume “latest policy is best.”
* Store multiple compiled variants and evaluate selection policies over time.

---

## 3) RLM / FRLM: Out-of-core reasoning as an execution mode

### 3.1 What RLM changes

RLM treats “context” as **data in an external environment** (REPL/sandbox), not as tokens in the model window. The model writes code to inspect/select snippets and can recursively call sub-models.

**OpenAgents mapping**

* RLM is not a prompt. It’s a runtime mode: `ExecutionMode::RlmLocal` / `ExecutionMode::Frlm`.

### 3.2 Implementation requirements

* Provide a sandboxed environment that exposes:

  * file access helpers, chunking, grep, structured buffers
  * `llm_query(lane, prompt, caps)` for subcalls
* Meter everything:

  * max recursion depth, max subcalls, budget caps
* Make it optimizable via DSPy:

  * `RlmPlanSignature`, `RlmProbeSignature`, `RlmSynthesisSignature`
* Use utility gating to prevent thrash:

  * stop criteria based on marginal utility

### 3.3 When to use RLM

RLM should trigger when:

* estimated context size is too large for base LMs,
* tasks are information-dense (need precise access, not summary),
* verification loops indicate ambiguity (stuck without new evidence).

---

## 4) OpenAgents implementation checklist (what you should do in code)

### 4.1 If you add a new decision point

✅ Create a **signature** with `confidence`
✅ Add a **metric** (contract + outcome-coupled)
✅ Record **decision + counterfactual** (legacy output)
✅ Label it post-hoc via session outcomes
✅ Include in auto-optimizer targeting

### 4.2 If you add a new tool

✅ Register in `ToolRegistry`
✅ Emit structured **receipt** + hashes
✅ Add safety: timeouts, bounded output, no infinite loops
✅ Add tests for schema and failure modes
✅ Ensure it is compatible across repos (no hardcoded paths)

### 4.3 If you add a new provider/lane

✅ Add adapter + health detection
✅ Define how tool calls are represented and parsed
✅ Add cost accounting (tokens/latency/sats)
✅ Ensure lane selection is policy-driven (signature), not hardcoded

### 4.4 If you improve performance

✅ Don’t “just tweak prompts.” Convert the behavior into:

* a signature/module, then compile it
  ✅ Add a metric that matches the real goal (tests green, fewer iterations, lower cost)
  ✅ Store the compile run artifact

---

## 5) The “flywheel” OpenAgents is aiming for

1. Autopilot runs tasks and produces **trajectories + receipts**
2. Sessions record decisions + counterfactuals + outcomes
3. Outcome feedback labels decisions/tool steps
4. dsrs compiles better signatures/modules (MIPROv2)
5. Policy bundles improve routing, planning, and tool usage
6. FRLM lets agents buy compute, distill improvements, reduce costs
7. Repeat—capabilities compound without rewriting orchestration code

---

## 6) Non-negotiable engineering principles

* **Verification first:** green tests > persuasive text
* **Everything is typed:** signatures and tools must be structured
* **Everything is logged:** decisions, tool receipts, costs, counterfactuals
* **Optimize what matters:** outcome-coupled metrics, not formatting
* **Avoid Goodhart:** pair metrics with objective checks and audits
* **Archives over greed:** keep variants; don’t overwrite history
