---
id: method.parallel-flat-checkpoint-reconstruction
version: 1
kind: method
title: Reconstruct parallel model weights from flat checkpoint shards
summary: >-
  When checkpoint shards contain flat buffers but omit a usable writer or
  index, reconstruct the serialization layout before combining weights. Treat
  key order, padding, fused tensors, rank mapping, and shard-specific
  transformations as explicit format details—not standard conventions to
  assume.
tags: [checkpoints, tensor-parallelism, pipeline-parallelism, serialization]
applies_when: >-
  A checkpoint contains flattened or packed parameter buffers distributed
  across tensor-, pipeline-, expert-, or other parallel ranks, and the
  checkpoint writer is unavailable or incomplete.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - mp-checkpoint-consolidation
  cites:
    - "Shoeybi et al., “Megatron-LM: Training Multi-Billion Parameter Language Models Using Model Parallelism,” Section 3, “Model Parallelism.”"
    - PyTorch documentation, “Tensor.view”; “torch.reshape.”
evidence: []
---

## Details

Use the model definition and framework source to establish the expected parameter names, shapes, and parallel ownership. Then determine the serialization order and offset rules for each shard. A flat buffer may include alignment padding between tensors; fused parameters may combine several logical weights; matrices may be stored transposed; and rank-local slices may use a fork-specific ordering. Do not infer ordering from alphabetical keys or apply a familiar framework's conventions unless verified against the actual serializer or shard contents.

Represent layout as an ordered sequence of `(key, stored_shape, transform)` records per parallel coordinate. Advance offsets using stored element counts and the verified alignment rule, then undo each storage transform while assembling the logical tensor. Account explicitly for padding and tied parameters. For pipeline parallelism, map local layer indices to global layers; for tensor or expert parallelism, apply the actual rank-to-slice mapping before concatenation or placement.

Shoeybi et al., “こう,” *Megatron-LM: Training Multi-Billion Parameter Language Models Using Model Parallelism*, Section 3, describes tensor and pipeline model parallelism; PyTorch’s `Tensor.view` and `torch.reshape` documentation explains shape reinterpretation versus reshaping. Neither source establishes a particular checkpoint writer's packing convention: confirm those conventions from the implementation and data.

## How to check

For every shard, assert that computed tensor spans are in bounds, do not overlap, and account for the buffer according to the observed padding rule. Check known replicated tensors across ranks and verify that reconstructed tensor shapes match the target model. Finally, load the merged state into the model, check the exact required key set, and compare outputs on fixed inputs.

```python
import math

def padded_span(offset, shape, alignment):
    n = math.prod(shape)
    end = offset + n
    next_offset = ((end + alignment - 1) // alignment) * alignment
    return (offset, end), next_offset

# For each shard, walk the independently established ordered layout:
# span, offset = padded_span(offset, stored_shape, alignment)
# assert span[1] <= buffer.numel()
# extract buffer[span[0]:span[1]].reshape(stored_shape)
```
