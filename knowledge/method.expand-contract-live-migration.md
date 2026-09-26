---
id: method.expand-contract-live-migration
version: 1
kind: method
title: Migrate a live database with expand, backfill, verify, switch, contract
summary: >-
  Change a schema or move data while the service keeps running by making only
  additive changes first, copying existing rows idempotently while new writes
  go to both places, proving the copies match, switching reads and writes, and
  only then removing the old path. Keep a rollback until the new path is proven.
tags: [database, migration, cutover, zero-downtime, backfill, postgresql]
applies_when: >-
  Moving a running application to a new table, schema, column format, or
  database server without losing writes made during the move.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "Danilo Sato, ParallelChange (martinfowler.com, 2014): expand, migrate, contract"
    - "Scott Ambler and Pramod Sadalage, Refactoring Databases: Evolutionary Database Design (Addison-Wesley, 2006), transition periods"
    - "PostgreSQL Documentation, chapter Logical Replication; and INSERT ... ON CONFLICT"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

A live migration fails in two characteristic ways: writes that arrive during
the copy are lost, or the switch happens before the copy is correct. The
phases below avoid both.

1. **Expand.** Add the new structure without breaking the old code: new tables,
   nullable columns or columns with defaults, new indexes built concurrently
   where the engine supports it. Nothing yet reads the new structure.
2. **Capture ongoing writes.** Make every new write reach both old and new
   (dual write in the application, a trigger, or change data capture such as
   logical replication). Start this *before* the bulk copy so no write falls in
   the gap.
3. **Backfill.** Copy existing rows in bounded batches keyed by primary key,
   using idempotent upserts (`INSERT ... ON CONFLICT (pk) DO UPDATE`) so a batch
   can be rerun and so the backfill never overwrites a newer dual-written row
   with an older copy (compare an `updated_at` or version column).
4. **Verify.** Compare old and new: row counts per key range, per-row or
   per-chunk checksums of the canonical column values, and domain invariants
   (sums, foreign keys, uniqueness). Investigate every mismatch; do not round
   it away.
5. **Switch.** Move reads, then writes, to the new path behind a flag or a
   single connection setting. For a whole-server cutover, briefly stop or
   queue writes, drain in-flight transactions, apply the final delta, reverify,
   then repoint clients.
6. **Contract.** After a soak period with the old path idle, remove dual
   writes and drop the old structure.

Details that break cutovers: identity/serial sequences that were not advanced
past the copied ids, triggers or constraints disabled for the copy and never
re-enabled, differing collation, encoding, or time zone settings between old
and new, and long-running transactions that commit old values after the
backfill passed their rows.

## How to check

Run the application's write traffic (or a replay of it) during the backfill
and confirm the verification step still shows zero differences afterwards.
Insert a new row through the application after the switch and confirm it gets
a fresh id and lands only once. Confirm the rollback path works before the
contract step.
