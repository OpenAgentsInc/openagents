---
id: method.black-box-relu-hyperplane-recovery
version: 1
kind: method
title: Recover ReLU hidden-layer directions from scalar queries
summary: >-
  For a scalar-output one-hidden-layer ReLU network, locate activation kinks
  along input-space lines and use gradient jumps to recover hidden weight
  directions. This identifies directions up to scale, sign convention,
  permutation, and inactive/cancelled units; unknown width requires a
  probabilistic discovery stopping rule.
tags: [black-box, neural-networks, relu, system-identification]
applies_when: >-
  An agent can query a piecewise-linear scalar function believed to be a
  one-hidden-layer ReLU network, but cannot access its parameters, and needs
  the first-layer weight directions.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - model-extraction-relu-logits
  cites:
    - Montúfar, Pascanu, Cho, Bengio, “On the Number of Linear Regions of Deep Neural Networks,” NeurIPS 27 (2014), Section 2
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details
A network of the form `f(x)=c+sum_i a_i max(0,w_i·x+b_i)` is affine on regions separated by the activation hyperplanes `w_i·x+b_i=0`. Restricting the oracle to a line `x=o+t v` gives a piecewise-affine scalar function. A kink at parameter `t*` occurs where a neuron activates or deactivates; the change of directional slope is `a_i (w_i·v)` (with sign determined by crossing orientation). For generic lines that cross only one hyperplane at a time, the kink location supplies a linear equation in the hyperplane normal and bias. Collecting crossings from lines with varied origins and directions permits fitting each hyperplane normal, then normalizing it to recover the weight direction.

No function-value-only method can determine the original magnitude of `w_i` separately from `a_i`: multiplying `w_i,b_i` by a positive scale and dividing `a_i` by that scale preserves the function. The neuron ordering is also arbitrary; canonical row signs are a representational convention, not recovered orientation. Units with zero output coefficient and coincident hyperplanes whose kinks cancel are unobservable. With unknown width, repeated random probe lines that yield no new directions are evidence of coverage, not a proof that all units have been found.

This method assumes a sufficiently accurate oracle and well-conditioned, distinguishable crossings. Use adaptive subdivision to find piecewise-linear departures, refine candidate kink locations, and verify each candidate with local slope estimates on both sides. Avoid treating finite differences at a fixed coarse grid as a complete detector: narrow intervals between activation points can be missed.

Source: Montúfar et al., “On the Number of Linear Regions of Deep Neural Networks,” *Advances in Neural Information Processing Systems 27* (2014), section 2, for the piecewise-linear regions induced by ReLU networks; the kink/slope-jump procedure follows directly by restricting the ReLU sum to a line.

## How to check
For each candidate crossing, query `f(o+(t*−h)v)`, `f(o+t*v)`, and `f(o+(t*+h)v)` for several decreasing `h`. The left and right secant slopes should stabilize to distinct limits. Fit normals from crossings on diverse lines, normalize them, and check that each held-out kink satisfies `abs(w_hat @ o + b_hat + t_star * (w_hat @ v))` within the kink-localization tolerance. Finally, run recovery from an independent random seed and compare direction sets by nearest-neighbor matching modulo row sign and permutation.
