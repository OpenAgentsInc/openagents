---
id: edge-case.duplicate-record-ids
version: 1
kind: edge-case
title: Do not use record IDs to restore output order
summary: >-
  Record identifiers may be repeated, while some input records may be consumed
  only as references and omitted from output. Track output rows by original
  non-omitted input position rather than by ID.
tags: [data-pipelines, ordering, duplicate-identifiers, references]
applies_when: >-
  A pipeline removes support or metadata records, reorders work for batching,
  or merges computed results back into input order.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - batched-eval-parity
  cites:
    - "Python Software Foundation, *Python Standard Library*, “Built-in Functions: `enumerate`.”"
evidence: []
---

## Details
Treat an ID used to resolve a referenced record as a lookup key only when the input contract makes it unique. Do not assume it is a unique row identity or cache key. Assign each output-eligible input row a stable ordinal before filtering, batching, or reordering; associate intermediate results with that ordinal and emit them in ordinal order. Keep reference resolution separate from result alignment, and treat optional prefix labels as optimization hints unless uniqueness and content identity are guaranteed.

## How to check
Create a small input with repeated IDs, interspersed non-output support records, and a processing order different from input order. Confirm that each eligible row appears once, in its original eligible-row order, with its own computed result. Python's `enumerate` provides an explicit ordinal during input traversal; see the Python Standard Library documentation, “Built-in Functions: `enumerate`.”
