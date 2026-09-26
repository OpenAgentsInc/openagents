---
id: edge-case.ambiguous-name-variants
version: 1
kind: edge-case
title: Do not treat nickname compatibility as unique identity
summary: >-
  Nickname and short-name mappings are often many-to-many: one short form can
  represent distinct full names. Keep alternatives rather than taking
  transitive closure, and use independent identity evidence to resolve
  ambiguous name matches.
tags: [entity-resolution, names, ambiguity, false-positives]
applies_when: >-
  Name matching uses nickname dictionaries, observed name variants, fuzzy
  similarity, or shared household attributes that may link multiple people.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - telecom-entity-resolution
  cites:
    - "Peter Christen, Data Matching: Concepts and Techniques for Record Linkage, Entity Resolution, and Duplicate Detection, 2012, Chapter 3."
    - Ivan P. Fellegi and Alan B. Sunter, “A Theory for Record Linkage,” Journal of the American Statistical Association, 1969, §2.
evidence: []
---

## Details

A name-variant relation is not necessarily an equivalence relation. If a short form is compatible with two full names, that does not make the two full names interchangeable. Avoid recursively expanding a nickname graph into a single canonical name, especially when the variants were inferred from weak or noisy matches.

Preserve the set of plausible name interpretations. A name agreement should contribute evidence, not establish identity by itself. Resolve ambiguity with independent individual-level evidence when available, such as a reliable identifier or consistent birth date. Shared address, surname, or phone may describe a household rather than one person; do not let those fields collapse likely relatives. If the evidence cannot distinguish candidates, defer the match instead of forcing one.

Sources: Christen, *Data Matching: Concepts and Techniques for Record Linkage, Entity Resolution, and Duplicate Detection*, Chapter 3 (similarity functions); Fellegi and Sunter, “A Theory for Record Linkage,” §2 (combining field-level evidence).
