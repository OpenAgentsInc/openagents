# Gradient Scaling Semantics

> Status: canonical `PLIB-214` / `#3729` reference record, updated 2026-03-16
> after landing the first bounded train-class gradient-scaling report in
> `crates/psionic/psionic-train/src/mixed_precision.rs`.

This document records the current bounded gradient-scaling semantics surface
for Psionic.

## Canonical Runner

Run the gradient-scaling harness from the repo root:

```bash
scripts/release/check-psionic-gradient-scaling-semantics.sh
```

## What Landed

`psionic-train` now exposes:

- `GradientScalingMode`
- `GradientScalingSignal`
- `GradientScalingDiagnostic`
- `TrainingGradientScalingPolicy`
- `GradientScalingDecision`
- `GradientScalingCaseResult`
- `GradientScalingSemanticsReport`
- `builtin_gradient_scaling_semantics_report()`

## Current Honest Posture

Today Psionic has a first-class typed train-class gradient-scaling surface, but
it does **not** claim broad mixed-precision training closure across all
backends or precision families.

The bounded seeded surface now makes these seams explicit:

- dynamic loss scaling for the current fp16 train path
- overflow handling that backs off the scale and skips the optimizer step
- underflow handling that grows the scale instead of silently accepting
  vanishing gradients
- an explicit bf16 no-scaling posture
- typed refusal when the bounded surface lacks fp32 master weights or receives
  unsupported gradient precisions

## Why This Matters

This report prevents two failure modes:

- claiming "mixed-precision training works" because autocast exists while loss
  scaling remains implicit
- silently masking overflow or underflow behavior inside one trainer loop
  instead of publishing a reusable contract

The point of this issue is to make train-class mixed-precision step behavior
machine-legible so later quantization, distributed data-feed, and export work
can build on one explicit contract.
