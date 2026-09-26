---
id: method.gaussian-bn-exhaustive-bic
version: 1
kind: method
title: Exhaustively recover small linear-Gaussian Bayesian networks with BIC
summary: >-
  For a small known variable set and known edge count, enumerate DAGs under
  structural constraints, score local Gaussian regressions with a decomposable
  BIC score, and retain all optima to expose Markov-equivalent solutions.
tags: [bayesian-networks, gaussian, structure-learning, bic]
applies_when: >-
  The data are continuous and approximately Gaussian, the node count is small
  enough for exhaustive DAG enumeration, and structural constraints such as a
  required root or fixed edge count are known.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - bn-fit-modify
  cites:
    - Chickering, Learning Equivalence Classes of Bayesian-Network Structures, Journal of Machine Learning Research 2 (2002), §§2–3
    - Schwarz, Estimating the Dimension of a Model, Annals of Statistics 6(2) (1978), §2
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

Represent each DAG by a topological ordering and allowed parent subsets drawn only from earlier nodes. This guarantees acyclicity and allows each node's local score to be computed once per candidate parent set. For a Gaussian network, fit each node by least squares on its parents; a common local BIC form is `n * log(RSS/n) + k * log(n)`, where `k` counts the fitted regression parameters (including the intercept). Sum local scores and retain graphs satisfying the required global edge count and other constraints.

Do not assume the best directed graph is uniquely identified. In observational data, distinct DAGs in the same Markov-equivalence class can encode the same conditional independences and often have equal scores. Keep all tied optima, compare their skeletons, and apply any stipulated orientation convention only after statistical optimization. Check the next distinct score class to assess whether the chosen model is separated from alternatives. Gaussian conditional-independence tests using partial correlations can diagnose candidate missing edges, but are a diagnostic rather than a replacement for scoring the constrained DAG space.

For a tiny graph, exhaustive search is practical and avoids heuristic-search instability. Cache each local score by child and parent set; enumerate subsets and permutations recursively rather than refitting every local regression for every DAG.

Sources: Chickering, “Learning Equivalence Classes of Bayesian-Network Structures,” *Journal of Machine Learning Research* 2 (2002), §§2–3; Schwarz, “Estimating the Dimension of a Model,” *Annals of Statistics* 6(2) (1978), §2.
