---
id: method.baseline-floating-point-reproducibility
version: 1
kind: method
title: Preserve floating-point operation order when matching a reference
summary: >-
  When a numerical implementation must match an existing scalar reference
  closely, preserve its loop and reduction order and avoid compiler
  transformations that reassociate or fuse operations; validate both numerical
  agreement and performance under the actual build flags.
tags: [floating-point, compiler, numerical-accuracy, optimization]
applies_when: >-
  Porting a floating-point algorithm from an interpreted reference into
  optimized native code with stringent equivalence tolerances.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - portfolio-optimization
  cites:
    - IEEE, IEEE Standard for Floating-Point Arithmetic (IEEE 754-2019), §5.4.1
    - GCC, Options That Control Optimization, “Options That Control Floating-Point Behavior”
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Floating-point addition and multiplication are rounded, so algebraically equivalent expressions can produce different results when reductions are reordered, vectorized, reassociated, or fused into multiply-add instructions. To reproduce a reference, first mirror its iteration and accumulation order. Avoid fast-math options that permit reassociation or assumptions about NaNs and infinities; where needed, disable contraction so multiply-add fusion does not change rounding. These restrictions are a compatibility choice, not a universal performance rule: measure the resulting implementation and relax them only if the required numerical contract permits it.

Source: IEEE, *IEEE Standard for Floating-Point Arithmetic (IEEE 754-2019)*, §5.4.1 (rounding of basic operations); GCC, *Options That Control Optimization*, sections “Options That Control Floating-Point Behavior” (`-ffast-math`, `-ffp-contract`).

## How to check

Compare native output against the reference on representative and edge-case inputs under the production compiler flags, using the required absolute/relative tolerance (or exact equality only when justified). Benchmark after changing compiler flags, since disabling transformations can affect speed independently of algorithmic correctness.
