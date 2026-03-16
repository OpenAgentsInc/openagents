# Advanced DType Semantics

> Status: canonical `PLIB-211` / `#3726` reference record, updated 2026-03-16
> after landing the first bounded advanced-dtype semantics report in
> `crates/psionic/psionic-core/src/lib.rs`.

This document records the current bounded advanced-dtype semantics surface for
Psionic.

## Canonical Runner

Run the advanced-dtype harness from the repo root:

```bash
scripts/release/check-psionic-advanced-dtype-semantics.sh
```

## What Landed

`psionic-core` now exposes:

- `ExtendedDType`
- `ExtendedDTypeClass`
- `DTypePromotionCaseResult`
- `DTypeCastCaseResult`
- `DTypeBackendCapabilityCaseResult`
- `AdvancedDTypeSemanticsReport`
- `builtin_advanced_dtype_semantics_report()`

## Current Honest Posture

Today Psionic has a first-class advanced-dtype vocabulary, but it does **not**
claim full runtime closure for that vocabulary.

The bounded seeded surface now makes three things explicit:

- selected promotion rules across wider integers, float8, half/bfloat, and
  complex numbers
- selected cast rules, including explicit support for widening and explicit
  refusal for complex-to-real imaginary-drop paths
- backend-family capability truth showing that richer dtypes can exist in meta
  execution while current runtime backends remain bounded to the compact
  `DType` subset

## Why This Matters

This report prevents two failure modes:

- pretending "advanced dtype support" exists just because the type names
  exist
- smuggling richer dtype assumptions into later autocast, quantization, or
  export work without one typed truth surface

The point of this issue is to make richer dtype behavior machine-legible while
keeping the current runtime boundary honest.
