---
id: slip.training-fit-as-generalization
version: 1
kind: slip
title: Treating training-pair fit as evidence of generalization
summary: >-
  A rule set can reproduce every observed pair by memorizing complete inputs
  while failing on new inputs. Measure performance on examples not used to
  construct the rules, especially when the task explicitly requires
  generalization.
tags: [testing, generalization, overfitting, rule-systems]
applies_when: >-
  A transformation, classifier, or rule cascade is evaluated against known
  input-output examples and is expected to handle unseen inputs.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - sound-change-cascade-1790395393
  cites:
    - Hastie, Tibshirani, and Friedman, The Elements of Statistical Learning, 2nd ed., §2.2, “The Bias-Variance Trade-Off”
evidence: []
---

## Details

Exact reproduction of training examples measures fit to those examples, not performance on unseen cases. A lookup table can achieve perfect training accuracy without capturing any reusable rule. Keep a held-out evaluation set, or use cross-validation when the dataset is too small, and report both training and held-out results. Do not use held-out examples to tune rules and then continue to call them held out.

Reference: Hastie, Tibshirani, and Friedman, *The Elements of Statistical Learning*, 2nd ed., §2.2, “The Bias-Variance Trade-Off.”

## How to check

For a small deterministic example, compare a memorizing baseline's accuracy on the construction examples and unseen examples:

```python
import random

pairs = [(f"input-{i}", f"output-{i}") for i in range(10)]
shuffled = pairs[:]
random.Random(0).shuffle(shuffled)
train, heldout = shuffled[:8], shuffled[8:]
lookup = dict(train)

accuracy = lambda rows: sum(lookup.get(x) == y for x, y in rows) / len(rows)
print("training accuracy:", accuracy(train))
print("held-out accuracy:", accuracy(heldout))
```

For a real system, evaluate the actual transformation on held-out inputs, not just a lookup baseline, and inspect failures before changing rules.
