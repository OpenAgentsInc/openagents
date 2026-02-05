# ADR-0005: Step Utility Semantics

## Status

**Accepted**

## Date

2026-01-13

## Context

OpenAgents uses a "step utility" signal to measure the usefulness of individual tool calls for learning and optimization. We need canonical semantics for this signal to ensure consistent interpretation across signatures, metrics, and training pipelines.

## Decision

**`step_utility` is the canonical learning signal with range -1.0 to +1.0**

### Field Definitions

| Field | Range | Purpose | Source |
|-------|-------|---------|--------|
| `step_utility` | -1.0 to +1.0 | Canonical learning signal | `ToolResultSignature` |
| `step_utility_norm` | 0.0 to 1.0 | Normalized for display/UI/helper judges | Derived or judge output |

### Interpretation

| Value | Meaning |
|-------|---------|
| +1.0 | Maximally useful — tool call directly advanced the task |
| +0.5 | Moderately useful — contributed to progress |
| 0.0 | Neutral — no clear positive or negative impact |
| -0.5 | Moderately harmful — wasted tokens or caused issues |
| -1.0 | Maximally harmful — tool call set back progress significantly |

### Canonical Source

`ToolResultSignature` in `crates/dsrs/src/signatures/` defines the authoritative output:

```rust
pub struct ToolResultSignature {
    // ...
    pub step_utility: f32,  // -1.0 to +1.0 — THE CANONICAL LABEL
}
```

### Related Signatures

`ToolStepUtilitySignature` in `crates/adjutant/src/dspy/tool_step_utility.rs` serves as a **utility judge** that outputs `step_utility_norm` (0.0 to 1.0). This is a helper/judge output, not the canonical training label.

| Signature | Output Field | Range | Role |
|-----------|--------------|-------|------|
| `ToolResultSignature` | `step_utility` | -1.0..+1.0 | **Canonical training label** |
| `ToolStepUtilitySignature` | `step_utility_norm` | 0.0..1.0 | Judge/helper output |

When both are available, `step_utility` from `ToolResultSignature` is authoritative for training and optimization.

### Usage Guidelines

- **Storage/logs**: Always use `step_utility` (raw -1..+1)
- **Display/UI**: May use `step_utility_norm` for 0-100% visualization
- **Aggregation**: Use raw values for sAPM calculation
- **Training**: Use `step_utility` (-1..+1) directly

### Conversion

```
step_utility_norm = (step_utility + 1) / 2
step_utility = (step_utility_norm * 2) - 1
```

## Scope

What this ADR covers:
- Canonical range and semantics for step_utility
- Relationship between step_utility and step_utility_norm
- Which signature is authoritative

What this ADR does NOT cover:
- How step_utility is computed (signature implementation)
- Aggregation into session-level metrics (see METRICS.md)
- Optimizer usage (see OPTIMIZERS.md)

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Field name | Stable: `step_utility` |
| Range | Stable: -1.0 to +1.0 |
| Semantics | Stable: positive = good, negative = harmful |

Backward compatibility:
- Code outputting 0-1 should be updated to use `step_utility_norm` suffix
- Training data labeled with 0-1 range needs conversion

### Related Metrics

| Metric | Formula | Range |
|--------|---------|-------|
| `verification_delta` | `tests_before - tests_after` | Integer |
| `sAPM` | `APM × 1{verified_success}` | 0+ |

The `OutcomeCoupledScorer` combines `step_utility`, `verification_delta`, repetition penalty, and schema validity into a composite score.

## Consequences

**Positive:**
- Clear, unambiguous definition
- Symmetric range makes interpretation intuitive
- Canonical source in code

**Negative:**
- Code using 0-1 range needs updating to use `_norm` suffix
- UI code needs conversion layer

**Neutral:**
- Two representations (raw and normalized) for different contexts

## Alternatives Considered

1. **0 to 1 as canonical** — Simpler but loses negative signal for harmful actions.

2. **0 to 100 integer** — More UI-friendly but loses precision.

3. **Categorical (good/neutral/bad)** — Too coarse for training.

## References

- [GLOSSARY.md](../GLOSSARY.md) — `step_utility`, `verification_delta`, `sAPM`
- [crates/dsrs/docs/SIGNATURES.md](../../crates/dsrs/docs/SIGNATURES.md) — Signature definitions
- [crates/dsrs/docs/METRICS.md](../../crates/dsrs/docs/METRICS.md) — Metric definitions
- [crates/dsrs/docs/OPTIMIZERS.md](../../crates/dsrs/docs/OPTIMIZERS.md) — Scorer implementation
