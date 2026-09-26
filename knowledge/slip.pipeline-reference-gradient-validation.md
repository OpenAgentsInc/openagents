---
id: slip.pipeline-reference-gradient-validation
version: 1
kind: slip
title: Do not validate pipeline parallelism by loss alone
summary: >-
  A partitioned run can produce a plausible loss while routing or scaling
  boundary gradients incorrectly. Validate stage-boundary gradients and all
  parameter gradients against a full-model reference across rank counts and
  attention/dtype variants.
tags: [distributed-training, pipeline-parallelism, testing, gradients]
applies_when: >-
  Testing custom layer-wise model partitioning, point-to-point activation
  transfer, or manually scheduled backward propagation.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - torch-pipeline-parallelism
  cites:
    - PyTorch documentation, Autograd mechanics, Backward pass
    - "Huang et al., GPipe: Efficient Training of Giant Neural Networks using Pipeline Parallelism, §3"
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Loss equality is a weak pipeline-parallelism check: incorrect gradient scaling, a missing boundary gradient, or a reversed microbatch association can leave the forward result unchanged. Build a deterministic small reference model and compare, for identical inputs and weights, (1) stage outputs/logits, (2) gradients with respect to each transmitted activation, and (3) every trainable parameter gradient. Include both one-rank and multi-rank execution, uneven layer partitions, and representative attention implementations and dtypes. Hook-based observations can help diagnose local discrepancies, but avoid relying on hooks as part of the production execution path.

For microbatching, check that the sum of microbatch losses has the same reduction convention as the reference batch loss; if each microbatch loss is a mean over its tokens/examples, account explicitly for the number and weighting of microbatches.

Sources: PyTorch, *Autograd mechanics*, “Backward pass”; Huang et al., “GPipe: Efficient Training of Giant Neural Networks using Pipeline Parallelism,” §3.
