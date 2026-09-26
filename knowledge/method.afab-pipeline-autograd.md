---
id: method.afab-pipeline-autograd
version: 1
kind: method
title: Implement all-forward-all-backward pipeline training with explicit autograd
summary: >-
  A two-phase pipeline can retain each microbatch’s local graph during
  forward, then propagate output gradients backward in reverse schedule
  without hooks or custom autograd. Compare activations and gradients against
  an unpartitioned reference, not only scalar loss.
tags: [distributed-training, pipeline-parallelism, autograd, transformers]
applies_when: >-
  Implementing layer-partitioned sequential models with a schedule that runs
  all microbatch forwards before any backwards, especially when communicating
  between distributed ranks.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - torch-pipeline-parallelism
  cites:
    - PyTorch documentation, Distributed Communication Package (torch.distributed), Point-to-point communication; send and recv API
    - PyTorch documentation, Autograd mechanics, Backward pass
    - "Huang et al., GPipe: Efficient Training of Giant Neural Networks using Pipeline Parallelism, §3"
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Partition sequential blocks contiguously across ranks, distributing the remainder among early ranks so layer counts differ by at most one. For each microbatch, the first rank embeds token IDs and later ranks receive floating-point hidden states; each rank retains its local graph and stage output. The final rank computes the loss, scaled by the number of equally weighted microbatches. Then traverse microbatches in reverse: backpropagate the final-stage loss, send the resulting input gradient upstream, and call backward on the corresponding retained stage output using the received gradient. This yields ordinary local parameter gradients while avoiding hooks and custom autograd functions.

Use explicit point-to-point communication with matching send/receive order and shapes; carry token IDs only to the embedding stage and floating activations/gradients at stage boundaries. Ensure every rank follows a compatible communication schedule, including stages with no layers when the number of ranks exceeds the number of blocks. Preserve model-specific positional, masking, and normalization semantics when invoking blocks independently; a seemingly equivalent generic triangular mask may not match every model or attention backend.

A useful correctness oracle is a copied, unpartitioned model run on the same microbatches. Compare logits or stage activations, the gradient at every stage boundary, and each parameter gradient within dtype-appropriate tolerances. Scalar loss agreement alone does not establish correct gradient routing.

Sources: PyTorch, *Distributed Communication Package (torch.distributed)*, “Point-to-point communication” and `send`/`recv` API documentation; PyTorch, *Autograd mechanics*, “Backward pass”; Huang et al., “GPipe: Efficient Training of Giant Neural Networks using Pipeline Parallelism,” §3 (all-forward-then-all-backward schedule).
