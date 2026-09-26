---
id: method.marginalize-hierarchical-binomial
version: 1
kind: method
title: Marginalize beta-binomial hierarchies and recover conditional latent rates
summary: >-
  For binomial groups with beta-distributed latent probabilities, integrate
  the group rates out analytically to sample only hyperparameters, then draw
  latent rates conditionally for posterior summaries. This can avoid difficult
  funnel-like geometry from sampling every group rate jointly.
tags: [bayesian, stan, hierarchical-model, mcmc]
applies_when: >-
  A model has y_i | theta_i ~ Binomial(n_i, theta_i), with independent theta_i
  ~ Beta(alpha,beta), and inference concerns alpha, beta, or posterior group
  probabilities.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - mcmc-sampling-stan
  cites:
    - Gelman et al., Bayesian Data Analysis, 3rd ed., section 5.6
    - Stan Development Team, Stan User's Guide, The Beta-Binomial Model
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Integrating out each group probability gives `y_i | alpha,beta ~ BetaBinomial(n_i, alpha,beta)`, whose log density is `lchoose(n_i,y_i) + lbeta(y_i+alpha,n_i-y_i+beta) - lbeta(alpha,beta)`. In Stan, the `beta_binomial` sampling statement expresses this marginalized likelihood. Sampling only the shared hyperparameters can improve HMC geometry and reduce dimension compared with jointly sampling all latent rates.

For posterior draws of the rates, conditional conjugacy gives `theta_i | y_i,alpha,beta ~ Beta(alpha+y_i,beta+n_i-y_i)`. Generate these in `generated quantities` if needed; do not include conditional generated draws when computing hyperparameter summaries. Validate count constraints (`0 <= y_i <= n_i`) before conversion to integer arrays.

Marginalization does not repair an improper or heavy-tailed hyperprior. Check whether posterior moments being reported exist under the specified prior and likelihood. If moments are theoretically infinite, label saved Monte Carlo averages as finite-run estimates, and do not interpret apparent chain convergence as evidence those moments exist.

Sources: Gelman et al., *Bayesian Data Analysis*, 3rd ed., section 5.6 (hierarchical models and marginalization); Stan Development Team, *Stan User's Guide*, section “The Beta-Binomial Model” (beta-binomial marginalization and conditional posterior).
