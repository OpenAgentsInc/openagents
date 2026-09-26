---
id: method.priority-schema-normalized-record-merge
version: 1
kind: method
title: Normalize heterogeneous records before priority-based merging
summary: >-
  Map source-specific aliases into a canonical schema, normalize values, then
  resolve fields by an explicit source precedence while recording
  disagreements. Useful for consolidating records from heterogeneous exports
  without making merge behavior depend on input column order.
tags: [data-integration, record-linkage, schema-normalization]
applies_when: >-
  Combining structured records from multiple sources whose field names or
  representations differ and whose conflicts must be resolved
  deterministically.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - multi-source-data-merger
  cites:
    - "W3C, PROV-O: The PROV Ontology, sections 2–3"
    - Apache Arrow, Columnar Format documentation, Schema
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Define a canonical field set and explicit alias lists per field. Normalize each source record into that schema before merging: coerce identifiers only when they are valid integers, map boolean activity values to a documented status vocabulary, and standardize date representations. Treat missing values as absent rather than allowing them to overwrite populated values.

Choose and document a total source precedence. For each canonical identifier and field, select the first present value in precedence order; independently collect the distinct source values when they disagree, so the conflict report explains the decision. Do not infer precedence from iteration accidents or let a lower-priority null erase a higher-priority value. Validate the output schema, identifier uniqueness, canonical value formats, and conflict report against the merged records.

Sources: W3C, *PROV-O: The PROV Ontology*, sections 2–3 (provenance entities and derivations); Apache Arrow, *Columnar Format* documentation, Schema section (explicit field types and schemas).
