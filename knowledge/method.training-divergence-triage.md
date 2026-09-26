---
id: method.training-divergence-triage
version: 1
kind: method
title: Triage a training run that diverges, spikes, or stalls
summary: >-
  Localize a bad training run before changing hyperparameters: reproduce it
  with a fixed seed, find the first step where loss or gradient norm goes bad,
  dump that batch, and check data, numerics, and code in that order. Overfit a
  single batch to prove the pipeline can learn at all.
tags: [machine-learning, training, pytorch, nan, divergence, mixed-precision, data-quality]
applies_when: >-
  A training or fine-tuning run produces NaN or inf loss, sudden loss spikes,
  a flat loss, or results that differ from a reference run, and you must find
  and fix the cause.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "Andrej Karpathy, A Recipe for Training Neural Networks (karpathy.github.io, 2019)"
    - "Goodfellow, Bengio, Courville, Deep Learning (MIT Press, 2016), chapter 11 Practical Methodology (debugging strategies)"
    - "PyTorch documentation: torch.autograd.set_detect_anomaly, torch.nn.utils.clip_grad_norm_, Automatic Mixed Precision (torch.amp, GradScaler)"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

**Localize first.** Fix seeds and data order, log loss, learning rate, and
global gradient norm (`clip_grad_norm_` returns it) every step, and find the
first bad step. A sudden spike at a specific step points at that step's batch
or a schedule change; slow drift points at the optimizer or numerics; a flat
loss from step 0 points at wiring (no gradient reaching the parameters). Save
the batch and the checkpoint just before the bad step so it can be replayed.

**Data (most common).**

- Scan inputs for NaN or inf, out-of-range values, and token ids at or above
  the vocabulary size (an embedding lookup with a bad id crashes on GPU but
  may silently read garbage elsewhere).
- Verify each shard or file decodes completely with the expected record
  count, dtype, and shape, and compare stored checksums; a truncated,
  zero-filled, duplicated, or misaligned shard produces spikes at the steps
  that read it.
- Confirm labels stay aligned with inputs after shuffling, filtering, or
  packing, and that normalization statistics match the ones the model
  expects.

**Numerics.** Learning rate too high or warmup missing; loss computed as
`log(softmax(x))` instead of `log_softmax` or `cross_entropy` on logits;
`BCELoss` on probabilities instead of `BCEWithLogitsLoss`; division by a count
that can be zero; fp16 overflow without a `GradScaler` (bf16 avoids most of
it); Adam `eps` too small for low precision; `sum` versus `mean` reduction
changing the effective learning rate with batch size.

**Code.** Missing `optimizer.zero_grad()`, `model.train()`/`eval()` in the
wrong mode (dropout, batch-norm statistics), tensors detached or converted to
Python numbers inside the graph, parameters missing from the optimizer,
gradient accumulation without dividing the loss, or a scheduler stepped per
batch when written per epoch.

Use `torch.autograd.set_detect_anomaly(True)` on the replayed step to find
the operation that first produces NaN in the backward pass.

## How to check

Overfit one small batch to near-zero loss; if that fails, the bug is in the
model or loss, not the data. After the fix, replay from the saved checkpoint
through the previously bad step and confirm loss and gradient norm stay in
range, then compare a short run with a known-good reference configuration.
