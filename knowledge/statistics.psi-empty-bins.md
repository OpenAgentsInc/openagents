---
id: statistics.psi-empty-bins
version: 1
kind: edge-case
title: Population stability index with empty bins
summary: >-
  The population stability index (PSI) sums (actual - expected) times
  ln(actual / expected) over shared bins. An empty bin makes the log
  infinite, so proportions need a small floor, and both samples must use the
  same bin edges taken from the reference.
tags: [statistics, psi, population-stability-index, drift-detection, histogram, binning, epsilon, divergence]
applies_when: >-
  Code computes PSI, a binned divergence, or a histogram-based drift score
  between a reference sample and a current sample.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - "Siddiqi, Credit Risk Scorecards (Wiley, 2006), chapter on scorecard monitoring"
    - "Yurdakul, Statistical Properties of Population Stability Index, PhD dissertation, Western Michigan University (2018)"
evidence: []
---

## Details

With reference proportions `e_i` and current proportions `a_i` over the same
bins:

    PSI = sum_i (a_i - e_i) * ln(a_i / e_i)

Every term is non-negative, so PSI is 0 only for identical proportions. It's
the symmetric (Jeffreys) form of the Kullback-Leibler divergence. A common
rule of thumb: under 0.1 stable, 0.1 to 0.25 moderate shift, over 0.25
significant shift.

Rules that working code follows:

- **One set of edges.** Compute bin edges from the reference only (quantiles
  or a fixed grid) and apply them to both samples. Binning each sample on its
  own edges hides the shift.
- **Open end bins.** Make the outer edges `-inf` and `+inf`, so current values
  outside the reference range land in the end bins instead of being dropped.
- **Proportions, not counts.** Divide each histogram by its own total.
- **Floor empty bins.** Replace zero proportions with a small epsilon (such
  as `1e-4` or `1e-6`) before the log, or add a pseudo-count to every bin.
  Clipping one side only, or skipping empty bins, understates the shift.
- **Sign and order.** `(a - e) * ln(a / e)` and `(e - a) * ln(e / a)` are
  equal; `(a - e) * ln(e / a)` is negative and wrong.
- **Quantile edges with ties.** Repeated values can give duplicate edges;
  drop duplicates with `np.unique`.

## How to check

PSI of a sample against itself is 0; PSI is symmetric in the two samples up
to the epsilon; shifting the current sample raises it; no bin yields `inf`
or `nan`.

```python
import numpy as np

def psi(ref, cur, bins=10, eps=1e-4):
    edges = np.unique(np.quantile(ref, np.linspace(0, 1, bins + 1)))
    edges[0], edges[-1] = -np.inf, np.inf
    e = np.histogram(ref, edges)[0] / len(ref)
    a = np.histogram(cur, edges)[0] / len(cur)
    e, a = np.clip(e, eps, None), np.clip(a, eps, None)
    return float(np.sum((a - e) * np.log(a / e)))
```
