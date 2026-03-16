# Semantics Claim Report

> Status: canonical `PLIB-209` / `#3724` reference record, updated 2026-03-16
> after adding autocast precision-policy evidence from `PLIB-213` / `#3728`
> into `crates/psionic/psionic-compat/src/lib.rs`.

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
- bounded program-transform capability coverage
- bounded extension-contract coverage
- bounded local data-ingress coverage
- tensor-family capability and refusal coverage
- advanced dtype promotion, cast, and backend-capability coverage
- reproducibility seed, generator-derivation, and checkpoint-restore coverage
- autocast-style precision-policy coverage with numerics diagnostics
- train-class gradient-scaling coverage with explicit overflow and underflow handling
- bounded quantization capability coverage above raw decode
- module and `state_dict` parity
- optimizer step parity
- fake-tensor and compiler-hygiene parity

But those artifacts are still seed-sized and explicitly bounded.

The report keeps broader future targets marked `pytorch_compatible_later`,
including:

- broader mixed-precision runtime systems beyond the current seeded fp16 and bf16 train window
- export-safe graph compatibility beyond the current quantization seed coverage
- distributed-training data-feed semantics beyond the current local ingress seed coverage
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
