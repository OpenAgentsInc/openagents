---
id: slip.assuming-parallel-checkpoint-replication
version: 1
kind: slip
title: Verify rank ownership instead of assuming checkpoint tensors are replicated
summary: >-
  In distributed checkpoint conversion, equal shapes and architectural
  expectations do not establish which rank owns the correct tensor. Compare
  parsed values across parallel ranks and against a trusted reference before
  selecting or merging a rank’s copy.
tags: [distributed-checkpoint, tensor-parallel, expert-parallel, debugging]
applies_when: >-
  Consolidating flat or sharded model checkpoints, especially when some
  parameters are believed to be replicated across tensor-, expert-, or
  pipeline-parallel ranks.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - mp-checkpoint-consolidation-1790394263
  cites:
    - "Shoeybi et al., “Megatron-LM: Training Multi-Billion Parameter Language Models Using Model Parallelism,” §3.2, “Model Parallelism.”"
evidence: []
---

## Details

Model parallelism partitions model state across ranks, and different parallel dimensions can use different ownership or replication rules. Do not infer that a tensor is replicated—or that rank zero is authoritative—just because its name and shape look like an unsharded parameter. A mistaken ownership assumption can produce correctly shaped output with incorrect values.

Before choosing a source rank or merging shards, compare the parsed candidate tensor across ranks. For parameters expected to be replicated, assert equality. For parameters that differ, consult the checkpoint writer’s ownership rules and compare each candidate with a trusted state-dict reference when available. When debugging an end-to-end mismatch, inspect exact parameter values before investigating downstream model behavior.

## How to check

Given parsed state dictionaries keyed by rank and a reference state dictionary, this snippet identifies rank variation and tests candidate values:

```python
import torch

key = "some.parameter.weight"
candidates = {rank: state[key] for rank, state in parsed_by_rank.items()}
for rank, tensor in candidates.items():
    assert tensor.shape == reference_state[key].shape, (rank, tensor.shape)
    print(rank, "max error:", (tensor - reference_state[key]).abs().max().item())

ranks = list(candidates)
for rank in ranks[1:]:
    if not torch.equal(candidates[ranks[0]], candidates[rank]):
        print("rank values differ:", ranks[0], rank)
```

Do not use a rank-selection or merge rule until these checks agree with the documented checkpoint ownership.
