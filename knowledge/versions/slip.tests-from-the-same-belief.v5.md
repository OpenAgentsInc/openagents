---
id: slip.tests-from-the-same-belief
version: 5
kind: slip
title: Validate optimized paths against an independent reference
summary: >-
  Tests that share the implementation’s assumptions can confirm the same
  bug. Compare optimized results to a simple cold reference path, then
  separately test invariance to batching, ordering, and cache state.
tags: [testing, differential-testing, invariance, caching]
applies_when: >-
  A rewrite or optimization changes execution paths, and ordinary unit tests
  may encode the same mistaken assumptions as the implementation.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - batched-eval-parity
  cites:
    - William M. McKeeman, “Differential Testing for Software,” Digital Technical Journal 10(1), 1998
evidence: []
---

## Details

Keep a deliberately simple reference path that follows the canonical semantics without batching or caching. Compare per-record values from the optimized implementation against it, preferably using the pre-optimization implementation when available. Do not use the optimized helper functions to construct the reference result.

Then test independent invariants: reorder inputs and restore results by stable input position; run different batch sizes and padding modes; compare cold and warm cache runs; and repeat with cache contents populated in a different order. Compare floating-point values with a tolerance appropriate to the computation, while requiring exact equality for discrete outputs and schema.

Differential testing detects disagreements between implementations; it does not establish that either implementation is correct, so pair it with explicit semantic edge cases. Source: McKeeman, “Differential Testing for Software,” *Digital Technical Journal* 10(1), 1998, discussion of differential testing.
