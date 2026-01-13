# ADR-0005: Step Utility Semantics

## Status

**Accepted**

## Date

2026-01-13

## Context

OpenAgents uses a "step utility" signal to measure the usefulness of individual tool calls for learning and optimization. This ADR establishes the canonical semantics.

## Decision

**`step_utility` is the canonical learning signal with range -1.0 to +1.0**

### Field Definitions

| Field | Range | Purpose | Source |
|-------|-------|---------|--------|
| `step_utility` | -1.0 to +1.0 | Raw learning signal | `ToolResultSignature` in dsrs |
| `step_utility_norm` | 0.0 to 1.0 | Normalized for display/UI | Derived: `(step_utility + 1) / 2` |

### Interpretation

| Value | Meaning |
|-------|---------|
| +1.0 | Maximally useful — tool call directly advanced the task |
| +0.5 | Moderately useful — contributed to progress |
| 0.0 | Neutral — no clear positive or negative impact |
| -0.5 | Moderately harmful — wasted tokens or caused issues |
| -1.0 | Maximally harmful — tool call set back progress significantly |

### Canonical Source

The `ToolResultSignature` in `crates/dsrs/src/signatures/` defines the authoritative schema:

```rust
pub struct ToolResultSignature {
    pub step_utility: f64,  // -1.0 to +1.0
    // ...
}
```

### Usage Guidelines

- **Storage/logs**: Always use `step_utility` (raw)
- **Display/UI**: May use `step_utility_norm` for 0-100% visualization
- **Aggregation**: Use raw values for sAPM calculation: `APM × 1{verified_success}`
- **Training**: Use raw values directly

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
- Existing code using 0-1 range needs updating
- UI code needs conversion layer

**Neutral:**
- Two representations (raw and normalized) for different contexts

## Alternatives Considered

1. **0 to 1 as canonical** — Simpler but loses negative signal for harmful actions.

2. **0 to 100 integer** — More UI-friendly but loses precision.

3. **Categorical (good/neutral/bad)** — Too coarse for training.

## References

- [GLOSSARY.md](../../GLOSSARY.md) — `step_utility`, `verification_delta`, `sAPM`
- [crates/dsrs/docs/SIGNATURES.md](../../crates/dsrs/docs/SIGNATURES.md) — Signature definitions
- [crates/dsrs/docs/OPTIMIZERS.md](../../crates/dsrs/docs/OPTIMIZERS.md) — Scorer implementation
