# Autocast Precision Policy

> Status: canonical `PLIB-213` / `#3728` reference record, updated 2026-03-16
> after landing the first bounded autocast-style precision-policy matrix in
> `crates/psionic/psionic-core/src/lib.rs`.

This document records the current bounded autocast semantics surface for
Psionic.

## Canonical Runner

Run the autocast harness from the repo root:

```bash
scripts/release/check-psionic-autocast-precision-policy.sh
```

## What Landed

`psionic-core` now exposes:

- `AutocastOperationFamily`
- `AutocastPrecisionPolicy`
- `AutocastNumericsDiagnostic`
- `AutocastPolicyStatus`
- `AutocastPolicyResolution`
- `AutocastPolicyMatrixReport`
- `builtin_autocast_policy_matrix_report()`

## Current Honest Posture

Today Psionic has a first-class typed autocast policy surface, but it does
**not** claim broad mixed-precision runtime closure.

The bounded seeded surface now makes these seams explicit:

- backend-aware policy resolution over the current runtime-backend vs
  meta-execution split
- explicit low-precision rules for seeded matmul, pointwise, and reduction
  families
- machine-readable numerics diagnostics for reduced mantissa, reduced dynamic
  range, FP32 accumulation, stability-preserving refusal to downcast, and
  experimental low-precision posture
- typed refusal when the bounded runtime surface cannot safely realize complex
  inputs, unsupported float8 preferences, or undeclared operator families

## Why This Matters

This matrix prevents two failure modes:

- claiming "mixed precision support" just because a low-precision dtype exists
- silently ignoring unsafe autocast requests instead of surfacing an explicit
  refusal or stability-preserving rule

The point of this issue is to make bounded precision-policy behavior
machine-legible so later grad-scaling, quantization, and export work can build
on one explicit contract.
