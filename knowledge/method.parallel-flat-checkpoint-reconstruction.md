---
id: method.parallel-flat-checkpoint-reconstruction
version: 2
kind: method
title: Reconstruct parallel checkpoints from flat parameter buffers
summary: >-
  Rebuild flat-buffer checkpoints by deriving shard shapes, ordering, padding,
  and parallel-rank ownership from the framework, then validating the
  assembled model against independent invariants and outputs. Useful when the
  original checkpoint writer is unavailable or the saved layout is
  nonstandard.
tags: [checkpoints, tensor-parallelism, pipeline-parallelism, expert-parallelism, validation]
applies_when: >-
  Checkpoint shards contain flattened buffers, rank-specific parameter
  fragments, or incomplete metadata, and a model definition or reference
  output is available.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - mp-checkpoint-consolidation
  cites:
    - Shoeybi et al., “大Scale Language Models with Model Parallelism,” §2, “Model Parallelism”
    - NVIDIA, Megatron Core User Guide, “Parallelism Strategies”
    - PyTorch, Serialization semantics, “Saving and loading tensors”
evidence: []
---

## Details

Treat reconstruction as schema recovery, not merely concatenation. Derive each rank’s parameter shapes and ownership from configuration and framework code; reproduce the writer’s flattening order and any per-parameter alignment padding before reading offsets. Account for pipeline-stage layer assignment and expert ownership as well as tensor-parallel splits. Strip padded vocabulary or other alignment rows only when rebuilding the corresponding unpadded model tensor.

Use multiple independent checks while recovering the layout: shard-buffer lengths and offsets, known parameter invariants (for example, norm weights or initialized biases), rank-to-rank replication or disjointness patterns, and the final model’s output on a reference input. Require the reconstructed parameter keys and shapes to match the target model, including the target’s convention for tied weights. Do not accept a layout merely because it consumes the buffer exactly; different parameter orders can have the same total size.

Sources: Shoeybi et al., “大Scale Language Models with Model Parallelism,” §2, “Model Parallelism”; NVIDIA, *Megatron Core User Guide*, “Parallelism Strategies”; PyTorch, *Serialization semantics*, “Saving and loading tensors.”
