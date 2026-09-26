---
id: method.multi-source-record-linkage
version: 1
kind: method
title: Normalize, block, and score multi-source records
summary: >-
  For records from systems with different schemas, normalize fields by type,
  generate candidates through multiple blocking keys, then score pairs using
  corroborating and conflicting evidence. Measure candidate-generation recall
  separately from match precision.
tags: [entity-resolution, record-linkage, blocking, data-quality]
applies_when: >-
  Matching records across datasets that have inconsistent formats, incomplete
  fields, or enough records that comparing every pair is impractical.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - telecom-entity-resolution
  cites:
    - Ivan P. Fellegi and Alan B. Sunter, “A Theory for Record Linkage,” Journal of the American Statistical Association, 1969, §2.
    - "Peter Christen, Data Matching: Concepts and Techniques for Record Linkage, Entity Resolution, and Duplicate Detection, 2012, Chapters 2 and 4."
evidence: []
---

## Details

Map source-specific fields into a shared representation before comparing records. Normalize cautiously: retain useful distinctions such as unit numbers and name suffixes, and treat ambiguous date formats explicitly rather than silently choosing an interpretation. Missing values are unknown, not agreement.

Use multiple blocking keys based on independent identifiers or combinations of attributes. Exact and approximate blocks can recover complementary candidates; cap or subdivide very large blocks to avoid quadratic blowups. Blocking is a recall-sensitive step: a true pair excluded here cannot be recovered by the scorer. Audit candidate recall using trusted links or labeled examples.

Score candidate pairs with evidence that reflects the reliability and frequency of field agreements and disagreements. Fellegi–Sunter likelihood-ratio weights are a principled formulation; calibrated supervised models are another option. Do not let a common attribute such as a shared household phone outweigh stronger contradictory evidence. Treat a discrepancy as a hard conflict only when the field is reliable enough for that use.

Sources: Fellegi and Sunter, “A Theory for Record Linkage,” §2; Christen, *Data Matching: Concepts and Techniques for Record Linkage, Entity Resolution, and Duplicate Detection*, Chapter 2 (preprocessing) and Chapter 4 (classification).
