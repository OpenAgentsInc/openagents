---
id: numerics.robust-numerical-optimization
version: 1
kind: method
title: Make numerical optimization reliable, then verify the optimum
summary: >-
  Scale the variables, check gradients against finite differences, pick a
  solver that handles the constraints you actually have, restart from several
  points when the objective is not convex, and verify the answer: solver
  status, constraint violation, first-order optimality, and a baseline.
tags: [optimization, scipy, gradient-check, constraints, multistart, numerics]
applies_when: >-
  Fitting parameters, designing to an objective, or tuning a system with a
  continuous optimizer, where the score depends on reaching a good optimum
  within limits.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "Nocedal and Wright, Numerical Optimization, 2nd ed. (Springer, 2006), chapters 2 (fundamentals), 12 (theory of constrained optimization), 18 (SQP)"
    - "SciPy documentation, scipy.optimize: minimize (L-BFGS-B, SLSQP, trust-constr), check_grad, least_squares, differential_evolution, basinhopping"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

1. **Formulate.** Write the objective and every constraint explicitly,
   including bounds that are only implied (non-negative widths, physical
   limits). Rescale variables to order one; badly scaled problems stall
   quasi-Newton methods. For sums of squared residuals use
   `least_squares`, which exploits that structure.
2. **Gradients.** Supply analytic or automatic gradients when you can, and
   check them with `scipy.optimize.check_grad` or a central difference at a
   few random points. A wrong gradient makes the solver report success at a
   non-optimum.
3. **Solver.** Bounds only: `L-BFGS-B`. General smooth constraints: `SLSQP`
   or `trust-constr`. Noisy or non-smooth objectives: derivative-free methods
   (`Nelder-Mead`, `Powell`, `COBYLA`), which are slower. Integer or
   combinatorial decisions: formulate as a MILP/CP model instead of rounding a
   continuous answer.
4. **Global structure.** For non-convex objectives, run from several seeded
   starting points (Latin hypercube or random within bounds) or use
   `differential_evolution` then polish the best with a local method; keep the
   best feasible result.
5. **Budget.** Profile one objective evaluation first; vectorize it or cache
   repeated sub-results before tuning solver settings.

## How to check

Read `res.success`, `res.status`, and `res.message`; compute the maximum
constraint violation yourself; check the gradient (or projected gradient,
under bounds) is near zero at the answer; perturb the solution slightly in a
few directions and confirm the objective does not improve; and compare the
objective with a simple baseline (the initial guess, a grid search, or a
known design). Rerun with a different seed to see whether the optimum is
stable.
