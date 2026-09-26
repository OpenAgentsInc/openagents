---
id: statistics.variance-ddof
version: 1
kind: method
title: Population and sample variance (ddof)
summary: >-
  Variance and standard deviation divide by n (population, ddof=0) or n - 1
  (sample, ddof=1). NumPy defaults to ddof=0 and pandas to ddof=1, so the
  same data gives different answers across libraries.
tags: [statistics, variance, standard-deviation, ddof, bessel-correction, numpy, pandas, z-score]
applies_when: >-
  Code computes a variance, standard deviation, z-score, standardization, or
  confidence interval, or must match numbers from another library.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - "Casella and Berger, Statistical Inference, 2nd edition (2002), section 5.2"
    - "NumPy documentation, numpy.std and numpy.var (ddof)"
    - "pandas documentation, DataFrame.std (ddof)"
    - "Python documentation, statistics.stdev and statistics.pstdev"
evidence: []
---

## Details

    population:  sum((x - mean)²) / n          ddof = 0
    sample:      sum((x - mean)²) / (n - 1)    ddof = 1 (Bessel's correction)

The sample form is the unbiased estimator of the population variance from a
sample. Defaults differ:

| Call | Divides by |
| --- | --- |
| `np.var`, `np.std`, `ndarray.std()` | n |
| `pandas.Series.std()`, `DataFrame.var()` | n - 1 |
| `statistics.stdev` / `statistics.pstdev` | n - 1 / n |
| `torch.std` (default) | n - 1 |

A specification that says "sample standard deviation" means ddof=1. With one
value, ddof=1 divides by zero: NumPy returns `nan` with a warning, and pandas
returns `NaN`.

## How to check

For `[1, 2, 3, 4]`: population variance 1.25, sample variance about 1.667.
Compare the code's result with both to see which it computes.
