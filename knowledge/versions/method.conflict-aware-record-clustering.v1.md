---
id: method.conflict-aware-record-clustering
version: 1
kind: method
title: Cluster links without trusting transitivity blindly
summary: >-
  Pairwise match decisions can form false chains when one mistaken edge joins
  otherwise distinct people. Merge clusters only when cross-cluster evidence
  is consistent, and defer ambiguous edges that have similarly strong
  competing candidates.
tags: [entity-resolution, clustering, transitivity, quality-control]
applies_when: >-
  Converting pairwise record-linkage scores into identity clusters,
  particularly when records have shared household attributes or multiple
  candidate matches.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - telecom-entity-resolution
  cites:
    - "Peter Christen, Data Matching: Concepts and Techniques for Record Linkage, Entity Resolution, and Duplicate Detection, 2012, Chapter 5."
    - Ivan P. Fellegi and Alan B. Sunter, “A Theory for Record Linkage,” Journal of the American Statistical Association, 1969, §2.
evidence: []
---

## Details

Pairwise similarity is not itself a globally consistent identity partition. A connected-components or union-find pass over every accepted edge assumes transitivity; one false bridge can merge many records. Process high-confidence links first, then consider weaker links with checks across the proposed cluster merge.

Before merging clusters, inspect cross-cluster pairs for reliable contradictions, such as incompatible high-quality identifiers. Also check whether either side has a similarly strong, incompatible alternative candidate. Defer ambiguous edges for later resolution rather than letting input order decide. This approach is conservative: it can leave some true matches unresolved, so tune the precision/recall tradeoff on representative labeled data.

Evaluate the resulting partition with pairwise same-entity precision and recall, and inspect cluster-size and source-composition distributions for suspicious mergers. Test bridge cases explicitly: a record compatible with two distinct identities should not automatically merge those identities.

Sources: Christen, *Data Matching: Concepts and Techniques for Record Linkage, Entity Resolution, and Duplicate Detection*, Chapter 5 (clustering); Fellegi and Sunter, “A Theory for Record Linkage,” §2.
