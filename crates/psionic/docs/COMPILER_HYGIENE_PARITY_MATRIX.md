# Compiler Hygiene Parity Matrix

> Status: canonical `PLIB-208` / `#3723` reference record, updated 2026-03-16
> after landing the first seeded symbolic-shape, fake-tensor, and
> compiler-hygiene parity matrix in
> `crates/psionic/psionic-compiler/src/lib.rs`.

This document records the first bounded compiler-hygiene parity artifact for
Psionic's semantics layer.

## Canonical Runner

Run the parity harness from the repo root:

```bash
scripts/release/check-psionic-compiler-hygiene-parity.sh
```

## What Landed

`psionic-compiler` now exposes:

- `CompilerHygieneParityCaseResult`
- `CompilerHygieneParityMatrixReport`
- `builtin_compiler_hygiene_parity_matrix_report()`

The current report is intentionally seed-sized and PyTorch-derived rather than
claiming full compiler-stack closure.

## Seeded Cases

The current matrix covers:

- fake-tensor graph-vs-plan meta-execution output parity for a broadcast-plus-
  reduction graph
- non-dense fake/meta tensor contracts for sparse and storage-aware declared
  outputs
- compiler cache-temperature and cache-action hygiene for cold compile then
  warm reuse
- alias-aware memory-planning hygiene for view lowering and fusion grouping
- explicit refusal for symbolic-shape and guard-environment parity, because the
  current bounded substrate still requires concrete `usize` dimensions in
  `TensorSpec`

## Why This Is Bounded

This matrix is a seed, not a blanket "PyTorch compiler-compatible" claim.

Current scope is intentionally limited to:

- fake/meta execution parity for a small graph slice
- non-dense fake-tensor contract validation
- cache-temperature, alias-view, and replay-safe compiler hygiene signals
- one explicit refusal-path proof for symbolic-shape absence

It does not yet claim:

- symbolic-shape environments or guard simplification
- Dynamo/AOTAutograd/Inductor-class compiler closure
- broad transform or export parity
- dynamic-shape specialization or recompilation heuristics

The point of this issue is to make support and refusal posture machine-legible,
repeatable, and expandable without silent skips.
