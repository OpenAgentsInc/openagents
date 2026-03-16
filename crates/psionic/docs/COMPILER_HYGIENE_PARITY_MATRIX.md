# Compiler Hygiene Parity Matrix

> Status: canonical `PLIB-208` / `#3723` reference record, updated 2026-03-16
> after widening the seeded symbolic-shape, fake-tensor, and
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
- one bounded shapeless trace-family identity case over same-rank primitive
  compile graphs
- explicit refusal for symbolic-shape and guard-environment parity, because the
  current bounded substrate still requires concrete `usize` dimensions in
  `TensorSpec`
- explicit refusal for reshape under `shapeless_trace_family`, because the
  current graph model still lacks symbolic output formulas for concrete reshape
  targets

## Why This Is Bounded

This matrix is a seed, not a blanket "PyTorch compiler-compatible" claim.

Current scope is intentionally limited to:

- fake/meta execution parity for a small graph slice
- non-dense fake-tensor contract validation
- cache-temperature, alias-view, and replay-safe compiler hygiene signals
- one bounded shapeless trace-family identity seed plus explicit refusal-path
  proofs for symbolic-shape absence and reshape-formula gaps

It does not yet claim:

- symbolic-shape environments or guard simplification
- Dynamo/AOTAutograd/Inductor-class compiler closure
- broad transform or export parity
- dynamic-shape specialization, guard-driven recompilation heuristics, or broad
  shapeless support over reshape, expand, and other shape-dependent graph ops

The point of this issue is to make support and refusal posture machine-legible,
repeatable, and expandable without silent skips.
