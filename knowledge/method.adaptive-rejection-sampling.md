---
id: method.adaptive-rejection-sampling
version: 1
kind: method
title: Adaptive rejection sampling with tangent hulls
summary: >-
  For univariate log-concave target densities, use piecewise tangent upper
  hulls to propose samples and a secant lower hull for squeeze acceptance,
  refining the envelope at rejected proposals. Correctness depends on
  domain-aware initialization and conservative numerical derivatives.
tags: [sampling, monte-carlo, log-concavity, numerical-methods]
applies_when: >-
  Implementing a univariate adaptive rejection sampler from a vectorized
  density or log-density, especially when the target has bounded or infinite
  support.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - adaptive-rejection-sampler
  cites:
    - Gilks, W. R. and Wild, P. (1992). Adaptive Rejection Sampling for Gibbs Sampling. Applied Statistics 41(2), sections 2–3.
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Adaptive rejection sampling (ARS) applies to univariate densities whose log density is concave on the domain. At ordered support points, evaluate the log density and its derivative; tangents form a piecewise-linear upper hull. Integrating the exponentiated hull gives a proposal distribution that can be sampled by choosing a segment proportional to its integrated mass and inverting the segment's exponential integral. A secant-based lower hull between support points provides a squeeze test, avoiding target evaluations for many proposals. If the squeeze is inconclusive, evaluate the target, accept against the upper hull, and add rejected proposals as new support points before rebuilding the envelope. See Gilks and Wild, “Adaptive Rejection Sampling for Gibbs Sampling,” *Applied Statistics* 41(2) (1992), sections 2–3.

Represent infinite domain endpoints explicitly and clip initialization points to the interior. Verify that the initial support points are ordered, have finite log-density values, and produce derivatives with the sign/geometry needed for integrable tails. Handle one-sided and finite domains separately when computing segment integrals and intersections. Keep computations in log space where possible; normalize segment weights stably, and use numerically stable exponential-integral formulas when a tangent slope is near zero.

For a density-only interface, transform positive values to log density before constructing hulls. Numerical finite differences require scale-aware steps and one-sided treatment near boundaries; when cancellation makes a derivative implausibly small, cautiously enlarge the step while remaining in-domain. During refinement, check the target against the proposed upper hull and against concavity-compatible bounds, and fail clearly on encountered violations rather than silently producing samples from an invalid envelope. Such checks are diagnostics, not a proof of global log-concavity.

## How to check

Use a seeded normal target on the whole real line and an exponential target on a nonnegative half-line. Check finiteness, support, empirical CDF distance, and moments against their analytic values; also test a truncated target and an unnormalized positive rescaling. Exercise a small-rate exponential to expose finite-difference cancellation:

```r
set.seed(1)
x <- ars(function(z) dexp(z, rate = 1e-5), c(0, Inf), n = 10000)
stopifnot(all(is.finite(x)), all(x > 0), abs(mean(x) * 1e-5 - 1) < 0.1)
```

A separate test should verify that a visibly non-log-concave target is rejected when the violating region is encountered, without claiming this establishes global concavity.
