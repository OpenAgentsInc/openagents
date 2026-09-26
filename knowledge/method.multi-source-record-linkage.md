---
id: method.multi-source-record-linkage
version: 2
kind: method
title: Multi-source record linkage with calibrated evidence and candidate blocks
summary: >-
  Normalize source-specific schemas, generate candidates through several
  complementary blocks, and score pairs using reliability-weighted evidence
  rather than treating any noisy field as identity. Applies when records from
  heterogeneous systems must be linked at scale.
tags: [record-linkage, entity-resolution, blocking, normalization]
applies_when: >-
  Linking person or entity records across datasets with different schemas,
  missing values, formatting differences, and typographical errors.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - telecom-entity-resolution
  cites:
    - Fellegi and Sunter, “A Theory for Record Linkage,” Journal of the American Statistical Association (1969), §2.
    - "Christen, Data Matching: Concepts and Techniques for Record Linkage, Entity Resolution, and Duplicate Detection (2012), chapters 4–5."
evidence: []
---

## Details

Use a staged workflow:

1. **Preserve raw values and normalize per source.** Parse names, dates, phones, addresses, and identifiers according to each source’s schema; keep both raw and normalized forms. Apply only semantics-preserving normalization by default. If a source has a known systematic defect (for example, reversed date components), retain the ordinary parse and a source-qualified alternative rather than silently rewriting the value.
2. **Profile before assigning evidence weights.** Measure missingness, within-source duplicates, cross-source agreement, and the frequency of each identifier. Exact agreement on a rare, reliable identifier is stronger evidence than agreement on a shared phone or common name. A field that is often shared or erroneous should not act as a stand-alone identity key.
3. **Generate candidates with multiple blocks.** Use a union of blocks—such as exact normalized identifiers, phone, email, address plus name, and name plus date—so a typo or missing field in one signal does not eliminate a true match. Blocking is a computational approximation, so measure candidate recall against known or manually reviewed pairs.
4. **Score pairs, then decide conservatively.** Compare field-level agreements and disagreements, accounting for missingness and source-specific error patterns. A disagreement is a hard veto only for a field whose reliability and semantics justify it; otherwise it is negative evidence. The Fellegi–Sunter framework formalizes evidence through match and non-match agreement probabilities.
5. **Review both pair and cluster behavior.** Pair scores do not by themselves guarantee safe transitive clustering. Check cluster-level conflicts and whether one bad bridge would join distinct people.

Fellegi and Sunter, “A Theory for Record Linkage,” *Journal of the American Statistical Association* (1969), §2; Christen, *Data Matching: Concepts and Techniques for Record Linkage, Entity Resolution, and Duplicate Detection* (2012), chapters 4–5.
