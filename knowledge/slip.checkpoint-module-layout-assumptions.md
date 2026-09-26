---
id: slip.checkpoint-module-layout-assumptions
version: 1
kind: slip
title: Do not assume one sharding convention for every module family
summary: >-
  Checkpoint writers may use different orders, transpositions, or rank
  ownership for attention, dense MLPs, routed experts, and shared experts.
  Infer and validate each module family’s layout separately instead of
  generalizing from one successfully reconstructed tensor.
tags: [checkpoints, moe, tensor-parallelism, layout-inference]
applies_when: >-
  A model mixes dense and expert layers, fused projections, or custom kernels,
  and the checkpoint writer is unavailable or differs from the reference
  model’s parameter layout.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - mp-checkpoint-consolidation
  cites:
    - Shoeybi et al., “大Scale Language Models with Model Parallelism,” §2, “Model Parallelism”
    - NVIDIA, Megatron Core User Guide, “Mixture of Experts”
    - NVIDIA, Megatron Core User Guide, “Parallelism Strategies”
evidence: []
---

## Details

Parameter names and compatible shapes do not prove that two tensors use the same storage convention. Fused gate/up projections may interleave rows differently from separate reference-model projections; down projections may be stored transposed; expert groups may be assigned to ranks differently from shared parameters. These choices can vary by module family, so document the inferred mapping per family and use framework/kernel documentation as evidence rather than assuming a familiar framework default.

Compare rank shards to find replicated and rank-specific regions, then test candidate mappings against reference-model behavior. Use controlled ablations—such as replacing one parameter family at a time with zeros or randomized values—to identify which family still drives a mismatch. Treat a better score from a candidate mapping as a clue, not proof; confirm with full-output validation and structural checks.

Sources: Shoeybi et al., “大Scale Language Models with Model Parallelism,” §2, “Model Parallelism”; NVIDIA, *Megatron Core User Guide*, “Mixture of Experts” and “Parallelism Strategies.”
