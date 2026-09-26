---
id: statistics.bootstrap-seeding
version: 1
kind: method
title: Seeding a bootstrap threshold calibration
summary: >-
  Calibrating a threshold by bootstrap resamples a reference sample with
  replacement, computes the statistic under the null, and takes an upper
  quantile. Use a local seeded generator, draw the two resamples
  independently, and match the resample size to the window size.
tags: [statistics, bootstrap, resampling, calibration, threshold, random-seed, reproducibility, numpy]
applies_when: >-
  Code calibrates a threshold, a critical value, or a false-alarm rate by
  resampling or permutation, or seeds random number generators for
  reproducible results.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - "Efron and Tibshirani, An Introduction to the Bootstrap (Chapman and Hall, 1993)"
    - "NumPy documentation, Random Generator (numpy.random.default_rng) and NEP 19"
evidence: []
---

## Details

To find a threshold with false-alarm rate alpha for a two-sample statistic
`S`: repeat B times (hundreds or more), draw two samples from the reference
with replacement, compute `S` between them, and take the `1 - alpha`
quantile of the B values.

Rules:

- **Local generator.** `rng = np.random.default_rng(seed)` and draw from
  `rng`. Calling `np.random.seed` inside library code changes global state
  for every other caller.
- **Independent draws.** Draw each of the two resamples from the same `rng`
  one after the other. Creating a new generator with the same seed for each
  resample makes both resamples identical, so every `S` is 0 and the
  threshold is 0.
- **Don't reseed inside the loop.** Seeding once gives B different
  resamples; reseeding each iteration gives B copies of one.
- **Same size as in use.** Many statistics depend on sample size (biased
  estimators especially), so resample the size of the windows the threshold
  will judge.
- **Quantile direction.** For "alert when `S` is large", the threshold is the
  upper quantile, `np.quantile(values, 1 - alpha)`.
- **Reproducibility.** A fixed seed makes the threshold identical across
  runs; a test that expects stable thresholds needs one.

## How to check

Run the calibration twice with one seed: the thresholds are equal. Check
that the B values aren't all equal. Then check the false-alarm rate: over
many fresh null pairs, about alpha of them exceed the threshold.
