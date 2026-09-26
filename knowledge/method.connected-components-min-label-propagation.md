---
id: method.connected-components-min-label-propagation
version: 1
kind: method
title: Compute connected components by iterative minimum-label propagation
summary: >-
  On an undirected edge relation, repeatedly propagate the minimum component
  label across neighbors until labels stabilize; this produces each
  component's minimum vertex identifier as a canonical label.
tags: [graph, connected-components, spark, iterative-algorithms]
applies_when: >-
  Distributed deduplication or entity clustering requires transitive grouping
  from pairwise similarity edges and a deterministic minimum-ID
  representative.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - distributed-dedup
  cites:
    - Awerbuch and Shiloach, New Connectivity and MSF Algorithms for Ultracomputer, 1987, connected-components algorithm
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Create a symmetric edge relation (or make neighbor propagation explicitly bidirectional), initialize each participating vertex label to its own identifier, then update each vertex to the minimum of its current label and labels received from neighbors. Repeat until no label changes. Each iteration's label is the minimum identifier reachable within that many propagation steps; after convergence it is therefore the minimum identifier in the connected component. Isolated vertices must be included separately with their own identifier as label.

A practical acceleration is pointer jumping: propagate a neighbor's current label and, where the representation supports it, the label of that label. This can shorten convergence depth, but preserve the invariant that labels only decrease and remain identifiers in the same component. Termination should be data-driven (no changed labels) or bounded with a correctness argument, not based on an arbitrary small iteration count.

Sources: Awerbuch and Shiloach, “New Connectivity and MSF Algorithms for Ultracomputer,” *Theoretical Computer Science* 1987, connected-components algorithm; Apache Spark SQL documentation, “DataFrame” and iterative query construction guidance (for relational implementation constraints).

## How to check

On small graphs, compare the final label for every vertex with a trusted traversal-based connected-components result. Include a long chain (to exercise propagation), a cycle, multiple components, and isolated vertices; assert labels are non-increasing across iterations and equal the minimum vertex ID of each reference component.
