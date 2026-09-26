---
id: method.auditable-rules-pipeline
version: 1
kind: method
title: Build eligibility and compliance pipelines as ordered, explainable rules
summary: >-
  Encode each business rule as a separate, ordered, effective-dated check that
  returns a decision and a reason code, route unparseable or unknown records to
  an exception outcome instead of dropping them, and reconcile counts and
  totals between input and output so every record is accounted for.
tags: [claims, compliance, eligibility, data-pipeline, business-rules, validation, reporting]
applies_when: >-
  Implementing claims adjudication, warranty or benefit eligibility, customs
  or tax declarations, or any batch that applies written rules to records and
  reports accepted, rejected, or adjusted results.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "Barbara von Halle, Business Rules Applied (Wiley, 2001): rules as separately managed, declarative statements"
    - "Object Management Group, Decision Model and Notation (DMN) 1.4: decision tables and hit policies"
    - "Martin Kleppmann, Designing Data-Intensive Applications (O'Reilly, 2017), chapter 10 (batch processing, deterministic reruns)"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

Written rules become code most reliably as a table of small checks rather
than one large conditional:

- **One rule, one check.** Give each rule an identifier, the condition, the
  outcome (accept, reject, adjust, refer), and the reason code or message the
  output needs. Evaluate in the documented order and state the hit policy:
  first matching rule wins, or all rules apply and their effects accumulate.
- **Effective dates and versions.** Rules, rates, product codes, and coverage
  periods usually depend on a date (service date, purchase date, declaration
  period). Select the rule version valid on the record's relevant date, and
  make boundary inclusivity explicit (is the end date covered?). Use calendar
  arithmetic for "within N months" rather than 30-day approximations.
- **Normalization first.** Trim and case-fold codes, parse dates in every
  format the input actually uses, map synonyms and legacy codes through an
  explicit table, and treat blanks versus zeros deliberately. Keep the raw
  value alongside the normalized one for the audit trail.
- **Nothing silently dropped.** Duplicates, unknown codes, malformed rows,
  and records that fail several rules each get a defined outcome and reason.
  Report every applicable reason if the output format allows more than one.
- **Aggregation.** When a report groups records (by period, partner,
  commodity code, country), group on normalized keys and apply the
  thresholds or rounding the rules specify at the grouping level they name.
- **Determinism.** Stable sort orders, fixed output formats, and no
  dependence on dictionary or file-system order, so reruns give byte-identical
  output.

## How to check

Reconcile: input record count equals the sum of output records across all
outcomes, and monetary or quantity totals match between input and output
after adjustments. For each rule, run one record that just meets and one that
just misses its condition (dates on the boundary, amounts at the threshold)
and check the outcome and reason code, not only the count of accepted rows.
