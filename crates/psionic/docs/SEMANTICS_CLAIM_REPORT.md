# Semantics Claim Report

> Status: canonical `PLIB-209` / `#3724` reference record, updated 2026-03-16
> after landing the first machine-readable PyTorch-facing semantics claim
> report in `crates/psionic/psionic-compat/src/lib.rs`.

This document records the current honest claim boundary for Psionic's
PyTorch-facing semantics layer.

## Canonical Runner

Run the claim-report harness from the repo root:

```bash
scripts/release/check-psionic-semantics-claim-report.sh
```

## What Landed

`psionic-compat` now exposes:

- `SemanticsClaimPosture`
- `SemanticsClaimArea`
- `SemanticsClaimReport`
- `builtin_semantics_claim_report()`

The report aggregates the current seeded parity artifacts and separates three
postures:

- `pytorch_credible`
- `seeded_evidence_only`
- `pytorch_compatible_later`

## Current Honest Posture

Today the overall semantics layer is **not** marked `pytorch_credible`.

The current overall posture is `seeded_evidence_only` because Psionic now has
machine-readable evidence for:

- operator parity
- module and `state_dict` parity
- optimizer step parity
- fake-tensor and compiler-hygiene parity

But those artifacts are still seed-sized and explicitly bounded.

The report keeps broader future targets marked `pytorch_compatible_later`,
including:

- advanced tensor, dtype, and precision systems
- quantization and export-safe graph compatibility
- dataset and distributed-training semantics
- extension and plugin contracts
- advanced operator-family breadth

## Why This Matters

This report prevents two failure modes:

- claiming `PyTorch-credible` too early because a few seeded parity matrices
  exist
- letting "compatible later" remain vague instead of tying it to concrete
  blockers and open issue references

The point of this issue is to make the claim vocabulary itself machine-legible,
versioned, and testable.
