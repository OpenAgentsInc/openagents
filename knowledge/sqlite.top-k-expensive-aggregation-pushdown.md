---
id: sqlite.top-k-expensive-aggregation-pushdown
version: 1
kind: method
title: Limit expensive SQLite aggregation to the selected top-k entities
summary: >-
  When a query ranks entities by grouped statistics and then computes
  additional per-entity aggregates, first identify a logically safe candidate
  set and materialize the ordered top-k before doing the expensive follow-up
  work. Validate exact ordered results because moving filters or limits across
  joins and grouping can change semantics.
tags: [sqlite, sql, query-optimization, aggregation, top-k]
applies_when: >-
  SQLite queries perform repeated or per-entity aggregation after computing
  grouped scores, especially when only a small ordered top-k is returned.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - query-optimize
  cites:
    - SQLite Documentation, The WITH Clause, section “Materialization Hints” (https://www.sqlite.org/lang_with.html#materialization_hints)
    - SQLite Documentation, EXPLAIN QUERY PLAN (https://www.sqlite.org/eqp.html)
    - SQLite Documentation, Window Functions (https://www.sqlite.org/windowfunctions.html)
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Reduce work in stages:

1. Identify a cheap eligibility condition that is provably necessary for final inclusion, and use it to avoid aggregating entities that cannot qualify. A prefilter must not discard rows needed to calculate the final statistics; do not assume an eligibility condition is safe merely because it resembles a final `HAVING` clause.
2. Compute grouped statistics and apply the original qualification and complete ordering, including every deterministic tie-breaker.
3. Materialize the ordered top-k entity rows, then restrict downstream joins, window functions, or per-entity aggregates to that selected set. This can replace repeated correlated work over a large population with one grouped computation over a much smaller subset.
4. Keep join multiplicity and NULL behavior intact. In particular, changing join order or replacing `COUNT(*)` with a distinct count is not generally semantics-preserving.

SQLite's `AS MATERIALIZED` CTE hint acts as an optimization fence and can help ensure that a small selected set is computed before downstream work; it is not universally faster, so compare plans and timings. SQLite query planning is cost-based, so benchmark plausible formulations on representative data rather than inferring speed from SQL text alone.

Sources: SQLite Documentation, “The WITH Clause,” section “Materialization Hints”; SQLite Documentation, “EXPLAIN QUERY PLAN”; SQLite Documentation, “Window Functions.”
