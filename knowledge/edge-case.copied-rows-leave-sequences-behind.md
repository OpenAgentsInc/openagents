---
id: edge-case.copied-rows-leave-sequences-behind
version: 1
kind: edge-case
title: Copying rows with explicit ids leaves the id sequence behind
summary: >-
  Inserting rows with their existing primary keys into a table whose key comes
  from a sequence or identity column does not advance that sequence. The next
  normal insert then reuses an id and fails with a duplicate key. Reset the
  sequence past the maximum copied id after any bulk copy or restore.
tags: [postgresql, sequence, identity, migration, bulk-load]
applies_when: >-
  After copying, restoring, or backfilling rows into a table with a serial,
  identity, or auto-increment key, before the application resumes inserts.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "PostgreSQL Documentation, Sequence Manipulation Functions: setval, pg_get_serial_sequence"
    - "PostgreSQL Documentation, CREATE TABLE: GENERATED ... AS IDENTITY and OVERRIDING SYSTEM VALUE"
    - "MySQL Reference Manual, Using AUTO_INCREMENT; SQLite documentation, Autoincrement"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

A sequence only advances when `nextval` is called. `INSERT` with an explicit
key value (or `COPY` of a key column) never calls it, so after a copy the
sequence can still sit at 1 while the table holds ids up to N. The first
application insert then fails with a unique violation, often only after the
migration looked finished.

PostgreSQL fix, per table:

```sql
SELECT setval(pg_get_serial_sequence('schema.table', 'id'),
              COALESCE((SELECT max(id) FROM schema.table), 0) + 1,
              false);
```

With `is_called = false`, the next `nextval` returns exactly the value given.
The same applies to `GENERATED ... AS IDENTITY` columns (`pg_get_serial_sequence`
finds their sequence too); inserting explicit values into a
`GENERATED ALWAYS` identity column needs `OVERRIDING SYSTEM VALUE`. Oracle
sequences behave the same way. Engines differ here: MySQL raises the
`AUTO_INCREMENT` counter when an explicit larger value is inserted, and SQLite
picks `max(rowid) + 1`, so the trap is mainly PostgreSQL- and Oracle-style
sequences. `pg_dump` output already contains `setval` calls; hand-written
copies usually do not.

## How to check

After the copy, compare each sequence's next value with `max(id)` for its
table, then perform one insert through the normal application path and check
that it succeeds with an id greater than every copied id.
