---
id: method.validate-translated-stan-posterior
version: 1
kind: method
title: Validate a translated Stan model against its source posterior
summary: >-
  When porting a Stan model between interfaces or languages, check the
  mathematical target independently of successful compilation and sampling by
  comparing fixed-parameter log densities and validating retained-draw
  summaries against exported outputs.
tags: [stan, mcmc, model-validation, reproducibility]
applies_when: >-
  A Stan model is translated between interfaces, sampling controls are
  remapped, or posterior draws are exported to files for downstream use.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - rstan-to-pystan
  cites:
    - "Evans and Rosenthal, Probability and Statistics: The Science of Uncertainty, multivariate normal distributions section."
    - "Carpenter et al., Stan: A Probabilistic Programming Language, section 2.3, Hamiltonian Monte Carlo."
    - Stan Reference Manual, Hamiltonian Monte Carlo and Constrained Parameter Transforms sections.
    - PyStan 3 documentation, model sampling and fit API sections.
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

Successful compilation and plausible posterior means do not establish that a translated model targets the same posterior. Validate in layers:

1. Compare source and translated model blocks, including covariance construction, likelihood factorization, priors, generated quantities, and parameter constraints. Preserve easily overlooked details such as transpose orientation in triangular solves and whether reported covariance includes observation noise or jitter.
2. At several fixed valid parameter values, independently evaluate the log posterior. For a Gaussian likelihood with covariance \(K=LL^\top\), the log density includes \(-\sum_i \log L_{ii}-\frac12\lVert L^{-1}(y-\mu)\rVert^2\), in addition to all prior terms and any unconstrained-coordinate Jacobian requested by the interface. Compare like with like: transformed-space and constrained-space log densities differ by the transform Jacobian.
3. Map sampling controls explicitly across interfaces. In particular, distinguish total iterations from warmup and retained samples, and distinguish per-chain draw counts from combined draws. Do not claim bitwise parity merely because seeds and controls match: Stan version, RNG implementation, compiler, and platform can affect draws.
4. After sampling, check diagnostics (including divergences and tree-depth saturation), expected parameter dimensions, finiteness and constraints, and output schema. Compare each exported summary with a fresh computation from the exact retained draws; this catches flattening, row/column orientation, and mismatched-sample errors.

The normal density is defined by M. Evans, J. Rosenthal, *Probability and Statistics: The Science of Uncertainty*, section on multivariate normal distributions. HMC/NUTS and Stan's constraint transforms are documented in Carpenter et al., “Stan: A Probabilistic Programming Language,” section 2.3, and the Stan Reference Manual, sections “Hamiltonian Monte Carlo” and “Constrained Parameter Transforms.” Interface-specific iteration and log-density/Jacobian behavior should be checked in the official documentation for the Stan interface and version being used.

## How to check

For a fitted draw array whose parameter axis is first, assert that exported means exactly match recomputation:

```python
import numpy as np

draws = np.load(draws_path)            # retained draws, parameter axis first
exported = load_exported_summary()     # the file(s) the task asks for, parsed back from disk
for name, table in exported.items():
    np.testing.assert_allclose(table, draws[name].mean(axis=-1))
    assert np.isfinite(table).all()
```

Also compare independently computed constrained-parameter log densities at multiple fixed points, and inspect chain diagnostics rather than relying on the summary check alone.
