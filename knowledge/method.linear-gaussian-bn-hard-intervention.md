---
id: method.linear-gaussian-bn-hard-intervention
version: 1
kind: method
title: Sample linear-Gaussian Bayesian networks under a hard intervention
summary: >-
  Fit a linear conditional Gaussian model on each node's observed parents,
  then implement do(X=x) by fixing X and deleting its incoming structural
  equation while retaining its outgoing causal effects.
tags: [bayesian-networks, causal-inference, intervention, linear-gaussian]
applies_when: >-
  A fitted acyclic linear-Gaussian structural network must generate samples
  under a perfect intervention setting one variable to a constant.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - bn-fit-modify
  cites:
    - "Pearl, Causality: Models, Reasoning, and Inference, 2nd ed. (2009), §§3.2–3.3"
    - "Koller and Friedman, Probabilistic Graphical Models: Principles and Techniques (2009), Ch. 5, §5.2"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

Write each node's structural equation as `X_i = intercept_i + sum_j beta_ij X_j + epsilon_i`, with mutually independent zero-mean Gaussian disturbances. Fit an intercept, parent coefficients, and residual variance for each node using the observational data. To simulate a hard intervention `do(X_k = c)`, replace the structural equation for `X_k` by the constant assignment `X_k=c`; equivalently, remove every edge into `X_k` from the mutilated graph. Keep all outgoing edges and all other structural equations unchanged, so descendants respond to the intervention. Generate nodes in topological order, drawing each nonintervened disturbance independently from its fitted Gaussian distribution.

Validate graph surgery explicitly: the intervened graph is the original graph with precisely the arrows whose head is the targeted variable removed. Then verify all generated values of the intervention variable equal the assigned constant. As a distributional check, derive the implied multivariate mean and covariance from the post-intervention linear equations, or compare Monte Carlo moments against them using sampling-error-scaled tolerances; this catches errors in coefficient placement, residual-noise handling, and graph traversal.

This is a perfect (surgical) intervention, not conditioning on observational cases with `X_k` near `c`. Conditioning retains the observational mechanism for the target and generally yields a different distribution.

Sources: Pearl, *Causality: Models, Reasoning, and Inference*, 2nd ed. (2009), §§3.2–3.3; Koller and Friedman, *Probabilistic Graphical Models: Principles and Techniques* (2009), Ch. 5, §5.2.
