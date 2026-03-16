# Module Parity Matrix

> Status: canonical `PLIB-206` / `#3721` reference record, updated 2026-03-16
> after landing the first seeded Rust-native module parity matrix in
> `crates/psionic/psionic-nn/src/lib.rs`.

This document records the first bounded module parity artifact for Psionic's
semantics layer.

## Canonical Runner

Run the parity harness from the repo root:

```bash
scripts/release/check-psionic-module-parity-matrix.sh
```

## What Landed

`psionic-nn` now exposes:

- `ModuleParityCaseResult`
- `ModuleParityMatrixReport`
- `builtin_module_parity_matrix_report()`

The current report is intentionally seed-sized and PyTorch-derived rather than
claiming full `module_db` closure.

## Seeded Cases

The current matrix covers:

- normalized `state_dict` key-set and module-tree parity for `linear`
- persistent-buffer and default `state_dict` parity for `batch_norm1d`
- normalized nested module, parameter, buffer, and all-buffer `state_dict`
  parity for a `transformer_encoder_layer`-style module tree
- explicit refusal for registration-order-preserving `state_dict` parity,
  because `psionic-nn` currently guarantees deterministic lexical traversal
  instead of PyTorch registration-order key ordering

## Why This Is Bounded

This matrix is a seed, not a blanket "PyTorch-compatible" claim.

Current scope is intentionally limited to:

- reusable module-tree and `state_dict` semantics
- normalized path-set and persistent-buffer behavior
- one explicit refusal-path proof for registration-order incompatibility

It does not yet claim:

- full `module_db` breadth
- forward numerics parity for standard modules
- registration-order-preserving module serialization
- device or dtype cartesian coverage

The point of this issue is to make support and refusal posture machine-legible,
repeatable, and expandable without silent skips.
