-- FC-2 (#8633): durable, owner-scoped FleetRun execution projection.
--
-- The Pylon-local orchestration database remains the one work-claim registry.
-- These tables accept only the bounded refs-only lifecycle/outcome projection
-- for the exact Sarah intake lease already accepted by that Pylon. They do not
-- carry prompts, output, worktree paths, credentials, raw provider events, or
-- another executable claim.

ALTER TABLE sarah_fleet_run_requests
  ADD COLUMN IF NOT EXISTS execution_state text NOT NULL DEFAULT 'pending'
    CHECK (execution_state IN ('pending', 'running', 'completed', 'failed', 'stopped')),
  ADD COLUMN IF NOT EXISTS execution_last_sequence bigint NOT NULL DEFAULT 0
    CHECK (execution_last_sequence >= 0),
  ADD COLUMN IF NOT EXISTS execution_started_at text,
  ADD COLUMN IF NOT EXISTS execution_updated_at text;

CREATE TABLE IF NOT EXISTS sarah_fleet_run_execution_events (
  run_ref          text NOT NULL,
  sequence         bigint NOT NULL CHECK (sequence >= 1),
  event_ref        text NOT NULL UNIQUE,
  owner_user_id    text NOT NULL,
  pylon_ref        text NOT NULL,
  intake_claim_ref text NOT NULL,
  event_kind       text NOT NULL
    CHECK (event_kind IN ('run_started', 'work_progress', 'work_terminal', 'run_terminal')),
  unit_ref         text,
  work_claim_ref   text,
  event_json       text NOT NULL,
  observed_at      text NOT NULL,
  recorded_at      text NOT NULL,
  PRIMARY KEY (run_ref, sequence),
  CONSTRAINT sarah_fleet_run_execution_events_run_fk
    FOREIGN KEY (run_ref) REFERENCES sarah_fleet_run_requests(run_ref)
      ON DELETE CASCADE,
  CONSTRAINT sarah_fleet_run_execution_events_claim_fk
    FOREIGN KEY (intake_claim_ref) REFERENCES sarah_fleet_run_intake_leases(claim_ref),
  CONSTRAINT sarah_fleet_run_execution_events_run_ref_shape
    CHECK (run_ref ~ '^fleet_run\.sarah\.[0-9a-f]{20}$'),
  CONSTRAINT sarah_fleet_run_execution_events_event_ref_shape
    CHECK (event_ref ~ '^event\.pylon\.fleet_run\.[0-9a-f]{24}$'),
  CONSTRAINT sarah_fleet_run_execution_events_owner_shape
    CHECK (owner_user_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{2,159}$'),
  CONSTRAINT sarah_fleet_run_execution_events_pylon_shape
    CHECK (pylon_ref ~ '^[a-z0-9][a-z0-9._:-]{2,119}$'),
  CONSTRAINT sarah_fleet_run_execution_events_claim_shape
    CHECK (intake_claim_ref ~ '^claim\.sarah_fleet_run\.[0-9a-f]{24}$'),
  CONSTRAINT sarah_fleet_run_execution_events_unit_shape
    CHECK (unit_ref IS NULL OR unit_ref ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{0,159}$'),
  CONSTRAINT sarah_fleet_run_execution_events_work_claim_shape
    CHECK (work_claim_ref IS NULL OR work_claim_ref ~ '^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,179}$'),
  CONSTRAINT sarah_fleet_run_execution_events_work_identity_coherence
    CHECK ((unit_ref IS NULL) = (work_claim_ref IS NULL))
);

CREATE INDEX IF NOT EXISTS sarah_fleet_run_execution_events_owner_run_idx
  ON sarah_fleet_run_execution_events (owner_user_id, run_ref, sequence);

CREATE INDEX IF NOT EXISTS sarah_fleet_run_execution_events_attempt_idx
  ON sarah_fleet_run_execution_events (run_ref, work_claim_ref, sequence)
  WHERE work_claim_ref IS NOT NULL;

CREATE TABLE IF NOT EXISTS sarah_fleet_run_work_unit_closeouts (
  run_ref              text NOT NULL,
  unit_ref             text NOT NULL,
  work_claim_ref       text NOT NULL,
  assignment_ref       text,
  worker_kind          text NOT NULL CHECK (worker_kind IN ('codex', 'claude', 'grok')),
  account_ref_hash     text,
  terminal_state       text NOT NULL CHECK (terminal_state IN ('accepted', 'failed', 'stale')),
  closeout_ref         text,
  usage_truth          text CHECK (usage_truth IN ('exact', 'not_measured')),
  token_usage_refs_json text,
  blocker_refs_json    text NOT NULL,
  observed_at          text NOT NULL,
  event_ref            text NOT NULL,
  PRIMARY KEY (run_ref, work_claim_ref),
  UNIQUE (run_ref, assignment_ref),
  UNIQUE (event_ref),
  CONSTRAINT sarah_fleet_run_work_unit_closeouts_unit_fk
    FOREIGN KEY (run_ref, unit_ref)
      REFERENCES sarah_fleet_run_work_units(run_ref, unit_ref)
      ON DELETE CASCADE,
  CONSTRAINT sarah_fleet_run_work_unit_closeouts_event_fk
    FOREIGN KEY (event_ref) REFERENCES sarah_fleet_run_execution_events(event_ref)
      ON DELETE CASCADE,
  CONSTRAINT sarah_fleet_run_work_unit_closeouts_work_claim_shape
    CHECK (work_claim_ref ~ '^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,179}$'),
  CONSTRAINT sarah_fleet_run_work_unit_closeouts_assignment_shape
    CHECK (assignment_ref IS NULL OR assignment_ref ~ '^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,179}$'),
  CONSTRAINT sarah_fleet_run_work_unit_closeouts_account_hash_shape
    CHECK (account_ref_hash IS NULL OR account_ref_hash ~ '^account\.pylon\.(codex|claude_agent|grok)\.[a-f0-9]{24}$'),
  CONSTRAINT sarah_fleet_run_work_unit_closeouts_closeout_shape
    CHECK (closeout_ref IS NULL OR closeout_ref ~ '^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,179}$'),
  CONSTRAINT sarah_fleet_run_work_unit_closeouts_proof_coherence
    CHECK (
      (assignment_ref IS NULL AND account_ref_hash IS NULL
       AND closeout_ref IS NULL AND usage_truth IS NULL
       AND token_usage_refs_json IS NULL)
      OR
      (assignment_ref IS NOT NULL AND account_ref_hash IS NOT NULL
       AND closeout_ref IS NOT NULL AND usage_truth IS NOT NULL
       AND token_usage_refs_json IS NOT NULL)
    ),
  CONSTRAINT sarah_fleet_run_work_unit_closeouts_accepted_proof
    CHECK (terminal_state <> 'accepted' OR assignment_ref IS NOT NULL)
);

CREATE INDEX IF NOT EXISTS sarah_fleet_run_work_unit_closeouts_state_idx
  ON sarah_fleet_run_work_unit_closeouts
    (run_ref, terminal_state, observed_at);

CREATE INDEX IF NOT EXISTS sarah_fleet_run_work_unit_closeouts_unit_idx
  ON sarah_fleet_run_work_unit_closeouts
    (run_ref, unit_ref, terminal_state, observed_at);
