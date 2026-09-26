---
id: method.shard-global-calibration
version: 1
kind: method
title: Compute batch-calibrated scores over the full evaluation shard
summary: >-
  When calibration is defined over a candidate population, compute its
  raw-score aggregate across the entire shard, not independently within
  execution minibatches. Normalize only after applying all raw-score
  subtractions so batching and input order cannot change rankings.
tags: [calibration, scoring, batching, determinism]
applies_when: >-
  Multiple-choice scoring uses a batch-calibrated PMI-style correction whose
  reference mean is specified over candidates sharing a label and effective
  calibration group.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - batched-eval-parity-1790405023
  cites:
    - Church and Hanks, “Word Association Norms, Mutual Information, and Lexicography,” *Computational Linguistics* 16(1), 1990, §2
evidence: []
---

## Details

Separate scoring from calibration aggregation. First collect each candidate’s raw conditional score and any raw unconditional or calibration scores. Then compute each calibration reference over the complete eligible shard population for the relevant choice label and effective group; use the declared fallback hierarchy for the group. Subtract the raw correction terms before applying sum, token-mean, or byte-mean normalization. Do not substitute a minibatch mean: changing batch size or record order would then change the score of an otherwise identical record.

The PMI component is a log-probability difference; see Church and Hanks, “Word Association Norms, Mutual Information, and Lexicography,” *Computational Linguistics* 16(1), 1990, §2. The scope of a batch-calibrated reference mean is an evaluation-contract choice and must be implemented at the population scope that contract specifies.

## How to check

Evaluate the same shard with different batch sizes and input permutations, then map results back to record positions and compare candidate scores and predictions. Include groups with multiple candidates, groups that use fallback labels, and candidates with different token/byte counts to verify that raw subtraction precedes normalization.
