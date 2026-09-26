---
id: method.rank-differential-layout-inference
version: 1
kind: method
title: Use rank-to-rank differences to localize packed shard fields
summary: >-
  When flat-shard offsets or expert packing are uncertain, compare buffers
  from ranks that differ in only one parallel coordinate. Difference regions
  can identify rank-specific payloads and test candidate layouts, while
  matching regions provide evidence for replicated or shared fields.
tags: [checkpoints, expert-parallelism, reverse-engineering, differential-analysis]
applies_when: >-
  Parallel checkpoint buffers lack per-tensor offsets, and corresponding ranks
  can be compared while holding other parallel coordinates fixed.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - mp-checkpoint-consolidation
  cites:
    - "Lepikhin et al., “GShard: Scaling Giant Models with Conditional Computation and Automatic Sharding,” Section 2.2, “Sparsely-Gated Mixture-of-Experts Layer.”"
evidence: []
---

## Details

Choose corresponding shards that differ in one rank coordinate at a time. Compare their buffers elementwise and summarize contiguous difference runs, optionally at the suspected alignment granularity. If only expert ownership changes, for example, rank-dependent regions are candidates for expert-local parameters, while unchanged regions are candidates for shared or replicated state. Compare several rank pairs and coordinates before assigning meaning to any run.

Use these observations to validate a candidate named layout: predicted rank-local regions should coincide with observed differences, and predicted shared regions should remain equal. Differences are evidence, not labels—initialization, optimizer state, nondeterminism, or other rank-dependent metadata can also vary. Inspect tensor shapes and model semantics before concluding that a region is a particular parameter. Lepikhin et al., “GShard,” Section 2.2, describes expert parallelism and expert placement; the rank-difference procedure is a practical diagnostic for serialized representations of such sharding, not a substitute for the serializer specification.

## How to check

Run the comparison on multiple shard pairs with all but one coordinate held fixed. Check whether difference boundaries repeat across layers or ranks and align with candidate tensor boundaries; then verify the resulting reconstruction by loading the model and comparing outputs.

```python
import torch

# a and b are corresponding flat buffers from two ranks.
assert a.shape == b.shape
changed = (a != b)
# Group consecutive changed positions into runs; inspect run boundaries
# against candidate tensor spans and repeat for other rank pairs.
indices = torch.where(changed)[0]
if indices.numel():
    breaks = torch.where(indices[1:] != indices[:-1] + 1)[0] + 1
    runs = torch.tensor_split(indices, breaks.tolist())
    print([(int(r[0]), int(r[-1]) + 1) for r in runs if r.numel()])
```
