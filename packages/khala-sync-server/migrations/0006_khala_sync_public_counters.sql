-- Khala Sync public-counter projections (KS-6.3, #8304).
-- Spec: docs/khala-sync/SPEC.md §2.1 (scope.public.<channel>), §7 invariant 8.
--
-- khala_sync_public_counters: one row per public counter (today:
-- `tokens-served`). The ingest path increments `total` and appends the
-- post-image to `scope.public.<counter_id>` in the SAME transaction; the
-- public route serves this row instead of the source-table full SUM.
--
-- BRING-UP ORDER: the row is created ONLY by the admin backfill/repair
-- action (projection = exact source SUM at that instant), never by the
-- increment path — an increment against a missing row is refused (and its
-- idempotency guard rolled back), so a fresh deploy can never serve a
-- small partial total in place of the real aggregate.
--
-- khala_sync_counter_applied: exact-once guard. One row per applied source
-- ledger event (its idempotency key); the guard insert shares the increment
-- transaction, so a replayed event (ON CONFLICT DO NOTHING) skips the
-- increment — the sync path never invents counter deltas (invariant 8).
--
-- khala_sync_public_counter_repairs: audit trail for backfill/repair. A
-- repair NEVER happens silently: every projection overwrite records the
-- previous total, the exact new total, its source, and an audit note.

CREATE TABLE IF NOT EXISTS khala_sync_public_counters (
  counter_id    text        PRIMARY KEY,
  total         bigint      NOT NULL DEFAULT 0
    CHECK (total >= 0),
  last_event_at timestamptz,
  updated_at    timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT khala_sync_public_counters_id_shape
    CHECK (counter_id ~ '^[a-z][a-z0-9-]*$')
);

CREATE TABLE IF NOT EXISTS khala_sync_counter_applied (
  counter_id      text        NOT NULL,
  idempotency_key text        NOT NULL,
  applied_at      timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (counter_id, idempotency_key),
  CONSTRAINT khala_sync_counter_applied_key_nonempty
    CHECK (length(idempotency_key) > 0)
);

CREATE TABLE IF NOT EXISTS khala_sync_public_counter_repairs (
  id             bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  counter_id     text        NOT NULL,
  previous_total bigint,
  new_total      bigint      NOT NULL
    CHECK (new_total >= 0),
  source         text        NOT NULL
    CHECK (source IN ('backfill', 'reconcile_repair')),
  audit_note     text        NOT NULL
    CHECK (length(audit_note) > 0),
  created_at     timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS khala_sync_public_counter_repairs_counter_idx
  ON khala_sync_public_counter_repairs (counter_id, created_at);
