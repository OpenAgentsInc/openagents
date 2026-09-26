---
id: statistics.solve-distribution-constraints-by-grouping
version: 1
kind: method
title: Solve distribution constraints by grouping exchangeable outcomes
summary: >-
  When a high-dimensional probability vector is constrained symmetrically,
  assign equal probabilities within outcome groups and solve for group masses
  in low dimension. Validate the expanded vector and constraints independently
  after saving.
tags: [probability, optimization, kl-divergence, numpy]
applies_when: >-
  Constructing a categorical distribution over many outcomes subject to
  aggregate constraints whose outcomes have interchangeable roles, especially
  divergences relative to a reference distribution.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - distribution-search
  cites:
    - Cover and Thomas, Elements of Information Theory, 2nd ed., §2.3
    - SciPy documentation, scipy.special.softmax and scipy.special.log_softmax
    - SciPy documentation, scipy.optimize.root
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details
Partition outcomes into groups of sizes \(n_j\) whose members are interchangeable, and let \(m_j\) be each group's total probability mass. Set each member's probability to \(m_j/n_j\); optimize the group masses rather than all coordinates. Against a uniform reference on \(N\) outcomes, the group contributions to KL are
\[
D(P\|U)=\sum_j m_j\log\frac{m_j}{n_j/N},\qquad
D(U\|P)=\sum_j \frac{n_j}{N}\log\frac{n_j/N}{m_j}.
\]
Parameterize positive masses with softmax logits, fixing one logit to remove the additive degree of freedom, and solve the constraint residuals numerically. Use natural logarithms when the requested divergence is in nats. A solver success flag is not sufficient: verify its residuals, then expand, normalize carefully, save, reload, and recompute constraints from the persisted array.

Sources: Cover and Thomas, *Elements of Information Theory*, 2nd ed., §2.3 (relative entropy); SciPy documentation, `scipy.special.softmax` / `log_softmax` and `scipy.optimize.root`.

## How to check
```python
import numpy as np
from scipy.special import rel_entr

p = np.load(path, allow_pickle=False)
u = np.full(p.size, 1.0 / p.size)
assert p.ndim == 1 and np.isfinite(p).all() and (p > 0).all()
assert np.isclose(p.sum(), 1.0, atol=1e-12, rtol=0)
assert abs(rel_entr(p, u).sum() - target_forward) <= tolerance
assert abs(rel_entr(u, p).sum() - target_reverse) <= tolerance
```
