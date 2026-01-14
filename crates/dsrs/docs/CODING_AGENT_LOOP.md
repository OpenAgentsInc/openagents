# Spec: DSPy-Driven Coding Agent Loop (OpenAgents)

## Purpose

Define the **core long-running coding agent loop** as a deterministic execution runtime plus **DSPy (dsrs) signatures/modules** that make decisions at explicit control points. The result is:

* auditable (REPLAY + RECEIPT),
* optimizable (policy bundles + training signals),
* safe (runtime-enforced tool schemas, privacy, retries),
* portable across model providers and lanes.

This document is a **design/spec**. Canonical schemas and terminology live elsewhere (see **Authority**).

---

## Authority and Canonical References

* **Behavior:** code wins
* **Terminology:** `GLOSSARY.md` wins
* **Architecture intent / invariants:** ADRs win

  * Verified Patch Bundle (ADR-0002)
  * Replay formats (ADR-0003)
  * Step utility semantics (ADR-0005)
  * Deterministic hashing (ADR-0006)
  * Tool execution contract (ADR-0007)
  * Session storage layout (ADR-0008)
  * Decision gating + counterfactuals (ADR-0010)
  * Lanes (ADR-0004), Schema IDs (ADR-0011), Objective vs subjective (ADR-0012)
  * Privacy defaults for swarm dispatch (ADR-0016)
  * Telemetry/trace contract (ADR-0017)
  * Forge adapter contract (ADR-0018)

Canonical schemas:

* `crates/dsrs/docs/REPLAY.md` — `REPLAY.jsonl` v1
* `crates/dsrs/docs/ARTIFACTS.md` — `PR_SUMMARY.md`, `RECEIPT.json`
* `docs/PROTOCOL_SURFACE.md` — job schemas, receipts (protocol-level), hashing notes

---

## Goals

1. Implement a **repeatable loop** that can run for minutes/hours without losing coherence.
2. Make every “choice” a **typed DSPy signature** (learnable, confidence-gated).
3. Make every tool action **runtime-enforced** (schema validation, timeouts, retries).
4. Emit **Verified Patch Bundle** per session:

   * `PR_SUMMARY.md`
   * `RECEIPT.json`
   * `REPLAY.jsonl`
5. Record **learning signals** per step:

   * `step_utility` in `[-1, +1]`
   * verification outcomes (`verification_delta`, pass/fail)
6. Support safe rollout:

   * `policy_bundle_id` attribution everywhere
   * counterfactual logging (DSPy vs legacy vs used)
   * shadow/canary capability

---

## Non-goals

* Defining full JSON schemas inline (link to canonical docs instead).
* Implementing a specific UI (HUD/replay viewer is downstream).
* Defining every tool (this spec defines the contract and loop integration).

---

## High-Level Architecture

**Principle:** *DSPy decides; runtime enforces.*

* **DSPy layer (dsrs):** typed signatures + modules

  * chooses context slices
  * chooses next step / plan
  * chooses tool calls (and parameters)
  * interprets tool results (facts + utility)
  * chooses model/lane (optional)
* **Execution runtime (adjutant/autopilot loop):**

  * validates tool params against JSON schema
  * executes tools deterministically (timeouts, bounded output)
  * retries tools safely (runtime policy)
  * enforces privacy defaults and redaction rules
  * writes receipts + replay events
  * runs objective verification (build/test/lint) when required

---

## Core Loop Overview

The loop is an iteration engine over a **session state**:

1. **Context selection**: decide what to include this turn
2. **Planning**: produce/update a PlanIR (step IDs + intents)
3. **Execution**: choose and run tool calls (or ask user)
4. **Interpretation**: extract facts + `step_utility`, decide continue/advance
5. **Verification**: run tests/build checks, compute `verification_delta`
6. **Synthesis + artifacts**: write Verified Patch Bundle

### Pseudocode (conceptual)

```text
init SessionState
emit ReplayHeader + SessionStart

while !done and iterations < max_iterations:
  context_plan = ContextSelectionSignature(SessionState.summary, budgets, privacy)
  ctx = runtime.assemble_context(context_plan)     // deterministic

  if needs_plan or plan_invalid:
    plan = PlanningSignature(ctx, task)            // emits PlanIR (or adapter -> PlanIR)
    runtime.record_plan(plan)                      // PlanStart event

  step = runtime.next_step(plan, SessionState)

  tool_call = ToolCallSignature(ctx, step, tool_schemas, history)
  if tool_call.needs_user_input:
    runtime.request_user(tool_call.question)
    continue

  runtime.validate_tool_params(tool_call)          // schema validation
  result = runtime.execute_tool(tool_call)         // timeouts, bounded output, retries
  runtime.emit_tool_events(tool_call, result)      // ToolCall + ToolResult events

  interpreted = ToolResultSignature(step, tool_call.expected_outcome, result.output)
  runtime.record_step_labels(interpreted)          // step_utility, extracted_facts

  if interpreted.should_continue and step.iterations < step.max_iterations:
    continue                                       // same step, new tool_call
  else:
    runtime.mark_step_complete(step)

verification = runtime.run_verification(plan.verification_strategy)
runtime.emit_verification(verification)

final = runtime.synthesize(PR_SUMMARY.md, RECEIPT.json, REPLAY.jsonl)
emit SessionEnd
```

---

## Deterministic Runtime Responsibilities

These must **not** live in DSPy adapters/modules (ADR-0007):

### Tool enforcement

* Validate tool params vs tool JSON schema **before execution**
* Execute tools with:

  * timeouts / cancellation
  * bounded outputs (truncate for display only; hash full output per ADR-0006)
  * safe retries (runtime policy; no “tool retries” via DSPy)

### Privacy enforcement

* Apply privacy policy (ADR-0016) *before any swarm dispatch or external emission*
* Reject on policy violations (do not silently proceed)

### Artifacts + hashing

* Compute:

  * `params_hash`, `output_hash` using canonical hashing rules (ADR-0006)
* Emit:

  * `REPLAY.jsonl` events per `REPLAY.md`
  * `RECEIPT.json` per `ARTIFACTS.md`
* Store under `${OPENAGENTS_HOME}` layout (ADR-0008)

### Verification

* Run objective checks (build/test/lint)
* Compute and record `verification_delta`
* Gate “success” on verification rules when applicable

---

## DSPy Signatures (Core Set)

These are the **minimum viable learnable control points** for parity-level capability.

### 1) ContextSelectionSignature

**Purpose:** Choose what context to include this turn (keep/summarize/drop).

* **Inputs:** session summary, recent turns, tool/file history, token budget, privacy mode, lane constraints
* **Outputs:** context plan (structured selection), confidence

**Runtime uses output deterministically** to assemble prompt/context.

---

### 2) PlanningSignature → PlanIR

**Purpose:** Produce a structured plan with stable step IDs.

* **Outputs:** PlanIR (or a lossless adapter to PlanIR), including:

  * `steps[]` with `id`, `intent`, `target_files`, `depends_on`, `max_iterations`
  * `verification_strategy`
  * `confidence`

**Note:** PlanIR semantics must align with the PlanIR ADR (ADR-0009).

---

### 3) ToolCallSignature

**Purpose:** Decide whether to call a tool, which tool, with what params.

* **Inputs:** current plan step (goal + intent), tool schemas, file op history, recent context, safety constraints
* **Outputs:**

  * `tool` + `params` OR `needs_user_input` + question
  * `expected_outcome`
  * `progress_estimate`
  * `confidence`

---

### 4) ToolResultSignature

**Purpose:** Interpret tool output and produce a training label.

* **Inputs:** step goal/intent, expected outcome, tool output (or controlled excerpt + hashes)
* **Outputs:**

  * `success` (yes/partial/no)
  * `extracted_facts[]`
  * `should_continue` (bool)
  * `step_utility` in `[-1, +1]` (canonical learning label; ADR-0005)

---

## Optional Signatures (Next Tier)

Add as capability grows (these are valuable but not required for MVP loop):

* **ModelSelectionSignature:** lane/model selection under budget + privacy
* **SkillMatchSignature:** choose which skill files to load
* **BranchSummarySignature / CompactionSummarySignature:** structured summaries for long sessions
* **FailureTriageSignature:** turn verification failure into targeted repair steps

---

## Modules / Pipelines (Composition)

DSPy modules are stable compositions of signatures with control flow:

### SessionContextModule

* runs ContextSelectionSignature
* incorporates summaries (compaction/branch) when available
* outputs deterministic “context ingredients”

### PlanningModule

* runs PlanningSignature (PlanIR)
* stores plan, emits PlanStart in replay

### ToolExecutionModule

* runs ToolCallSignature
* runtime executes tool
* runs ToolResultSignature
* records labels + facts + replay events

### VerificationModule

* runtime-owned execution of tests/build
* emits Verification events + receipt fields
* feeds outcome back into scoring/counterfactuals

---

## Telemetry, Replay, and Receipts

### Replay

* Runtime emits `REPLAY.jsonl` v1 per `crates/dsrs/docs/REPLAY.md`
* At minimum, capture:

  * SessionStart/End
  * PlanStart
  * ToolCall/ToolResult (with `step_id`, hashes)
  * Verification (with `verification_delta`)

### Receipts

* Runtime emits `RECEIPT.json` per `crates/dsrs/docs/ARTIFACTS.md`
* Must include `policy_bundle_id`, hashes, verification evidence, and relevant payment proofs when applicable

### Callbacks

* Use dsrs callback contract (ADR-0017):

  * required events must be emitted
  * non-blocking
  * external emission must be redacted

---

## Policy Bundles and Optimization

* Every session must attribute behavior to a `policy_bundle_id` (ADR-0015).
* Decision points must be confidence-gated with counterfactual logging (ADR-0010):

  * record DSPy output, legacy output, and used output
  * support shadow mode

Optimization loop:

1. collect labeled traces (tool calls + step_utility + verification outcomes)
2. compile updated policies into a new bundle
3. roll out via shadow → promoted with rollback capability

---

## Failure Handling (Normative Expectations)

* If a signature output is invalid/unparseable:

  * runtime falls back to legacy/default behavior (and logs fallback reason)
* If tool execution fails:

  * runtime applies tool retry policy (bounded) and records failure
* If verification fails:

  * runtime may re-enter planning/execution (or invoke failure triage if present)
  * do not “declare success” without verification when required

---

## Test Strategy: What Must Be Verifiable

This loop is only “real” if tests enforce its contracts:

Minimum invariants to test:

* **Replay contract:** emitted events validate against `REPLAY.md`
* **Receipt contract:** `RECEIPT.json` validates against `ARTIFACTS.md`
* **Hash invariants:** canonicalization + full-output hashing
* **Tool enforcement:** schema validation happens before execution; invalid params are rejected
* **Step utility range:** `step_utility ∈ [-1, +1]`, normalized is derived only
* **Policy attribution:** every session has `policy_bundle_id`
* **Session storage layout:** written under `${OPENAGENTS_HOME}` layout (ADR-0008)
* **Privacy defaults:** swarm dispatch uses default privacy policy and rejects violations (ADR-0016)

(How you structure the test harness is a separate implementation doc; this spec just sets what must be true.)

---

## Implementation Notes (Non-Canonical Pointers)

Typical code homes (may evolve; code is truth):

* Signatures: `crates/dsrs/src/signatures/*`
* Runtime loop: `crates/adjutant/src/autopilot_loop.rs` (or equivalent)
* Tools + registry: runtime-owned tool system
* Replay writer/exporter: autopilot-core replay + exporter to `REPLAY.jsonl`
* Policy bundles: dsrs compiler/optimizer artifacts + selection logic

---

## Summary

A long-running coding agent becomes robust and optimizable when:

* every “choice” is a **DSPy signature** with typed inputs/outputs,
* every “action” is a **runtime-enforced tool execution** with receipts/replay,
* every session produces a **Verified Patch Bundle**,
* every decision is **confidence-gated** and attributable to a **policy_bundle_id**.

That’s the core loop. Everything else (skills, UI, multi-agent, marketplace jobs) composes cleanly on top.
