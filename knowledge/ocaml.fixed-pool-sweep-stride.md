---
id: ocaml.fixed-pool-sweep-stride
version: 1
kind: slip
title: Advance fixed-size pool sweeps by slot size, not compressed free-run length
summary: >-
  In a fixed-size block pool, a free header may encode a run of slots for the
  freelist, but the sweep cursor must advance by one slot at a time. Confusing
  the encoded run size with the physical traversal stride can skip slots and
  corrupt heap traversal.
tags: [ocaml, garbage-collection, memory-management, heap]
applies_when: >-
  Collector code scans or sweeps fixed-size pools whose free-block headers
  encode multiple adjacent slots as one run.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - fix-ocaml-gc
  cites:
    - Richard Jones, Antony Hosking, and Eliot Moss, The Garbage Collection Handbook, 2nd edition, sections 3.2 and 3.3.
    - The OCaml Manual, The runtime system, section on the major heap.
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Keep separate the *logical run length* stored in a free header and the *physical stride* used by a pool traversal. A fixed-size pool consists of slots of a known size; even if a free header describes a run spanning several slots, a sweep loop that processes each slot must advance by exactly the pool's slot size. Advancing by the encoded run's total size can skip the intervening slot headers, eventually misread live data as metadata, or walk outside the valid pool structure. If the algorithm intentionally coalesces runs, make that a distinct operation with explicit bounds and ensure all per-slot invariants remain satisfied.

A productive debugging sequence for heap bootstrap failures is: build and reproduce the earliest crash; inspect pointer advancement alongside the metadata's meaning; compare traversal strides used in sibling loops; then test the corrected collector against repeated free/reallocate patterns and debug heap checks. A regression should exercise multiple size classes and cycles, since a single allocation shape may not reveal a skipped-slot defect.

Sources: Richard Jones, Antony Hosking, and Eliot Moss, *The Garbage Collection Handbook*, 2nd ed., sections 3.2 (free lists) and 3.3 (mark-sweep collection); OCaml manual, *The runtime system*, section on the major heap (pool allocation and sweeping).
