---
id: slip.tests-from-the-same-belief
version: 2
kind: slip
title: Training-set exactness does not establish generalization
summary: >-
  A rule system can reproduce every training example by memorizing them while
  failing on unseen inputs. When the goal is to infer reusable behavior,
  validate on held-out examples and inspect whether the rules generalize
  beyond the observed forms.
tags: [testing, generalization, overfitting, rule-systems]
applies_when: >-
  A model, rewrite cascade, parser, or other inferred rule system passes
  checks built only from the examples used to construct it.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - sound-change-cascade-1790395251
  cites:
    - Ron Kohavi, “A Study of Cross-Validation and Bootstrap for Accuracy Estimation and Model Selection,” §2, “Cross-Validation.”
evidence: []
---

## Details

Exact reproduction of the training set is a necessary fit check, not evidence that a learned rule will work on new inputs. A lookup table can score perfectly on its training examples and still have no predictive behavior. Keep a validation set separate from rule inference; when examples have related structures, split by structural group rather than randomly so that close variants do not leak across the split. For transformation tasks, also probe unseen combinations of familiar patterns to check that rules compose as intended.

Do not treat schema checks, determinism, or training-pair reproduction as substitutes for hidden or held-out behavior checks. If the system is required to generalize, a solution consisting only of per-example exceptions should be recognized as a fallback, not accepted as evidence that the general rule has been found.

Source: Ron Kohavi, “A Study of Cross-Validation and Bootstrap for Accuracy Estimation and Model Selection,” §2, “Cross-Validation.”

## How to check

Keep a held-out collection unavailable during rule construction and evaluate the finished transformation against it. A basic exact-match check is:

```python
# train and heldout are disjoint iterables of (input, expected_output) pairs.
# The learner must be fit using train only.
model = fit_rules(train)
errors = [(x, y, model(x)) for x, y in heldout if model(x) != y]
assert not errors, errors[:10]
```

For structured inputs, group related examples before splitting, and add cases that combine learned local patterns in arrangements absent from training.
