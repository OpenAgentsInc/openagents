# Operator Parity Matrix

> Status: canonical `PLIB-205` / `#3720` reference record, updated 2026-03-16
> after landing the first seeded Rust-native operator parity matrix in
> `crates/psionic/psionic-ir/src/lib.rs`.

This document records the first bounded operator parity artifact for Psionic's
semantics layer.

## Canonical Runner

Run the parity harness from the repo root:

```bash
scripts/release/check-psionic-operator-parity-matrix.sh
```

## What Landed

`psionic-ir` now exposes:

- `OperatorParityCaseResult`
- `OperatorParityMatrixReport`
- `builtin_operator_parity_matrix_report()`

The current report is intentionally seed-sized and PyTorch-derived rather than
claiming full `OpInfo` closure.

## Seeded Cases

The current matrix covers:

- supported shape/dtype/device parity for `add`
- supported shape/dtype/device parity for `mul`
- supported shape/dtype/device parity for `matmul`
- supported shape/dtype/device parity for `reshape`
- supported shape/dtype/device parity for `permute`
- supported shape/dtype/device parity for `concat`
- supported rank-4 output-shape parity for `scaled_dot_product_attention`
- explicit backend-capability refusal for `rms_norm` when the meta capability
  profile does not declare that kernel

## Why This Is Bounded

This matrix is a seed, not a blanket "PyTorch-compatible" claim.

Current scope is intentionally limited to:

- deterministic meta-execution contracts
- shape/dtype/device conformance for a small operator subset
- one explicit refusal-path proof

It does not yet claim:

- full `op_db` breadth
- broad dtype/device cartesian coverage
- numerical kernel parity across backend runtimes
- export, transform, or distributed parity

The point of this issue is to make support and refusal posture machine-legible,
repeatable, and expandable without silent skips.
