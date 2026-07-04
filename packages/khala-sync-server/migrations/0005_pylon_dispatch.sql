-- KS-8.1 (#8307): pylon assignments/dispatch domain — Postgres twins of the
-- D1 tables `pylon_api_registrations`, `pylon_api_assignments`, and
-- `pylon_api_events` (worker migrations 0123/0134/0135/0146/0177/0256).
-- Plan: docs/khala-sync/MIGRATION_PLAN.md §1 (universal porting rules).
--
-- TYPE FIDELITY (v1, reconciliation-bearing): every column keeps D1's byte
-- representation — TEXT ISO-8601 timestamps (they sort correctly as text),
-- 0/1 booleans as smallint, JSON payloads as text (NOT jsonb: row-hash
-- reconciliation compares exact bytes). Tightening to native types is a
-- post-retirement cleanup, never mid-migration.
--
-- IDEMPOTENCY KEYS PORT EXACTLY: the D1 dedupe-SELECT-then-INSERT pairs on
-- `idempotency_key_hash` collapse to `ON CONFLICT ... DO NOTHING` upserts
-- against the SAME unique keys (assignments: idempotency_key_hash +
-- assignment_ref; events: idempotency_key_hash + event_ref; registrations:
-- pylon_ref).
--
-- NO CROSS-TABLE FOREIGN KEYS (deliberate, unlike D1's
-- assignments→registrations / events→registrations FKs): dual-write mirrors
-- and the backfill land per-table and per-row; an assignment mirror may
-- arrive before its registration is backfilled. Referential integrity is
-- verified by set-membership at reconciliation (MIGRATION_PLAN §3.7 rule),
-- not enforced mid-migration.
--
-- INDEXES ARE RE-DERIVED FROM ACTUAL QUERY PATTERNS in
-- apps/openagents.com/workers/api/src/pylon-api.ts — NOT blind-ported from
-- D1 (MIGRATION_PLAN universal rule). Deliberately dropped D1 indexes:
--   * idx_pylon_api_assignments_state_updated  (no query in this domain
--     filters by state without pylon_ref; the fleet stall detector's
--     cross-pylon scans re-home with KS-8.4/KS-8.2)
--   * idx_pylon_api_assignments_lease_expires  (the stale-lease sweep is
--     pylon-scoped: pylon_ref + state + lease_expires_at + updated_at is
--     served by the pylon_ref partial index below)
--   * idx_pylon_api_events_kind_created        (no store query filters by
--     event_kind alone)
--   * idx_pylon_api_registrations_status_updated (no store query filters
--     by status)

CREATE TABLE IF NOT EXISTS pylon_registrations (
  id                              text NOT NULL,
  pylon_ref                       text PRIMARY KEY,
  owner_agent_user_id             text NOT NULL,
  owner_agent_credential_id       text NOT NULL,
  owner_agent_token_prefix        text NOT NULL,
  display_name                    text NOT NULL,
  status                          text NOT NULL,
  resource_mode                   text NOT NULL,
  capability_refs_json            text NOT NULL,
  client_version                  text,
  client_protocol_version         text,
  wallet_ref                      text,
  wallet_ready                    smallint NOT NULL DEFAULT 0
    CHECK (wallet_ready IN (0, 1)),
  latest_heartbeat_at             text,
  latest_heartbeat_status         text,
  latest_resource_mode            text,
  latest_health_refs_json         text NOT NULL DEFAULT '[]',
  latest_load_refs_json           text NOT NULL DEFAULT '[]',
  latest_capacity_refs_json       text NOT NULL DEFAULT '[]',
  provider_nostr_pubkey           text,
  provider_nostr_npub             text,
  provider_market_relay_refs_json text NOT NULL DEFAULT '[]',
  provider_nip90_lane_refs_json   text NOT NULL DEFAULT '[]',
  public_projection_json          text NOT NULL,
  created_at                      text NOT NULL,
  updated_at                      text NOT NULL,
  archived_at                     text,
  CONSTRAINT pylon_registrations_id_unique UNIQUE (id)
);

-- Gate read: listRegistrationsForOwnerAgentUserIds (the June-29 503 victim:
-- "could not read linked owner registration").
CREATE INDEX IF NOT EXISTS pylon_registrations_owner_updated_idx
  ON pylon_registrations (owner_agent_user_id, updated_at DESC)
  WHERE archived_at IS NULL;

-- listRegistrations(limit): global newest-first over live rows.
CREATE INDEX IF NOT EXISTS pylon_registrations_updated_idx
  ON pylon_registrations (updated_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS pylon_assignments (
  id                            text NOT NULL,
  assignment_ref                text PRIMARY KEY,
  pylon_ref                     text NOT NULL,
  owner_agent_user_id           text NOT NULL,
  idempotency_key_hash          text NOT NULL,
  job_kind                      text NOT NULL,
  state                         text NOT NULL,
  payment_mode                  text NOT NULL DEFAULT 'unpaid_smoke'
    CHECK (payment_mode IN (
      'unpaid_smoke',
      'operator_credit',
      'payable_pending_settlement',
      'settled_bitcoin',
      'rejected_no_pay'
    )),
  lease_expires_at              text NOT NULL,
  task_refs_json                text NOT NULL,
  acceptance_criteria_refs_json text NOT NULL,
  result_expectation_refs_json  text NOT NULL,
  artifact_refs_json            text NOT NULL,
  proof_refs_json               text NOT NULL,
  accepted_work_refs_json       text NOT NULL,
  rejection_refs_json           text NOT NULL,
  closeout_refs_json            text NOT NULL,
  coding_assignment_json        text,
  public_projection_json        text NOT NULL,
  created_at                    text NOT NULL,
  updated_at                    text NOT NULL,
  archived_at                   text,
  CONSTRAINT pylon_assignments_id_unique UNIQUE (id),
  CONSTRAINT pylon_assignments_idempotency_key_unique
    UNIQUE (idempotency_key_hash)
);

-- listAssignmentsForPylon(s) (capacity gate reads: pylon_ref + active
-- states + newest-first) AND the pylon-scoped stale-lease sweep.
CREATE INDEX IF NOT EXISTS pylon_assignments_pylon_updated_idx
  ON pylon_assignments (pylon_ref, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS pylon_assignment_events (
  id                     text NOT NULL,
  event_ref              text PRIMARY KEY,
  pylon_ref              text NOT NULL,
  owner_agent_user_id    text NOT NULL,
  idempotency_key_hash   text NOT NULL,
  event_kind             text NOT NULL,
  assignment_ref         text,
  status                 text NOT NULL,
  event_body_json        text NOT NULL,
  public_projection_json text NOT NULL,
  created_at             text NOT NULL,
  archived_at            text,
  CONSTRAINT pylon_assignment_events_id_unique UNIQUE (id),
  CONSTRAINT pylon_assignment_events_idempotency_key_unique
    UNIQUE (idempotency_key_hash)
);

-- listEventsForPylon: pylon_ref + newest-first over live rows.
CREATE INDEX IF NOT EXISTS pylon_assignment_events_pylon_created_idx
  ON pylon_assignment_events (pylon_ref, created_at DESC)
  WHERE archived_at IS NULL;

-- listEventsForAssignment: assignment_ref + newest-first (event-chain reads).
CREATE INDEX IF NOT EXISTS pylon_assignment_events_assignment_created_idx
  ON pylon_assignment_events (assignment_ref, created_at DESC)
  WHERE archived_at IS NULL AND assignment_ref IS NOT NULL;
