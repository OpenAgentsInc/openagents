---
id: method.cost-aware-batching-with-shared-shapes
version: 1
kind: method
title: Optimize batches jointly with latency, padding, compilation, and shared-shape costs
summary: >-
  For request batching under per-batch latency and global padding/cost
  constraints, sort compatible requests by generation length, use dynamic
  programming to choose contiguous batch partitions, then quantize prompt
  shapes globally under a shared compilation budget. Applies when a supplied
  evaluator charges both padded execution and shape compilation.
tags: [batching, optimization, dynamic-programming, latency, padding]
applies_when: >-
  A planner assigns requests to batches and tensor shapes, with additive
  per-batch execution costs, hard latency limits, global padding limits, and a
  cap or penalty on distinct compiled shapes.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - llm-inference-batching-scheduler
  cites:
    - Richard Bellman, Dynamic Programming, 1st ed., Chapter I, §§1–3
    - Ravindra K. Ahuja, Thomas L. Magnanti, and James B. Orlin, Network Flows, Chapter 9, §9.1
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

Treat batching as a constrained optimization problem rather than grouping only by prompt length or minimizing batch count. First inspect the evaluator's exact equations, rounding/alignment conventions, shape identity, and aggregation rules; derive feasibility margins before optimizing.

For each prompt-alignment class (and any other property that changes batch latency), sort requests by generation length. A batch's latency and padding can often be computed from its maximum padded sequence extent, so candidate batches are contiguous intervals in this ordering. Precompute interval costs, then use dynamic programming or shortest-path search to select a partition that minimizes execution cost while respecting per-batch latency. Track any global padding budget as a state or use a Pareto frontier; a Lagrangian penalty can produce candidates, but each candidate must be checked against the original hard constraints.

Shape selection is coupled to batching: fewer shared shapes can reduce compilation overhead but increase per-request shape padding. Aggregate demand by aligned prompt length, compute the cost of assigning intervals to representative shapes, and use a bounded-cluster dynamic program to select a globally shared shape set. Reassign requests using the selected shapes and rerun batching, since shape rounding changes latency and padding. If the two stages are approximations, iterate and retain only fully feasible plans.

A useful objective decomposition is execution cost plus one compile charge per distinct shape. Do not infer shape cost from a single batch: charge compilation according to the evaluator's exact reuse scope. Likewise, compute p95 and sequential-time aggregates exactly as specified rather than substituting averages or batch counts.

This formulation relies on interval optimality: for fixed prompt class and monotone generation-dependent batch extent/cost, sorting by generation length permits an optimal contiguous partition. If the cost model has cross-request interactions that violate this property, use a general partition/assignment method instead. Definitions: Bellman, *Dynamic Programming*, 1st ed., Chapter I, §§1–3 (principle of optimality and recurrence); Ahuja, Magnanti & Orlin, *Network Flows*, Chapter 9, §9.1 (shortest paths and dynamic programming).
