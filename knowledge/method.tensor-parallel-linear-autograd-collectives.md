---
id: method.tensor-parallel-linear-autograd-collectives
version: 1
kind: method
title: Tensor-parallel linear layers with autograd-correct collectives
summary: >-
  Implement column- and row-sharded linear layers by pairing forward
  communication with its correct backward collective. Use when linear
  parameters and activations are partitioned across distributed ranks but
  outputs or losses are replicated.
tags: [pytorch, distributed, tensor-parallel, autograd, linear]
applies_when: >-
  A distributed implementation slices `nn.Linear` weights across ranks and
  requires ordinary autograd to produce reference-equivalent outputs and
  gradients.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - torch-tensor-parallelism
  cites:
    - PyTorch, `torch.nn.Linear` documentation, Shape section
    - PyTorch, `torch.autograd.Function` documentation, Extending torch.autograd
    - PyTorch, `torch.distributed` documentation, Collective functions
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

PyTorch linear weights follow `(out_features, in_features)` layout. Column parallelism partitions weight rows and bias entries across ranks, computes local output-feature blocks, then concatenates those blocks so every rank has the full output. Its gathered output's backward must select the local gradient slice; each rank owns a distinct output block.

Row parallelism partitions weight columns and consumes the matching input-feature shard. Each rank computes a partial output, then sums partials so each rank has the full output. Add a replicated bias after this sum, not in every partial matmul. In backward, the partial-output sum implies that each rank's local weight shard receives the same replicated output gradient; the input gradient is formed by concatenating local shard gradients, not summing them.

For a replicated input feeding column parallelism, forward is an identity, but backward must all-reduce input gradients contributed by the distinct output shards. This is distinct from the gather's backward slice and the row-output reduce's backward identity. Implement communication in custom autograd functions when the distributed operation itself lacks the required autograd semantics. Support a one-rank fast path, and ensure collectives run in consistent order on every rank.

References: PyTorch documentation, `torch.nn.Linear` (weight shape and affine operation), `torch.autograd.Function` (custom forward/backward), and `torch.distributed` collectives (`all_gather`, `all_reduce`).
