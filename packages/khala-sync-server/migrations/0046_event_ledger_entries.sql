-- CFG-17 (issue #8533): event_ledger_entries D1 -> Postgres evacuation.
--
-- The append path (recordEventLedgerMessageWithOwnerMutex) reserved a gapless
-- per-owner `ordering_sequence` in Postgres (event_ledger_owner_order,
-- migration 0045) under the owned oa-infra advisory-lock Mutex, then wrote the
-- actual ledger row to `event_ledger_entries` in Cloudflare D1 via the
-- `d1-http` bridge. That D1 bridge is 401-dead account-wide (the Cloudflare
-- plan is cancelled and the token cannot be rotated), so every append
-- dead-lettered with `persisted_at` left NULL. This table is the Postgres home
-- for the ledger row so the whole reserve -> insert -> mark-persisted append
-- completes on Postgres under the same per-owner advisory lock.
--
-- Schema mirrors the final D1 shape (worker migrations 0285/0286/0287): all
-- timestamp/ISO fields stay `text` to preserve the exact string contract the
-- TypeScript row mapper passes straight through (rowToEntry), `ordering_key`
-- carries `source:external_ref`, and the two UNIQUE constraints give the same
-- per-owner dedup (idempotent redelivery) and gapless per-owner ordering the
-- DO/D1 pair gave.
--
-- EVACUATION COST (accepted): existing D1 `event_ledger_entries` rows cannot be
-- backfilled here because D1 is 401-dead and unreadable. This is an append-only
-- audit ledger; new appends land in Postgres, historical D1 rows stay only in
-- any prior D1 export (see docs/cloud/2026-07-06-d1-domain-cutover-readiness.md)
-- for a later optional import. New appends do NOT depend on that import.

CREATE TABLE IF NOT EXISTS event_ledger_entries (
  entry_id                 text     PRIMARY KEY,
  owner_agent_user_id      text     NOT NULL,
  owner_ref                text     NOT NULL,
  source                   text     NOT NULL CHECK (source IN ('github', 'slack')),
  external_ref             text     NOT NULL,
  actor_ref                text     NOT NULL,
  content_ref              text     NOT NULL,
  subject_ref              text     NOT NULL,
  event_type               text     NOT NULL,
  source_refs_json         text     NOT NULL,
  payload_summary_json     text     NOT NULL,
  occurred_at              text     NOT NULL,
  received_at              text     NOT NULL,
  ordering_key             text     NOT NULL,
  ordering_sequence        bigint   NOT NULL CHECK (ordering_sequence >= 1),
  handled_state            text     NOT NULL DEFAULT 'open'
    CHECK (handled_state IN ('open', 'handled', 'responded', 'ignored')),
  handled_by_run_id        text,
  handled_by_definition_id text,
  handled_at               text,
  handled_reason_ref       text,
  training_consent         smallint NOT NULL DEFAULT 0 CHECK (training_consent = 0),
  created_at               text     NOT NULL,
  updated_at               text     NOT NULL,
  UNIQUE (owner_agent_user_id, source, external_ref),
  UNIQUE (owner_agent_user_id, ordering_sequence)
);

CREATE INDEX IF NOT EXISTS idx_event_ledger_entries_owner_received
  ON event_ledger_entries (owner_agent_user_id, received_at DESC);

CREATE INDEX IF NOT EXISTS idx_event_ledger_entries_owner_source_subject
  ON event_ledger_entries (owner_agent_user_id, source, subject_ref);

CREATE INDEX IF NOT EXISTS idx_event_ledger_entries_owner_state_sequence
  ON event_ledger_entries (owner_agent_user_id, handled_state, ordering_sequence);

CREATE INDEX IF NOT EXISTS idx_event_ledger_entries_handled_run
  ON event_ledger_entries (owner_agent_user_id, handled_by_run_id);
