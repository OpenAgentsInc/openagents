# Advanced Operator Program Matrix

> Status: canonical `PLIB-220` / `#3735` reference record, updated 2026-03-16
> after landing the first bounded advanced operator-program matrix in
> `crates/psionic/psionic-ir/src/lib.rs`.

This document records the current bounded advanced operator-family program
surface for Psionic.

## Canonical Runner

Run the advanced-operator harness from the repo root:

```bash
scripts/release/check-psionic-advanced-operator-programs.sh
```

## What Landed

`psionic-ir` now exposes:

- `GraphBuilder::linalg_gram_matrix(...)`
- `GraphBuilder::signal_naive_dft(...)`
- `GraphBuilder::attention_rotary_residual_block(...)`
- `GraphBuilder::distribution_categorical_program(...)`
- `GraphBuilder::special_function_program(...)`
- `AdvancedOperatorProgramMatrixReport`
- `builtin_advanced_operator_program_matrix_report()`

## Current Honest Posture

Today Psionic has a first bounded advanced operator-family program layer, but
it does **not** claim full PyTorch-class breadth across all advanced families.

The bounded seeded surface now makes these seams explicit:

- linalg-family gram-matrix programs above the compact core
- signal or FFT-style Fourier projection programs above the compact core
- attention-family programs that compose RoPE, scaled dot-product attention,
  and residual addition
- explicit backend-capability refusal when those attention programs are asked
  to run without the required backend kernels
- explicit refusal for distribution and special-function families that still
  require normalization, sampling, or special-function primitives

## Why This Matters

This report prevents two failure modes:

- implying the current operator matrix already covers advanced family programs
- hand-waving missing distribution or special-function support as if those
  families were already implemented somewhere else in the stack

The point of this issue is to make bounded advanced-family semantics
machine-legible and reusable while leaving the still-missing families explicit.
