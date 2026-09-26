---
id: statistics.ks-two-sample
version: 1
kind: method
title: The two-sample Kolmogorov-Smirnov statistic
summary: >-
  The two-sample KS statistic is the largest absolute gap between the two
  empirical distribution functions, taken over every point of both samples.
  Evaluating at one sample's points, using a one-sided gap, or mishandling
  ties gives a wrong statistic.
tags: [statistics, ks, kolmogorov-smirnov, two-sample-test, ecdf, drift-detection, p-value, scipy]
applies_when: >-
  Code computes a KS statistic or p-value between two samples, per feature or
  per dimension, or compares empirical distribution functions.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - "Hollander, Wolfe, and Chicken, Nonparametric Statistical Methods, 3rd edition (Wiley, 2014), the two-sample Kolmogorov-Smirnov test"
    - "SciPy documentation, scipy.stats.ks_2samp"
evidence: []
---

## Details

For samples `x` (size m) and `y` (size n), with empirical distribution
functions `F_m(t) = #{x_i <= t} / m` and `G_n(t) = #{y_j <= t} / n`:

    D = sup_t |F_m(t) - G_n(t)|

The supremum is reached at a data point, so evaluate both functions at every
value of the pooled sample. With sorted arrays, `np.searchsorted(sorted_x,
t, side="right") / m` is `F_m(t)`: `side="right"` counts ties as at or
below `t`.

Common defects:

- Evaluating only at `x`'s points, or on a fixed grid, which can miss the
  largest gap.
- `side="left"`, which gives the fraction strictly below `t`.
- A one-sided gap, `max(F - G)` without the absolute value.
- Forgetting to sort before `searchsorted`.
- For many dimensions, running one test per dimension and alerting on any
  p-value under alpha without a multiple-testing correction (such as
  Bonferroni: alpha divided by the number of tests), so stable data alerts.
- Treating a small p-value as "no drift": drift is a large D or a small
  p-value.

## How to check

`D` is in [0, 1], 0 for identical samples, 1 for samples with disjoint
ranges, and matches `scipy.stats.ks_2samp(x, y).statistic` when SciPy is
available.

```python
import numpy as np

def ks_stat(x, y):
    x, y = np.sort(x), np.sort(y)
    t = np.concatenate([x, y])
    f = np.searchsorted(x, t, side="right") / len(x)
    g = np.searchsorted(y, t, side="right") / len(y)
    return float(np.max(np.abs(f - g)))
```
