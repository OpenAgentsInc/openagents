# Optimizer Parity Matrix

> Status: canonical `PLIB-207` / `#3722` reference record, updated 2026-03-16
> after landing the first seeded Rust-native optimizer parity matrix in
> `crates/psionic/psionic-train/src/optimizer.rs`.

This document records the first bounded optimizer parity artifact for
Psionic's semantics layer.

## Canonical Runner

Run the parity harness from the repo root:

```bash
scripts/release/check-psionic-optimizer-parity-matrix.sh
```

## What Landed

`psionic-train` now exposes:

- `OptimizerParityCaseResult`
- `OptimizerParityMatrixReport`
- `builtin_optimizer_parity_matrix_report()`

The current report is intentionally seed-sized and PyTorch-derived rather than
claiming full `optim_db` closure.

## Seeded Cases

The current matrix covers:

- single-step parity for SGD with momentum
- single-step parity for Adam
- single-step parity for AdamW with decoupled weight decay
- single-step parity for LARS trust-ratio and momentum-buffer semantics
- single-step parity for LAMB trust-ratio and Adam-moment semantics
- explicit refusal for optimizer-state kind mismatch, so bounded scope remains
  machine-legible instead of being hidden behind generic test failure

## Why This Is Bounded

This matrix is a seed, not a blanket "PyTorch-compatible" claim.

Current scope is intentionally limited to:

- reusable single-tensor optimizer-step semantics
- typed optimizer-state mutation and trust-ratio behavior
- one explicit refusal-path proof for state-kind mismatch

It does not yet claim:

- broad `optim_db` parameter-group breadth
- sparse, mixed-precision, or distributed optimizer closure
- scheduler cartesian parity coverage
- backend-specific fused-kernel equivalence

The point of this issue is to make support and refusal posture machine-legible,
repeatable, and expandable without silent skips.
