-- KS-8.4 (#8315): Pylon control-plane remainder -- Postgres twins for the
-- D1 tables left after KS-8.1's dispatch slice.
--
-- Source tables:
--   pylon_api_quarantines, pylon_marketplace_job_intakes,
--   pylon_marketplace_assignments, pylon_marketplace_triage_actions,
--   pylon_provider_job_lifecycle, pylon_agent_runner_status_events,
--   pylon_capacity_funnel_snapshots, pylon_spark_payout_targets,
--   pylon_codex_raw_events, pylon_codex_raw_event_chunks,
--   runner_sessions, fleet_alerts.
--
-- Boundary notes:
--   * pylon_api_registrations / pylon_api_assignments / pylon_api_events
--     already moved in 0005_pylon_dispatch.sql as pylon_registrations,
--     pylon_assignments, and pylon_assignment_events.
--   * pylon_codex_raw_events and pylon_codex_raw_event_chunks remain metadata
--     indexes only. Payload bodies stay in R2; this schema keeps refs,
--     content digests, sizes, and ordering keys needed for closeout/proof
--     verification.
--   * raw_spark_address is private payment material. It is present here only
--     because Cloud SQL becomes the private authoritative store; it must not
--     be projected into Khala Sync post-images or public logs.
--
-- TYPE FIDELITY (v1): keep D1 byte-compatible representations where exact
-- reconciliation depends on it: TEXT timestamps / JSON-as-text, bigint for
-- D1 INTEGER counters, and nullable text fields matching the source tables.
--
-- IDEMPOTENCY KEYS PORT EXACTLY: every UNIQUE natural key in D1 is preserved
-- so backfill and future mirrors can use INSERT ... ON CONFLICT DO NOTHING
-- with the same duplicate semantics.
--
-- NO CROSS-TABLE FOREIGN KEYS DURING MIGRATION: rows are backfilled and
-- mirrored per table while dispatch tables may be in a separate cutover phase.
-- Set membership and chain contiguity are verified by the backfill tooling
-- before any read cutover.

CREATE TABLE IF NOT EXISTS pylon_quarantines (
  id                       text PRIMARY KEY,
  quarantine_ref           text NOT NULL UNIQUE,
  pylon_ref                text NOT NULL,
  owner_agent_user_id      text,
  state                    text NOT NULL,
  reason_refs_json         text NOT NULL,
  action_refs_json         text NOT NULL,
  source_refs_json         text NOT NULL,
  expires_at               text,
  released_at              text,
  public_projection_json   text NOT NULL,
  created_at               text NOT NULL,
  updated_at               text NOT NULL,
  archived_at              text
);

CREATE INDEX IF NOT EXISTS pylon_quarantines_active_idx
  ON pylon_quarantines (pylon_ref, state, released_at, expires_at, updated_at DESC);

CREATE INDEX IF NOT EXISTS pylon_quarantines_updated_idx
  ON pylon_quarantines (updated_at DESC);

CREATE TABLE IF NOT EXISTS pylon_marketplace_job_intakes (
  id                       text PRIMARY KEY,
  intake_ref               text NOT NULL UNIQUE,
  job_ref                  text NOT NULL,
  idempotency_key          text NOT NULL UNIQUE,
  request_hash             text NOT NULL,
  state                    text NOT NULL,
  source                   text NOT NULL,
  job_kind                 text NOT NULL,
  privacy_class            text NOT NULL,
  record_json              text NOT NULL,
  created_at               text NOT NULL,
  updated_at               text NOT NULL
);

CREATE INDEX IF NOT EXISTS pylon_marketplace_job_intakes_updated_idx
  ON pylon_marketplace_job_intakes (updated_at DESC);

CREATE INDEX IF NOT EXISTS pylon_marketplace_job_intakes_state_updated_idx
  ON pylon_marketplace_job_intakes (state, updated_at DESC);

CREATE TABLE IF NOT EXISTS pylon_marketplace_assignments (
  id                       text PRIMARY KEY,
  assignment_ref           text NOT NULL UNIQUE,
  intake_ref               text NOT NULL,
  job_ref                  text NOT NULL,
  idempotency_key          text NOT NULL UNIQUE,
  request_hash             text NOT NULL,
  state                    text NOT NULL,
  payout_state             text NOT NULL,
  record_json              text NOT NULL,
  created_at               text NOT NULL,
  updated_at               text NOT NULL
);

CREATE INDEX IF NOT EXISTS pylon_marketplace_assignments_intake_updated_idx
  ON pylon_marketplace_assignments (intake_ref, updated_at DESC);

CREATE INDEX IF NOT EXISTS pylon_marketplace_assignments_state_updated_idx
  ON pylon_marketplace_assignments (state, updated_at DESC);

CREATE TABLE IF NOT EXISTS pylon_marketplace_triage_actions (
  id                       text PRIMARY KEY,
  target_intake_ref        text NOT NULL,
  idempotency_key          text NOT NULL UNIQUE,
  request_hash             text NOT NULL,
  outcome                  text NOT NULL,
  response_json            text NOT NULL,
  created_at               text NOT NULL
);

CREATE INDEX IF NOT EXISTS pylon_marketplace_triage_actions_target_idx
  ON pylon_marketplace_triage_actions (target_intake_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS pylon_provider_job_lifecycle (
  id                       text PRIMARY KEY,
  pylon_ref                text NOT NULL,
  assignment_ref           text NOT NULL UNIQUE,
  owner_agent_user_id      text NOT NULL,
  job_kind                 text NOT NULL,
  stage                    text NOT NULL,
  task_refs_json           text NOT NULL,
  artifact_refs_json       text NOT NULL,
  proof_refs_json          text NOT NULL,
  closeout_refs_json       text NOT NULL,
  accepted_work_refs_json  text NOT NULL,
  public_projection_json   text NOT NULL,
  created_at               text NOT NULL,
  updated_at               text NOT NULL,
  archived_at              text
);

CREATE INDEX IF NOT EXISTS pylon_provider_job_lifecycle_pylon_updated_idx
  ON pylon_provider_job_lifecycle (pylon_ref, updated_at DESC);

CREATE INDEX IF NOT EXISTS pylon_provider_job_lifecycle_stage_updated_idx
  ON pylon_provider_job_lifecycle (stage, updated_at DESC);

CREATE TABLE IF NOT EXISTS pylon_agent_runner_status_events (
  event_ref                text PRIMARY KEY,
  owner_agent_user_id      text NOT NULL,
  runner_ref               text NOT NULL,
  runner_kind              text NOT NULL,
  pylon_ref                text,
  assignment_ref           text,
  state                    text NOT NULL,
  state_started_at         text NOT NULL,
  updated_at               text NOT NULL,
  retention_state          text NOT NULL,
  event_json               text NOT NULL,
  created_at               text NOT NULL,
  retained_at              text,
  archived_at              text
);

CREATE INDEX IF NOT EXISTS pylon_agent_runner_status_owner_retention_updated_idx
  ON pylon_agent_runner_status_events
    (owner_agent_user_id, retention_state, updated_at DESC);

CREATE INDEX IF NOT EXISTS pylon_agent_runner_status_owner_runner_live_idx
  ON pylon_agent_runner_status_events
    (owner_agent_user_id, runner_ref, retention_state);

CREATE TABLE IF NOT EXISTS pylon_capacity_funnel_snapshots (
  id                       text PRIMARY KEY,
  bucket_kind              text NOT NULL,
  bucket_start_at          text NOT NULL,
  snapshot_at              text NOT NULL,
  total_count              bigint NOT NULL DEFAULT 0 CHECK (total_count >= 0),
  aggregate_json           text NOT NULL,
  public_projection_json   text NOT NULL,
  created_at               text NOT NULL,
  updated_at               text NOT NULL,
  archived_at              text,
  CONSTRAINT pylon_capacity_funnel_bucket_unique
    UNIQUE (bucket_kind, bucket_start_at)
);

CREATE INDEX IF NOT EXISTS pylon_capacity_funnel_snapshots_bucket_start_idx
  ON pylon_capacity_funnel_snapshots (bucket_kind, bucket_start_at DESC);

CREATE INDEX IF NOT EXISTS pylon_capacity_funnel_snapshots_updated_idx
  ON pylon_capacity_funnel_snapshots (updated_at DESC);

CREATE TABLE IF NOT EXISTS pylon_spark_payout_targets (
  pylon_ref                text PRIMARY KEY,
  owner_agent_user_id      text NOT NULL,
  payout_target_ref        text NOT NULL,
  raw_spark_address        text NOT NULL,
  created_at               text NOT NULL,
  updated_at               text NOT NULL
);

CREATE INDEX IF NOT EXISTS pylon_spark_payout_targets_owner_idx
  ON pylon_spark_payout_targets (owner_agent_user_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS pylon_codex_raw_events (
  raw_event_ref            text PRIMARY KEY,
  assignment_ref           text NOT NULL,
  lease_ref                text NOT NULL,
  pylon_ref                text NOT NULL,
  owner_user_id            text NOT NULL,
  run_ref                  text,
  session_ref              text,
  workspace_ref            text,
  turn_index               bigint NOT NULL,
  event_count              bigint NOT NULL DEFAULT 0 CHECK (event_count >= 0),
  byte_length              bigint NOT NULL DEFAULT 0 CHECK (byte_length >= 0),
  content_digest           text NOT NULL UNIQUE,
  r2_key                   text NOT NULL,
  observed_at              text NOT NULL,
  created_at               text NOT NULL,
  updated_at               text NOT NULL,
  demand_kind              text NOT NULL DEFAULT 'own_capacity',
  demand_source            text NOT NULL DEFAULT 'khala_coding_delegation',
  CONSTRAINT pylon_codex_raw_events_turn_unique
    UNIQUE (assignment_ref, lease_ref, pylon_ref, turn_index)
);

CREATE INDEX IF NOT EXISTS pylon_codex_raw_events_owner_observed_idx
  ON pylon_codex_raw_events (owner_user_id, observed_at DESC);

CREATE INDEX IF NOT EXISTS pylon_codex_raw_events_assignment_turn_idx
  ON pylon_codex_raw_events (assignment_ref, turn_index);

CREATE INDEX IF NOT EXISTS pylon_codex_raw_events_session_turn_idx
  ON pylon_codex_raw_events (session_ref, turn_index);

CREATE TABLE IF NOT EXISTS pylon_codex_raw_event_chunks (
  chunk_ref                text PRIMARY KEY,
  assignment_ref           text NOT NULL,
  lease_ref                text NOT NULL,
  pylon_ref                text NOT NULL,
  owner_user_id            text NOT NULL,
  run_ref                  text,
  session_ref              text,
  workspace_ref            text,
  turn_index               bigint NOT NULL,
  chunk_index              bigint NOT NULL,
  event_count              bigint NOT NULL DEFAULT 0 CHECK (event_count >= 0),
  byte_length              bigint NOT NULL DEFAULT 0 CHECK (byte_length >= 0),
  content_digest           text NOT NULL UNIQUE,
  r2_key                   text NOT NULL,
  observed_at              text NOT NULL,
  created_at               text NOT NULL,
  updated_at               text NOT NULL,
  demand_kind              text NOT NULL DEFAULT 'own_capacity',
  demand_source            text NOT NULL DEFAULT 'khala_coding_delegation',
  CONSTRAINT pylon_codex_raw_event_chunks_chunk_unique
    UNIQUE (assignment_ref, lease_ref, pylon_ref, turn_index, chunk_index)
);

CREATE INDEX IF NOT EXISTS pylon_codex_raw_event_chunks_owner_observed_idx
  ON pylon_codex_raw_event_chunks (owner_user_id, observed_at DESC);

CREATE INDEX IF NOT EXISTS pylon_codex_raw_event_chunks_assignment_turn_chunk_idx
  ON pylon_codex_raw_event_chunks (assignment_ref, turn_index, chunk_index);

CREATE INDEX IF NOT EXISTS pylon_codex_raw_event_chunks_session_turn_chunk_idx
  ON pylon_codex_raw_event_chunks (session_ref, turn_index, chunk_index);

CREATE TABLE IF NOT EXISTS runner_sessions (
  id                       text PRIMARY KEY,
  runner_id                text NOT NULL,
  lane                     text NOT NULL,
  backend                  text NOT NULL,
  status                   text NOT NULL,
  team_id                  text,
  thread_id                text,
  workroom_id              text,
  provider_account_ref     text,
  active_auth_grant_ref    text,
  opencode_server_url      text,
  opencode_server_auth_ref text,
  created_at               text NOT NULL,
  updated_at               text NOT NULL,
  started_at               text,
  completed_at             text,
  failed_at                text
);

CREATE INDEX IF NOT EXISTS runner_sessions_thread_created_idx
  ON runner_sessions (thread_id, created_at);

CREATE INDEX IF NOT EXISTS runner_sessions_status_idx
  ON runner_sessions (status);

CREATE TABLE IF NOT EXISTS fleet_alerts (
  id                       text PRIMARY KEY,
  alert_ref                text NOT NULL UNIQUE,
  detected_at              text NOT NULL,
  classification           text NOT NULL,
  reason_ref               text NOT NULL,
  burn_tokens_window       bigint NOT NULL DEFAULT 0 CHECK (burn_tokens_window >= 0),
  window_minutes           bigint NOT NULL DEFAULT 0 CHECK (window_minutes >= 0),
  stall_threshold_tokens   bigint NOT NULL DEFAULT 0 CHECK (stall_threshold_tokens >= 0),
  active_assignments       bigint NOT NULL DEFAULT 0 CHECK (active_assignments >= 0),
  queued_assignments       bigint NOT NULL DEFAULT 0 CHECK (queued_assignments >= 0),
  recovery_actions_json    text NOT NULL,
  recovered_lease_count    bigint NOT NULL DEFAULT 0 CHECK (recovered_lease_count >= 0),
  created_at               text NOT NULL
);

CREATE INDEX IF NOT EXISTS fleet_alerts_detected_at_idx
  ON fleet_alerts (detected_at DESC);

CREATE INDEX IF NOT EXISTS fleet_alerts_classification_detected_idx
  ON fleet_alerts (classification, detected_at DESC);
