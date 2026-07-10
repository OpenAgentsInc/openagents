-- FC-3 (#8639): first-class, owner-private FleetRun work-unit and attempt
-- materialization for body-free Sarah supervision.
--
-- `unit_ref` is stable plan identity. `attempt_ref` is exactly the Pylon
-- `work_claim_ref`; `assignment_ref` is only an optional graph edge. The
-- server derives owner, accepted intake claim, Pylon, and receipt clocks from
-- the authenticated authority transaction instead of trusting event payloads.

ALTER TABLE sarah_fleet_run_work_units
  ADD COLUMN IF NOT EXISTS state text,
  ADD COLUMN IF NOT EXISTS latest_attempt_ref text,
  ADD COLUMN IF NOT EXISTS accepted_attempt_ref text,
  ADD COLUMN IF NOT EXISTS updated_at text;

UPDATE sarah_fleet_run_work_units AS unit
SET state = COALESCE(unit.state, 'planned'),
    updated_at = COALESCE(unit.updated_at, request.created_at)
FROM sarah_fleet_run_requests AS request
WHERE request.run_ref = unit.run_ref
  AND (unit.state IS NULL OR unit.updated_at IS NULL);

ALTER TABLE sarah_fleet_run_work_units
  ALTER COLUMN state SET NOT NULL,
  ALTER COLUMN updated_at SET NOT NULL;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conname = 'sarah_fleet_run_work_units_state_check'
  ) THEN
    ALTER TABLE sarah_fleet_run_work_units
      ADD CONSTRAINT sarah_fleet_run_work_units_state_check
      CHECK (state IN (
        'planned', 'running', 'verification_pending',
        'succeeded', 'failed', 'stale'
      ));
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conname = 'sarah_fleet_run_work_units_pointer_coherence'
  ) THEN
    ALTER TABLE sarah_fleet_run_work_units
      ADD CONSTRAINT sarah_fleet_run_work_units_pointer_coherence
      CHECK (
        (state = 'planned'
          AND latest_attempt_ref IS NULL
          AND accepted_attempt_ref IS NULL)
        OR
        (state = 'succeeded'
          AND latest_attempt_ref IS NOT NULL
          AND accepted_attempt_ref = latest_attempt_ref)
        OR
        (state IN ('running', 'verification_pending', 'failed', 'stale')
          AND latest_attempt_ref IS NOT NULL
          AND accepted_attempt_ref IS NULL)
      );
  END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS sarah_fleet_run_intake_leases_attempt_binding
  ON sarah_fleet_run_intake_leases
    (claim_ref, run_ref, owner_user_id, pylon_ref);

CREATE TABLE IF NOT EXISTS sarah_fleet_run_attempts (
  run_ref                text NOT NULL,
  attempt_ref            text NOT NULL,
  work_unit_ref          text NOT NULL,
  owner_user_id          text NOT NULL,
  intake_claim_ref       text NOT NULL,
  pylon_ref              text NOT NULL,
  worker_kind            text NOT NULL
    CHECK (worker_kind IN ('codex', 'claude', 'grok')),
  state                   text NOT NULL
    CHECK (state IN (
      'running', 'evidence_pending', 'succeeded', 'failed', 'stale'
    )),
  progress_class          text NOT NULL
    CHECK (progress_class IN ('active', 'blocked', 'terminal')),
  assignment_ref          text,
  account_ref_hash        text,
  capacity_class          text NOT NULL CHECK (capacity_class = 'owner_local'),
  marginal_cost_class     text NOT NULL
    CHECK (marginal_cost_class IN ('owner_capacity', 'not_reported')),
  verification_json       text NOT NULL,
  artifact_refs_json      text NOT NULL,
  proof_refs_json         text NOT NULL,
  authority_receipt_refs_json text NOT NULL,
  closeout_ref            text,
  usage_truth             text NOT NULL
    CHECK (usage_truth IN ('pending', 'exact', 'not_measured')),
  token_usage_refs_json   text NOT NULL,
  blocker_refs_json       text NOT NULL,
  last_event_ref          text NOT NULL,
  first_observed_at       text NOT NULL,
  last_observed_at        text NOT NULL,
  started_at              text NOT NULL,
  terminal_at             text,
  updated_at              text NOT NULL,
  PRIMARY KEY (run_ref, attempt_ref),
  CONSTRAINT sarah_fleet_run_attempts_unit_fk
    FOREIGN KEY (run_ref, work_unit_ref)
      REFERENCES sarah_fleet_run_work_units(run_ref, unit_ref)
      ON DELETE CASCADE,
  CONSTRAINT sarah_fleet_run_attempts_claim_fk
    FOREIGN KEY (intake_claim_ref, run_ref, owner_user_id, pylon_ref)
      REFERENCES sarah_fleet_run_intake_leases
        (claim_ref, run_ref, owner_user_id, pylon_ref),
  CONSTRAINT sarah_fleet_run_attempts_event_fk
    FOREIGN KEY (last_event_ref)
      REFERENCES sarah_fleet_run_execution_events(event_ref),
  CONSTRAINT sarah_fleet_run_attempts_owner_shape
    CHECK (owner_user_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{2,159}$'),
  CONSTRAINT sarah_fleet_run_attempts_attempt_shape
    CHECK (attempt_ref ~ '^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,179}$'),
  CONSTRAINT sarah_fleet_run_attempts_pylon_shape
    CHECK (pylon_ref ~ '^[a-z0-9][a-z0-9._:-]{2,119}$'),
  CONSTRAINT sarah_fleet_run_attempts_claim_shape
    CHECK (intake_claim_ref ~ '^claim\.sarah_fleet_run\.[0-9a-f]{24}$'),
  CONSTRAINT sarah_fleet_run_attempts_assignment_shape
    CHECK (
      assignment_ref IS NULL OR
      assignment_ref ~ '^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,179}$'
    ),
  CONSTRAINT sarah_fleet_run_attempts_account_hash_shape
    CHECK (
      account_ref_hash IS NULL OR
      account_ref_hash ~ '^account\.pylon\.(codex|claude_agent|grok)\.[a-f0-9]{24}$'
    ),
  CONSTRAINT sarah_fleet_run_attempts_closeout_shape
    CHECK (
      closeout_ref IS NULL OR
      closeout_ref ~ '^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,179}$'
    ),
  CONSTRAINT sarah_fleet_run_attempts_terminal_clock_coherence
    CHECK ((state = 'running') = (terminal_at IS NULL)),
  CONSTRAINT sarah_fleet_run_attempts_worker_account_coherence
    CHECK (
      account_ref_hash IS NULL OR
      (worker_kind = 'codex'
        AND account_ref_hash ~ '^account\.pylon\.codex\.[a-f0-9]{24}$') OR
      (worker_kind = 'claude'
        AND account_ref_hash ~ '^account\.pylon\.claude_agent\.[a-f0-9]{24}$') OR
      (worker_kind = 'grok'
        AND account_ref_hash ~ '^account\.pylon\.grok\.[a-f0-9]{24}$')
    ),
  CONSTRAINT sarah_fleet_run_attempts_evidence_coherence
    CHECK (
      (state = 'running'
        AND progress_class IN ('active', 'blocked')
        AND verification_json = '{"truth":"pending"}'
        AND artifact_refs_json = '[]'
        AND proof_refs_json = '[]'
        AND authority_receipt_refs_json = '[]'
        AND closeout_ref IS NULL
        AND usage_truth = 'pending'
        AND token_usage_refs_json = '[]')
      OR
      (state = 'evidence_pending'
        AND progress_class = 'terminal'
        AND verification_json = '{"truth":"not_reported"}'
        AND artifact_refs_json = '[]'
        AND proof_refs_json = '[]'
        AND authority_receipt_refs_json = '[]'
        AND closeout_ref IS NOT NULL
        AND usage_truth <> 'pending')
      OR
      (state = 'succeeded'
        AND progress_class = 'terminal'
        AND verification_json <> '{"truth":"pending"}'
        AND verification_json <> '{"truth":"not_reported"}'
        AND artifact_refs_json <> '[]'
        AND proof_refs_json <> '[]'
        AND authority_receipt_refs_json <> '[]'
        AND closeout_ref IS NOT NULL
        AND usage_truth <> 'pending'
        AND blocker_refs_json = '[]')
      OR
      (state IN ('failed', 'stale')
        AND progress_class = 'terminal'
        AND verification_json <> '{"truth":"pending"}')
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS sarah_fleet_run_attempts_assignment_unique
  ON sarah_fleet_run_attempts (run_ref, assignment_ref)
  WHERE assignment_ref IS NOT NULL;

CREATE INDEX IF NOT EXISTS sarah_fleet_run_attempts_owner_run_idx
  ON sarah_fleet_run_attempts
    (owner_user_id, run_ref, updated_at DESC, attempt_ref);

CREATE INDEX IF NOT EXISTS sarah_fleet_run_attempts_unit_idx
  ON sarah_fleet_run_attempts
    (run_ref, work_unit_ref, updated_at DESC, attempt_ref);

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conname = 'sarah_fleet_run_work_units_latest_attempt_fk'
  ) THEN
    ALTER TABLE sarah_fleet_run_work_units
      ADD CONSTRAINT sarah_fleet_run_work_units_latest_attempt_fk
      FOREIGN KEY (run_ref, latest_attempt_ref)
      REFERENCES sarah_fleet_run_attempts(run_ref, attempt_ref);
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conname = 'sarah_fleet_run_work_units_accepted_attempt_fk'
  ) THEN
    ALTER TABLE sarah_fleet_run_work_units
      ADD CONSTRAINT sarah_fleet_run_work_units_accepted_attempt_fk
      FOREIGN KEY (run_ref, accepted_attempt_ref)
      REFERENCES sarah_fleet_run_attempts(run_ref, attempt_ref);
  END IF;
END $$;
