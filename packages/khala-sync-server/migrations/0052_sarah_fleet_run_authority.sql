-- FC-1B (#8637): production-durable Sarah FleetRun intake authority.
--
-- This is the server-side handoff into the existing Pylon orchestration
-- authority. `sarah_fleet_run_intake_leases` leases an entire run to exactly
-- one owner-linked Pylon; it is NOT a second work-unit claim registry. Work
-- unit claims remain in the Pylon orchestration store after the claimed run is
-- imported there.
--
-- `request_json` is canonical, public-safe JSON. Owner identity and Pylon
-- linkage remain private columns and never enter the fleet Sync post-image.

CREATE TABLE IF NOT EXISTS sarah_fleet_run_requests (
  run_ref              text PRIMARY KEY,
  owner_user_id        text NOT NULL,
  idempotency_key      text NOT NULL,
  request_fingerprint  text NOT NULL,
  request_json         text NOT NULL,
  status               text NOT NULL
    CHECK (status IN ('pending_executor', 'claimed_by_pylon', 'cancelled')),
  target_preference    text NOT NULL
    CHECK (target_preference IN ('owner_local', 'managed_cloud', 'auto')),
  worker_kind          text NOT NULL
    CHECK (worker_kind IN ('codex', 'claude', 'grok', 'auto')),
  target_concurrency   integer NOT NULL
    CHECK (target_concurrency BETWEEN 1 AND 8),
  created_at           text NOT NULL,
  updated_at           text NOT NULL,
  CONSTRAINT sarah_fleet_run_requests_owner_idempotency_unique
    UNIQUE (owner_user_id, idempotency_key),
  CONSTRAINT sarah_fleet_run_requests_run_ref_shape
    CHECK (run_ref ~ '^fleet_run\.sarah\.[0-9a-f]{20}$'),
  CONSTRAINT sarah_fleet_run_requests_owner_shape
    CHECK (owner_user_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{2,159}$'),
  CONSTRAINT sarah_fleet_run_requests_idempotency_shape
    CHECK (idempotency_key ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{7,119}$'),
  CONSTRAINT sarah_fleet_run_requests_fingerprint_shape
    CHECK (request_fingerprint ~ '^[0-9a-f]{64}$')
);

CREATE INDEX IF NOT EXISTS sarah_fleet_run_requests_owner_pending_idx
  ON sarah_fleet_run_requests (owner_user_id, created_at, run_ref)
  WHERE status = 'pending_executor'
    AND target_preference IN ('owner_local', 'auto');

CREATE TABLE IF NOT EXISTS sarah_fleet_run_work_units (
  run_ref              text NOT NULL,
  owner_user_id        text NOT NULL,
  unit_index           integer NOT NULL CHECK (unit_index >= 0),
  unit_ref             text NOT NULL,
  issue_ref            text,
  title                text,
  depends_on_refs_json text NOT NULL,
  PRIMARY KEY (run_ref, unit_ref),
  CONSTRAINT sarah_fleet_run_work_units_position_unique
    UNIQUE (run_ref, unit_index),
  CONSTRAINT sarah_fleet_run_work_units_run_fk
    FOREIGN KEY (run_ref) REFERENCES sarah_fleet_run_requests(run_ref)
      ON DELETE CASCADE,
  CONSTRAINT sarah_fleet_run_work_units_owner_shape
    CHECK (owner_user_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{2,159}$'),
  CONSTRAINT sarah_fleet_run_work_units_ref_shape
    CHECK (unit_ref ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{0,159}$')
);

CREATE INDEX IF NOT EXISTS sarah_fleet_run_work_units_owner_run_idx
  ON sarah_fleet_run_work_units (owner_user_id, run_ref, unit_index);

CREATE TABLE IF NOT EXISTS sarah_fleet_run_intake_leases (
  run_ref               text PRIMARY KEY,
  claim_ref             text NOT NULL UNIQUE,
  owner_user_id         text NOT NULL,
  pylon_ref             text NOT NULL,
  claim_idempotency_key text NOT NULL,
  claim_fingerprint     text NOT NULL,
  state                  text NOT NULL
    CHECK (state IN ('claimed', 'accepted', 'released')),
  lease_expires_at       text NOT NULL,
  created_at             text NOT NULL,
  updated_at             text NOT NULL,
  CONSTRAINT sarah_fleet_run_intake_leases_run_fk
    FOREIGN KEY (run_ref) REFERENCES sarah_fleet_run_requests(run_ref)
      ON DELETE CASCADE,
  CONSTRAINT sarah_fleet_run_intake_leases_owner_pylon_idempotency_unique
    UNIQUE (owner_user_id, pylon_ref, claim_idempotency_key),
  CONSTRAINT sarah_fleet_run_intake_leases_claim_ref_shape
    CHECK (claim_ref ~ '^claim\.sarah_fleet_run\.[0-9a-f]{24}$'),
  CONSTRAINT sarah_fleet_run_intake_leases_owner_shape
    CHECK (owner_user_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{2,159}$'),
  CONSTRAINT sarah_fleet_run_intake_leases_pylon_shape
    CHECK (pylon_ref ~ '^[a-z0-9][a-z0-9._:-]{2,119}$'),
  CONSTRAINT sarah_fleet_run_intake_leases_idempotency_shape
    CHECK (claim_idempotency_key ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{7,119}$'),
  CONSTRAINT sarah_fleet_run_intake_leases_fingerprint_shape
    CHECK (claim_fingerprint ~ '^[0-9a-f]{64}$')
);

CREATE INDEX IF NOT EXISTS sarah_fleet_run_intake_leases_owner_active_idx
  ON sarah_fleet_run_intake_leases
    (owner_user_id, state, lease_expires_at, updated_at DESC);
