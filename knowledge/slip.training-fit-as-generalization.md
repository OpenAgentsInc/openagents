---
id: slip.training-fit-as-generalization
version: 2
kind: slip
title: Treating fitted oracle probes as evidence of generalization
summary: >-
  A scorer that matches a few visible examples may still fail on feature
  interactions, boundaries, defaults, or alternate packets. Use designed probe
  combinations and held-out cases before treating a reverse-engineered model
  as general.
tags: [black-box-testing, model-parity, generalization, interactions]
applies_when: >-
  Reconstructing behavior from a command-line oracle or other black-box
  reference, especially when the implementation must work on inputs beyond the
  examples used to infer it.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - sound-change-cascade-1790395393
    - risk-scorer-replay-1790394700
  cites:
    - Montgomery, Design and Analysis of Experiments, 10th ed., Chapter 5
    - Hastie, Tibshirani, and Friedman, The Elements of Statistical Learning, 2nd ed., §7.1
evidence: []
---

## Details

Matching selected examples establishes only that an implementation fits those observations. One-feature-at-a-time probes can reveal main effects but miss interactions, and a few representative values can miss cutoffs, missing-value behavior, and route changes. This is a generalization failure, not evidence that the reference is inconsistent.

Use a structured probe plan: sweep boundaries and defaults, combine important feature pairs (or use a small factorial design), and reserve cases that do not influence implementation decisions as a validation set. Compare both intermediate behavior and final outputs where possible. Montgomery, *Design and Analysis of Experiments*, 10th ed., Chapter 5, describes factorial designs for estimating effects and interactions; Hastie, Tibshirani, and Friedman, *The Elements of Statistical Learning*, 2nd ed., §7.1, discusses generalization and model assessment.

## How to check

Keep validation probes out of the fitting loop and run them after implementation changes. For a small set of factors, generate combinations rather than varying only one factor at a time:

```python
from itertools import product

factors = {
    "amount": ["0", "boundary-near", "large"],
    "age": ["", "below-cutoff", "above-cutoff"],
    "segment": ["known-a", "known-b", "unknown"],
}
validation_cases = [dict(zip(factors, values))
                    for values in product(*factors.values())]
# Evaluate each case against both the reference and the implementation.
```

Include additional held-out combinations and compare exact route/decision behavior as well as numeric scores; a match on isolated sweeps is not sufficient.
