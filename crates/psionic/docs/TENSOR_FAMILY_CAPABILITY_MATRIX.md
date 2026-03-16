# Tensor Family Capability Matrix

> Status: canonical `PLIB-210` / `#3725` reference record, updated 2026-03-16
> after landing the first machine-readable capability and refusal matrix for
> dense, sparse, nested, masked, and storage-aware tensor families in
> `crates/psionic/psionic-ir/src/lib.rs`.

This document records the current bounded tensor-family semantics surface for
Psionic.

## Canonical Runner

Run the matrix harness from the repo root:

```bash
scripts/release/check-psionic-tensor-family-capability-matrix.sh
```

## What Landed

`psionic-ir` now exposes:

- `TensorFamilyCapabilitySurface`
- `TensorFamilyCapabilityStatus`
- `TensorFamilyCapabilityCaseResult`
- `TensorFamilyCapabilityMatrixReport`
- `builtin_tensor_family_capability_matrix_report()`

## Current Honest Posture

Today Psionic treats dense tensors as the only family that is fully
materializable by the current runtime.

Sparse, nested, masked, and storage-aware families are now first-class typed
contracts, but their support is explicitly bounded:

- they can flow through meta execution
- they can be declared by custom operators as explicit output families
- their contracts serialize as stable graph/meta metadata
- storage-aware posture is currently limited to dense-layout alias and
  broadcast view semantics
- non-dense runtime materialization is refused explicitly instead of being left
  ambiguous

## Why This Matters

This matrix prevents two failure modes:

- pretending non-dense families already have real runtime support because the
  meta layer can describe them
- re-inventing sparse or masked family vocabulary independently in downstream
  crates

The matrix gives Psionic one machine-readable tensor-family truth surface that
future dtype, autocast, quantization, and export work can extend.
