---
id: method.conflict-aware-record-clustering
version: 2
kind: method
title: Cluster record links with explicit cannot-link checks
summary: >-
  A connected component can merge distinct entities through a single erroneous
  bridge; validate proposed unions against cluster-level contradictions, not
  just the linking pair. Apply when pairwise matches are converted into entity
  clusters.
tags: [record-linkage, clustering, constraints, transitivity]
applies_when: >-
  Turning scored record-pair links into entity clusters, particularly when
  records have conflicting identifiers or when clusters may contain duplicate
  records from one source.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - telecom-entity-resolution
  cites:
    - Bansal, Blum, and Chawla, “Correlation Clustering,” Machine Learning (2004), §2.
    - "Christen, Data Matching: Concepts and Techniques for Record Linkage, Entity Resolution, and Duplicate Detection (2012), chapter 6."
evidence: []
---

## Details

Treat pairwise links as evidence for clustering, not as proof that every connected component is one entity. Before merging two components, compare the proposed union with explicit cannot-link evidence and domain constraints: incompatible trusted identifiers, incompatible high-confidence dates, or mutually exclusive attributes may veto a merge. Re-evaluate these constraints at the component level because a chain of individually plausible links can still create an impossible cluster.

Keep constraints tied to field reliability and domain rules. Missing values are not conflicts; noisy fields should usually contribute penalties rather than vetoes. Do not impose a one-record-per-source rule unless the data-generating process guarantees it—duplicate records within a source may be legitimate or may need a separate deduplication policy. Correlation clustering provides a formal view in which positive and negative pairwise evidence jointly shapes the partition, rather than blindly taking transitive closure.

## How to check

```python
def may_merge(left, right, records, cannot_link):
    proposed = left | right
    for a in proposed:
        for b in proposed:
            if a < b and cannot_link(records[a], records[b]):
                return False
    return True

# Property: every final cluster contains no pair that violates a hard constraint.
assert all(
    not cannot_link(records[a], records[b])
    for cluster in clusters
    for a in cluster
    for b in cluster
    if a < b
)
```

Also inspect candidate bridge links: removing a single low-confidence edge should not unexpectedly join clusters with contradictory high-quality evidence.
