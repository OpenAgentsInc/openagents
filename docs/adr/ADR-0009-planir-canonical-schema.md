# ADR-0009: PlanIR Canonical Schema and Unification

## Status

**Accepted**

## Date

2026-01-13

## Context

OpenAgents currently has multiple plan representations across crates (e.g., Adjutant subtasks JSON vs Autopilot implementation_steps Vec<String>). This causes:
- fragmented training data,
- inconsistent replay/receipts (no stable step IDs),
- duplicated glue code and evaluation.

We need a single plan intermediate representation that:
- is stable enough to log/replay/train against,
- supports step-level receipts/events,
- works across planners (Adjutant, Autopilot, RLM/FRLM).

## Decision

**PlanIR is the canonical intermediate representation for plans across OpenAgents. All planners MUST emit PlanIR (or a lossless adapter to it).**

### Canonical Owner

- **Spec definition:** [docs/dse/SIGNATURES.md](../dse/SIGNATURES.md) (planning signatures define PlanIR shape)
- **Target code location:** `crates/dsrs/src/ir/plan.rs` (not yet implemented — spec-only status)
- **Current implementations:**
  - Adjutant: `SubtaskPlanningSignature.subtasks` → maps to `PlanIR.steps`
  - Autopilot: `PlanningSignature.implementation_steps` → maps to `PlanIR.steps`
- Canonical terminology: [GLOSSARY.md](../GLOSSARY.md) (`PlanIR`, `StepIntent`, `verification_delta`)
- Execution/replay integration: [docs/execution/REPLAY.md](../execution/REPLAY.md) (step_id linkage)

### PlanIR (Normative shape)

PlanIR MUST include:
- `analysis: String`
- `steps: Vec<PlanStep>`
- `verification_strategy: VerificationStrategy`
- `complexity: Complexity`
- `confidence: f32`

PlanStep MUST include:
- `id: String` (stable within session; referenced by replay/receipt)
- `description: String`
- `intent: StepIntent` (Investigate | Modify | Verify | Synthesize)
- `target_files: Vec<String>`
- `depends_on: Vec<String>`
- `max_iterations: u8`

VerificationStrategy MUST include:
- `commands: Vec<String>`
- `success_criteria: String`
- `max_retries: u8`

### Compatibility rule

- Older plan formats are allowed internally, but they MUST be converted to PlanIR for:
  - replay emission (`PlanStart.step_count`, `ToolCall.step_id`)
  - receipt fields (`plan_hash` when present)
  - training datasets (plan quality metrics)

## Scope

What this ADR covers:
- PlanIR as canonical IR
- Minimum required fields and semantics
- Step ID semantics (what "step-1" means)

What this ADR does NOT cover:
- Planner choice/routing (decision pipelines)
- Tool execution signatures (ToolCall/ToolResult)
- Scheduling/parallel execution engine behavior

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Plan name | Canonical term: `PlanIR` |
| Step identity | `PlanStep.id` is stable within a session |
| Intent enum | `StepIntent` values are stable |
| Replay linkage | `ToolCall.step_id` references `PlanStep.id` |
| Verification delta | Computed as `tests_before - tests_after` |

Backward compatibility:
- Adding new optional fields to PlanIR is allowed.
- Renaming/removing required fields requires a superseding ADR + migration.

## Consequences

**Positive:**
- Unified training/eval surface for planning quality
- Replay/receipt can reference stable step IDs
- Less duplicated glue between Adjutant and Autopilot

**Negative:**
- Requires adapters/migrations from existing plan outputs

**Neutral:**
- PlanIR versioning may be needed later (if/when fields expand)

## Alternatives Considered

1. **Keep multiple plan formats** — rejected (data fragmentation and replay ambiguity).
2. **Use freeform textual plans only** — rejected (not machine-actionable).
3. **Use a protocol-level PlanIR** — deferred; PlanIR is internal IR first.

## References

- [GLOSSARY.md](../GLOSSARY.md) — `PlanIR`, `StepIntent`
- [docs/execution/REPLAY.md](../execution/REPLAY.md) — step_id linkage
- `crates/adjutant/src/dspy/module.rs` (current planning outputs)
- `crates/autopilot-core/docs/EXECUTION_FLOW.md` (legacy plan flow)
