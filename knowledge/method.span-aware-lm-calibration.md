---
id: method.span-aware-lm-calibration
version: 1
kind: method
title: Score selected spans while retaining full causal context
summary: >-
  For language-model evaluation, distinguish text that supplies context from
  text whose log probabilities contribute to a score. Compute calibration
  adjustments on raw aggregate scores before applying the requested
  normalization.
tags: [language-modeling, log-probability, span-scoring, calibration, normalization]
applies_when: >-
  Evaluating choices or continuations with unscored prefixes/suffixes, marked
  spans, unconditional baselines, or batch-level calibration.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - batched-eval-parity
  cites:
    - Jurafsky and Martin, *Speech and Language Processing*, 3rd ed. draft, §3.1, “N-Gram Language Models.”
    - Church and Hanks, “Word Association Norms, Mutual Information, and Lexicography,” §2, *Computational Linguistics* 16(1) (1990).
evidence: []
---

## Details
Construct the full causal sequence, including all context text, but apply the scoring mask only to the requested target positions. Unscored tokens still affect later conditional probabilities; excluding them from the input changes the model computation. Sum selected token log probabilities to obtain a raw score, and track the selected token or byte count separately for normalization. When a scoring rule subtracts an unconditional or calibration baseline, subtract raw scores first and normalize the adjusted result afterward. If the baseline is a mean over a candidate set, compute it over the complete specified set before splitting work into batches; otherwise batch composition or input order can change scores. Preserve the specified candidate order when resolving ties.

## How to check
Use a short sequence with an unscored prefix, scored middle span, and unscored suffix. Verify that the scored positions match the corresponding conditional log probabilities from the full sequence, then independently verify that changing the unscored context can change later scores. Compare calibrated results when the same records are evaluated in different batch sizes and orders. The chain rule for sequence probabilities is described in Jurafsky and Martin, *Speech and Language Processing* (3rd ed. draft), §3.1; pointwise mutual information and its log-ratio form are discussed in Church and Hanks, “Word Association Norms, Mutual Information, and Lexicography,” §2 (1990).
