---
id: numerics.float-comparison
version: 2
kind: edge-case
title: Compare floating-point results with scale- and execution-aware tolerances
summary: >-
  A small maximum error can still fail default elementwise closeness near
  zero, while parallel reduction order or thread count can change results
  slightly. Validate with explicit tolerances and inspect error scale and
  repeatability rather than relying on one default comparison.
tags: [floating-point, validation, tolerances, reproducibility]
applies_when: >-
  Comparing model outputs or numerical results across implementations,
  devices, thread counts, or reduction orders.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - mp-checkpoint-consolidation
  cites:
    - PyTorch documentation, “torch.allclose.”
    - Goldberg, “What Every Computer Scientist Should Know About Floating-Point Arithmetic,” Section 4, “Exceptions, Rounding, and Ulps.”
evidence: []
---

## Details

PyTorch defines `allclose` elementwise using `|actual - expected| <= atol + rtol * |expected|`; consequently, relative tolerance contributes little when the reference value is near zero. A maximum absolute error alone does not show whether errors are concentrated near zero or large relative to the values being compared. Report the comparison tolerances, maximum absolute error, and a scale-aware error measure, and inspect where the largest errors occur.

Floating-point addition is not associative, so different reduction orders or execution configurations can produce small output differences even with identical weights and inputs. Establish tolerances appropriate to the dtype, computation, and execution environment. Repeat the same computation under relevant configurations to estimate ordinary numerical variation. Do not treat matching argmax or another downstream decision as proof that the underlying tensors are close.

PyTorch documentation, “torch.allclose,” specifies the elementwise tolerance rule. Goldberg, “What Every Computer Scientist Should Know About Floating-Point Arithmetic,” Section 4, discusses rounding error and the effects of floating-point operations.

## How to check

```python
import torch

err = (actual - expected).abs()
print("max absolute error:", err.max().item())
print("allclose:", torch.allclose(actual, expected, rtol=1e-5, atol=1e-6))
# Repeat with tolerances chosen for the dtype and execution context.
# Compare repeated runs too if thread or device changes can affect reductions.
```

A passing comparison should use a documented tolerance justified by the intended precision and observed reproducibility; do not silently loosen tolerances just to make a check pass.
