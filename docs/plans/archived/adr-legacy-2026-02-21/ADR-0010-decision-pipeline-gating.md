# ADR-0010: Decision Pipeline Gating and Counterfactual Recording

## Status

**Accepted**

## Date

2026-01-13

## Context

OpenAgents is migrating routing decisions (complexity/delegation/RLM trigger/etc.) from heuristics to DSPy pipelines. This requires safety:
- confidence gating,
- fallbacks,
- a consistent record of what would have happened under legacy rules,
- support for shadow/canary rollouts.

Without counterfactuals, we cannot quantify regressions or train decision policies reliably.

## Decision

**All routing decision points MUST be confidence-gated and MUST record counterfactuals (DSPy vs legacy vs used).**

### Canonical Owner

- Decision record type: `crates/adjutant/src/dspy/sessions.rs` (or a shared crate if moved)
- Terminology: [GLOSSARY.md](../GLOSSARY.md) (`policy_bundle_id`, lane classes)
- Implementation status: [SYNTHESIS_EXECUTION.md](../SYNTHESIS_EXECUTION.md)

### Gating rules (Normative)

For any decision signature that overrides behavior:
- Output MUST include `confidence: f32`
- If `confidence < threshold` OR output fails parsing/validation:
  - execute **legacy** decision
  - record reason for fallback
- If `confidence >= threshold`:
  - execute **DSPy** decision

Thresholds:
- default threshold is configurable, but the *effective threshold used* MUST be recorded per decision.

### Counterfactual recording (Normative)

Every decision record MUST store:
- `decision_type` (e.g., complexity, delegation, rlm_trigger, economic_routing)
- `policy_bundle_id`
- `dspy_output` (structured)
- `legacy_output` (structured)
- `used_output` (structured)
- `used_source` (`dspy|legacy|shadow`)
- `confidence`
- `fallback_reason` (if used_source != dspy)
- `timestamp`, `session_id`

Shadow mode:
- `used_source = shadow`
- legacy output is executed
- DSPy output is logged only

Canary mode:
- policy chooses used_source probabilistically by rollout config
- still logs all three (dspy/legacy/used)

## Scope

What this ADR covers:
- confidence gating rules
- required counterfactual fields
- shadow/canary semantics

What this ADR does NOT cover:
- optimizer training recipes
- exact decision signature fields for each pipeline
- UI presentation of counterfactuals

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Confidence field | Required for override decisions |
| Used decision provenance | Always recorded (`used_source`) |
| Counterfactuals | Always record DSPy + legacy outputs |
| Bundle identity | Always `policy_bundle_id` (not policy_version) |

Backward compatibility:
- Adding optional fields to DecisionRecord allowed.
- Changing required fields requires superseding ADR + migration.

## Consequences

**Positive:**
- Safe migration path from heuristics to DSPy
- Enables offline evaluation ("DSPy would have done X")
- Enables canary/rollbacks for policy bundles

**Negative:**
- More logging/storage
- Requires legacy decision implementations to remain available during migration

**Neutral:**
- Shadow-mode can be default for new decision types until stable

## Alternatives Considered

1. **DSPy always-on** — rejected (too risky).
2. **No counterfactuals** — rejected (no regression measurement).
3. **Log only used decision** — rejected (no baseline comparison).

## References

- [GLOSSARY.md](../GLOSSARY.md) — terminology
- `crates/adjutant/src/dspy/decision_pipelines.rs` — decision pipelines
- `crates/adjutant/src/dspy/sessions.rs` — session records
- [ROADMAP.md](../ROADMAP.md) — shadow/canary MVP gating
