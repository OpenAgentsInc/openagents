# Program Transform Capability Matrix

> Status: canonical `PLIB-216` / `#3731` reference record, updated 2026-03-16
> after landing bounded public `vmap` capability alongside the original
> program-transform matrix in `crates/psionic/psionic-ir/src/lib.rs`.

This document records the current bounded program-transform semantics surface
for Psionic.

## Canonical Runner

Run the transform harness from the repo root:

```bash
scripts/release/check-psionic-program-transform-capability.sh
```

## What Landed

`psionic-ir` now exposes:

- `ProgramTransformFamily`
- `ProgramTransformCapabilityOutcome`
- `ProgramTransformCapabilityCaseResult`
- `ProgramTransformCapabilityMatrixReport`
- `Graph::program_transform_capability(...)`
- `builtin_program_transform_capability_matrix_report()`

## Current Honest Posture

Today Psionic has a first-class program-transform capability surface, but it
does **not** claim broad higher-order transform closure.

The bounded seeded surface now makes these seams explicit:

- functionalization over graphs with explicit alias-root and barrier reporting
- symbolic-rewrite readiness over functionalized graphs
- export-safe graph handoff under `export_safe_only` policy
- public `vmap` over dense `f32` single-lane graphs with explicit cast and
  backend-extension refusal
- public `jvp` over dense `f32` primitive graphs with explicit cast and
  backend-extension refusal
- explicit future posture for `jacobian`

## Why This Matters

This matrix prevents two failure modes:

- claiming transform support based only on internal functionalization helpers
- implying broader transform closure because one graph happens to be
  "transform-safe"

The point of this issue is to make transform capability machine-legible so
export, extension, and deployment work can build on one honest contract.
