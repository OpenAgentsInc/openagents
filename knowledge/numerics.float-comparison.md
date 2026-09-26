---
id: numerics.float-comparison
version: 1
kind: edge-case
title: Comparing floating-point numbers
summary: >-
  Floating-point results rarely equal a decimal expectation exactly; compare
  with a relative and an absolute tolerance. Relative tolerance alone fails
  near zero, and float32 arithmetic carries only about seven significant
  digits.
tags: [numerics, floating-point, tolerance, isclose, allclose, float32, precision, rounding]
applies_when: >-
  Code or tests compare computed floating-point values, check a result is
  zero, or mix float32 and float64 data.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - "Goldberg, What Every Computer Scientist Should Know About Floating-Point Arithmetic, ACM Computing Surveys 23(1) (1991)"
    - "PEP 485, A Function for testing approximate equality (math.isclose)"
    - "NumPy documentation, numpy.isclose"
evidence: []
---

## Details

- `0.1 + 0.2 == 0.3` is false. Use a tolerance.
- `math.isclose(a, b)` defaults to `rel_tol=1e-9, abs_tol=0`, so comparing
  anything with 0 fails unless you set `abs_tol`.
- `np.isclose(a, b)` defaults to `rtol=1e-5, atol=1e-8` and isn't symmetric:
  the tolerance scales with `b`.
- float32 has a 24-bit significand (about 7 decimal digits). Sums of many
  float32 values drift; accumulate in float64 (`x.astype(np.float64)` or
  `np.sum(x, dtype=np.float64)`) when precision matters.
- Summation order changes the last bits; parallel or vectorized sums may
  differ from a loop.
- `nan != nan`; use `np.isnan` or `math.isnan`. Every comparison with `nan`
  is false, so a `nan` passes neither `x > t` nor `x <= t`.
- A threshold check on a value computed at the threshold (`score >= 0.5`)
  can flip on rounding; don't put test data exactly on a boundary.

## How to check

Assert with `pytest.approx`, with NumPy's `assert_allclose` given both
`rtol` and `atol`, or with `math.isclose` given both tolerances.
