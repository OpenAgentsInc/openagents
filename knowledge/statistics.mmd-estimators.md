---
id: statistics.mmd-estimators
version: 1
kind: method
title: Biased and unbiased MMD estimators
summary: >-
  The squared maximum mean discrepancy (MMD) has a biased estimator (a
  V-statistic that averages every kernel entry, diagonal included) and
  unbiased estimators (U-statistics that leave out the within-sample
  diagonal). The biased one is positive even when both samples come from
  the same distribution.
tags: [statistics, mmd, maximum-mean-discrepancy, kernel, rbf, two-sample-test, drift-detection, u-statistic]
applies_when: >-
  Code computes MMD, a kernel two-sample test, or a drift or distance score
  from kernel matrices of two samples.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - "Gretton, Borgwardt, Rasch, Schölkopf, and Smola, A Kernel Two-Sample Test, JMLR 13 (2012) 723-773: equation 5 (biased), equation 3 and Lemma 6 (unbiased)"
    - "Hoeffding, A Class of Statistics with Asymptotically Normal Distribution, Annals of Mathematical Statistics 19 (1948)"
evidence: []
---

## Details

Samples `x` (size m) and `y` (size n), kernel `k`. Write `Kxx`, `Kyy`, and
`Kxy` for the three kernel matrices.

**Biased estimator (V-statistic, Gretton et al. equation 5).** Average every
entry of each matrix, diagonal included:

    MMD²_b = mean(Kxx) + mean(Kyy) - 2 mean(Kxy)

The diagonals `k(x_i, x_i)` are the kernel's largest values (1 for an RBF
kernel), so the estimate is inflated by roughly `(1/m + 1/n)` times the gap
between the diagonal and the typical off-diagonal value. It's never negative,
it's positive for two independent samples from one distribution, and the
bias shrinks only as the samples grow.

**Unbiased estimators (U-statistics).** Two standard forms; both are
correct and they differ only slightly:

1. General form (equation 3; any m and n): leave the diagonal out of the two
   within-sample averages, and average the cross matrix in full.

       MMD²_u = sum_{i≠j} Kxx[i,j] / (m(m-1))
              + sum_{i≠j} Kyy[i,j] / (n(n-1))
              - 2 mean(Kxy)

2. Lemma 6 form (equal sizes, m = n): one average over pairs i ≠ j of
   `h(i,j) = Kxx[i,j] + Kyy[i,j] - Kxy[i,j] - Kxy[j,i]`, which also leaves
   the diagonal out of the cross term.

An unbiased estimate can be negative; its expected value is exactly the
population MMD², so it centers on 0 when the distributions match. If the
code reports MMD rather than MMD², take `sqrt(max(value, 0))`.

Kernel conventions differ: `exp(-gamma * ||a-b||²)` versus
`exp(-||a-b||² / (2 sigma²))`. Keep whichever the surrounding code and task
define; the estimator choice is separate from the bandwidth choice.

## How to tell them apart

- In the code: a plain `.mean()` of each full kernel matrix is the biased
  form. Subtracting the trace, or masking the diagonal, and dividing by
  `m(m-1)` is the unbiased form.
- By behavior: draw two independent samples from one distribution many
  times. The unbiased estimate averages near 0 and is sometimes negative;
  the biased one is always above 0.
- Don't discriminate with identical inputs, `mmd(a, a)`: the biased form
  and the Lemma 6 form both return 0 there. Use two different samples.

```python
import numpy as np

def rbf(a, b, gamma):
    d = (a * a).sum(1)[:, None] + (b * b).sum(1)[None, :] - 2 * a @ b.T
    return np.exp(-gamma * np.maximum(d, 0.0))

def mmd2_biased(x, y, gamma):
    return rbf(x, x, gamma).mean() + rbf(y, y, gamma).mean() - 2 * rbf(x, y, gamma).mean()

def mmd2_unbiased(x, y, gamma):
    m, n = len(x), len(y)
    kxx, kyy = rbf(x, x, gamma), rbf(y, y, gamma)
    return ((kxx.sum() - np.trace(kxx)) / (m * (m - 1))
            + (kyy.sum() - np.trace(kyy)) / (n * (n - 1))
            - 2 * rbf(x, y, gamma).mean())

rng = np.random.default_rng(1)
null = [mmd2_unbiased(rng.normal(size=(50, 4)), rng.normal(size=(50, 4)), 0.5) for _ in range(200)]
print("unbiased mean under the null:", np.mean(null))   # close to 0
```
