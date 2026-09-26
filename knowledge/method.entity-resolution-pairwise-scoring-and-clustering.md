---
id: method.entity-resolution-pairwise-scoring-and-clustering
version: 1
kind: method
title: Pairwise evidence scoring with constrained entity clustering
summary: >-
  For multi-source record linkage, high precision on easy identifiers is not
  enough: recover variants and missing-field links with calibrated pair
  evidence, then cluster under hard cannot-link constraints. Evaluate pairwise
  recall and F1, including adversarial subsets, rather than treating complete
  partition coverage as matching success.
tags: [entity-resolution, record-linkage, clustering, evaluation]
applies_when: >-
  Building person/entity clusters from heterogeneous records with noisy,
  partially missing identifiers and pairwise ground-truth scoring.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - telecom-entity-resolution-1790399102
  cites:
    - "Peter Christen, Data Matching: Concepts and Techniques for Record Linkage, Entity Resolution, and Duplicate Detection, 2012, chapters 3–4"
    - William E. Winkler, Overview of Record Linkage and Current Research Directions, U.S. Census Bureau Research Report RRS2006/02, comparison and decision models
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Normalize fields by their actual schema, then generate candidate pairs through multiple complementary blocking keys: exact identifiers, contact fields, names plus dates, and carefully bounded typo-tolerant keys. Score each candidate from independent evidence (including matches, disagreements, and missingness), rather than using one shared field as a merge rule. Exact identifiers can be corrupted or misassigned; names and dates can also vary, so retain both positive evidence and explicit conflict signals.

Cluster with constrained agglomeration: before joining two components, check cross-component pairs for hard conflicts and one-record-per-source violations. A pairwise relation that seems plausible locally must not create a transitive cluster containing a known incompatible pair. Treat name variants cautiously: infer compatibility from reliable linked examples where possible, and do not expand ambiguous nicknames without other corroboration; adversarial households can contain distinct people whose names collapse to the same nickname.

Most importantly, optimize and inspect pairwise recall as well as precision. A system that outputs singleton clusters everywhere can pass coverage and uniqueness checks while failing the actual linkage objective. Track precision, recall, and F1 globally and on designated difficult slices; inspect missed positive links and false merges separately, and validate the final serialized result against those metrics.

Source: Peter Christen, *Data Matching: Concepts and Techniques for Record Linkage, Entity Resolution, and Duplicate Detection*, Springer, 2012, chapters 3–4 (record linkage, indexing, and comparison); William E. Winkler, “Overview of Record Linkage and Current Research Directions,” *U.S. Census Bureau Research Report* RRS2006/02, sections on comparison and decision models.

## How to check

- Measure pairwise precision, recall, and F1, not just record coverage, cluster-size bounds, or direct-key agreement.
- Report metrics on hard/adversarial subsets separately; inspect false negatives where DOB or a name differs despite corroborating independent identifiers.
- Audit candidate-generation recall: determine whether known/strongly corroborated pairs are generated at all, then assess scoring and cluster constraints separately.
- Verify that transitive merges preserve cannot-link constraints across every pair in the resulting components.
