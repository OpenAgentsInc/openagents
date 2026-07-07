-- CFG-17 (issue #8533): EVENT_LEDGER_OWNER Durable Object evacuation target.
--
-- The DO gave per-owner single-writer ordering: `getByName(ownerAgentUserId)`
-- routed every event-ledger-ingest job for an owner to one single-threaded
-- object, whose per-owner SQLite `event_ledger_owner_order` table allocated a
-- gapless monotonic `ordering_sequence` (MAX+1) and deduped by ordering key.
-- On Cloud Run there is no DO; this table is its Postgres home, and the owned
-- oa-infra Mutex (pg_advisory_xact_lock keyed by owner) provides the
-- serialization the DO's single thread used to provide.
--
-- Reservation is idempotent per (owner_agent_user_id, ordering_key) — the
-- `source:externalRef` of the delivery — so a redelivered job re-reads its
-- existing sequence instead of allocating a new one. `ordering_sequence` is
-- unique per owner, giving the same gapless per-owner ordering the DO did.
-- `persisted_at` flips once the D1 ledger row is written (append proof).

CREATE TABLE IF NOT EXISTS event_ledger_owner_order (
  owner_agent_user_id text        NOT NULL,
  ordering_key        text        NOT NULL,
  ordering_sequence   bigint      NOT NULL,
  first_seen_at       timestamptz NOT NULL,
  persisted_at        timestamptz,
  PRIMARY KEY (owner_agent_user_id, ordering_key),
  UNIQUE (owner_agent_user_id, ordering_sequence)
);
