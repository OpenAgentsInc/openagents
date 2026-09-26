---
id: tool.sqlite-query-equivalence-benchmark
version: 1
kind: tool
title: Validate SQLite query rewrites with plans and ordered result comparisons
summary: >-
  For performance rewrites, inspect `EXPLAIN QUERY PLAN`, benchmark candidate
  queries on the same read-only database, and compare complete ordered results
  against the original. This catches semantic drift that a faster runtime or
  matching row count cannot reveal.
tags: [sqlite, sql, benchmarking, query-plan, regression-testing]
applies_when: >-
  An agent rewrites an existing SQLite query for speed and must preserve its
  exact output.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - query-optimize
  cites:
    - SQLite Documentation, EXPLAIN QUERY PLAN (https://www.sqlite.org/eqp.html)
    - SQLite Documentation, Open a Database Connection, section “URI Filenames” (https://www.sqlite.org/c3ref/open.html)
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Treat output equivalence and performance as separate checks. Run the original and candidate against the same database state, fetch all rows, and compare the ordered sequences (including column names when the interface exposes them). A row-count comparison alone misses changed values, duplicates, tie ordering, or column order. If output order is part of the contract, preserve all ordering keys and compare without sorting the results in the test harness.

Use `EXPLAIN QUERY PLAN` to locate full scans, temporary B-trees for grouping or sorting, repeated correlated subqueries, and automatic indexes. Then benchmark plausible alternatives under the same SQLite version and data conditions; execution plans explain a strategy but do not establish its runtime. Open source data read-only when possible so validation cannot mutate it.

Runnable Python check:

```python
import sqlite3

con = sqlite3.connect("file:database.sqlite?mode=ro", uri=True)
original_sql = open("original.sql", encoding="utf-8").read()
candidate_sql = open("candidate.sql", encoding="utf-8").read()

original = con.execute(original_sql)
original_columns = [column[0] for column in original.description]
original_rows = original.fetchall()

candidate = con.execute(candidate_sql)
candidate_columns = [column[0] for column in candidate.description]
candidate_rows = candidate.fetchall()

assert candidate_columns == original_columns
assert candidate_rows == original_rows
for row in con.execute("EXPLAIN QUERY PLAN " + candidate_sql):
    print(row)
```

Sources: SQLite Documentation, “EXPLAIN QUERY PLAN”; SQLite Documentation, “Open a Database Connection,” section “URI Filenames.”
