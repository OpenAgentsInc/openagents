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
    CHECK (marginal_cost_class IN (
      'free', 'subscription', 'api_metered', 'not_measured'
    )),
  verification_json       text NOT NULL,
  artifact_refs_json      text NOT NULL,
  proof_refs_json         text NOT NULL,
  authority_receipt_refs_json text NOT NULL,
  closeout_ref            text,
  usage_json              text NOT NULL,
  usage_truth             text NOT NULL
    CHECK (usage_truth IN ('pending', 'exact', 'not_measured')),
  usage_evidence_ref      text,
  usage_provider          text,
  usage_model             text,
  usage_demand_kind       text,
  usage_demand_source     text,
  usage_input_tokens      bigint,
  usage_output_tokens     bigint,
  usage_reasoning_tokens  bigint,
  usage_cache_read_tokens bigint,
  usage_total_tokens      bigint,
  usage_token_rows        bigint,
  token_usage_refs_json   text NOT NULL,
  blocker_refs_json       text NOT NULL,
  last_event_ref          text NOT NULL,
  first_remote_observed_at text NOT NULL,
  remote_observed_at      text NOT NULL,
  last_observed_at        text NOT NULL,
  started_at              text NOT NULL,
  terminal_at             text,
  updated_at              text NOT NULL,
  PRIMARY KEY (run_ref, attempt_ref),
  UNIQUE (attempt_ref),
  UNIQUE (run_ref, work_unit_ref, attempt_ref),
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
  CONSTRAINT sarah_fleet_run_attempts_usage_columns_coherence
    CHECK (
      (usage_truth = 'pending'
        AND usage_json = '{"truth":"pending"}'
        AND usage_evidence_ref IS NULL
        AND usage_provider IS NULL
        AND usage_model IS NULL
        AND usage_demand_kind IS NULL
        AND usage_demand_source IS NULL
        AND usage_input_tokens IS NULL
        AND usage_output_tokens IS NULL
        AND usage_reasoning_tokens IS NULL
        AND usage_cache_read_tokens IS NULL
        AND usage_total_tokens IS NULL
        AND usage_token_rows IS NULL
        AND token_usage_refs_json = '[]')
      OR
      (usage_truth = 'exact'
        AND usage_json::jsonb ->> 'truth' = 'exact'
        AND usage_evidence_ref IS NOT NULL
        AND usage_provider IN (
          'pylon-codex-own-capacity', 'pylon-claude-own-capacity'
        )
        AND usage_model IN (
          'openagents/pylon-codex', 'openagents/pylon-claude'
        )
        AND usage_demand_kind = 'own_capacity'
        AND usage_demand_source = 'khala_coding_delegation'
        AND usage_input_tokens >= 0
        AND usage_output_tokens >= 0
        AND usage_reasoning_tokens >= 0
        AND usage_reasoning_tokens <= usage_output_tokens
        AND usage_cache_read_tokens >= 0
        AND usage_cache_read_tokens <= usage_input_tokens
        AND usage_total_tokens > 0
        AND usage_total_tokens = usage_input_tokens + usage_output_tokens
        AND usage_token_rows > 0
        AND token_usage_refs_json <> '[]')
      OR
      (usage_truth = 'not_measured'
        AND usage_json::jsonb ->> 'truth' = 'not_measured'
        AND usage_evidence_ref IS NOT NULL
        AND usage_provider IS NULL
        AND usage_model IS NULL
        AND usage_demand_kind IS NULL
        AND usage_demand_source IS NULL
        AND usage_input_tokens IS NULL
        AND usage_output_tokens IS NULL
        AND usage_reasoning_tokens IS NULL
        AND usage_cache_read_tokens IS NULL
        AND usage_total_tokens IS NULL
        AND usage_token_rows IS NULL
        AND token_usage_refs_json = '[]')
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
        AND usage_json = '{"truth":"pending"}'
        AND usage_truth = 'pending'
        AND usage_evidence_ref IS NULL
        AND usage_provider IS NULL
        AND usage_model IS NULL
        AND usage_demand_kind IS NULL
        AND usage_demand_source IS NULL
        AND usage_input_tokens IS NULL
        AND usage_output_tokens IS NULL
        AND usage_reasoning_tokens IS NULL
        AND usage_cache_read_tokens IS NULL
        AND usage_total_tokens IS NULL
        AND usage_token_rows IS NULL
        AND token_usage_refs_json = '[]')
      OR
      (state = 'evidence_pending'
        AND progress_class = 'terminal'
        AND verification_json = '{"truth":"not_reported"}'
        AND artifact_refs_json = '[]'
        AND proof_refs_json = '[]'
        AND authority_receipt_refs_json = '[]'
        AND closeout_ref IS NOT NULL
        AND usage_json = '{"truth":"pending"}'
        AND usage_truth = 'pending'
        AND usage_evidence_ref IS NULL
        AND token_usage_refs_json = '[]'
        AND blocker_refs_json = '[]')
      OR
      (state = 'succeeded'
        AND progress_class = 'terminal'
        AND verification_json::jsonb ->> 'truth' = 'passed'
        AND artifact_refs_json <> '[]'
        AND proof_refs_json <> '[]'
        AND authority_receipt_refs_json <> '[]'
        AND closeout_ref IS NOT NULL
        AND usage_truth <> 'pending'
        AND usage_evidence_ref IS NOT NULL
        AND usage_json::jsonb ->> 'schema' =
          'openagents.pylon.fleet_run_usage_evidence.v1'
        AND blocker_refs_json = '[]')
      OR
      (state IN ('failed', 'stale')
        AND progress_class = 'terminal'
        AND verification_json <> '{"truth":"pending"}'
        AND blocker_refs_json <> '[]')
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
      FOREIGN KEY (run_ref, unit_ref, latest_attempt_ref)
      REFERENCES sarah_fleet_run_attempts
        (run_ref, work_unit_ref, attempt_ref);
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conname = 'sarah_fleet_run_work_units_accepted_attempt_fk'
  ) THEN
    ALTER TABLE sarah_fleet_run_work_units
      ADD CONSTRAINT sarah_fleet_run_work_units_accepted_attempt_fk
      FOREIGN KEY (run_ref, unit_ref, accepted_attempt_ref)
      REFERENCES sarah_fleet_run_attempts
        (run_ref, work_unit_ref, attempt_ref);
  END IF;
END $$;

CREATE OR REPLACE FUNCTION sarah_fleet_run_attempt_pointer_guard()
RETURNS trigger AS $$
DECLARE
  accepted_state text;
BEGIN
  IF NEW.accepted_attempt_ref IS NOT NULL THEN
    SELECT attempt.state INTO accepted_state
    FROM sarah_fleet_run_attempts AS attempt
    WHERE attempt.run_ref = NEW.run_ref
      AND attempt.work_unit_ref = NEW.unit_ref
      AND attempt.attempt_ref = NEW.accepted_attempt_ref;
    IF accepted_state IS DISTINCT FROM 'succeeded' THEN
      RAISE EXCEPTION 'accepted fleet attempt must be succeeded'
        USING ERRCODE = '23514';
    END IF;
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS sarah_fleet_run_attempt_pointer_guard_trigger
  ON sarah_fleet_run_work_units;
CREATE TRIGGER sarah_fleet_run_attempt_pointer_guard_trigger
  BEFORE INSERT OR UPDATE OF latest_attempt_ref, accepted_attempt_ref, state
  ON sarah_fleet_run_work_units
  FOR EACH ROW EXECUTE FUNCTION sarah_fleet_run_attempt_pointer_guard();

-- Deterministic replay repair for events persisted before 0056. Legacy v1
-- accepted closeouts have refs-only usage and no verifier/artifact/authority
-- proof, so they become evidence_pending (never succeeded). The function is
-- retained as an operator-safe repair job and is invoked once below.
CREATE OR REPLACE FUNCTION sarah_backfill_fleet_run_attempts_v1()
RETURNS integer AS $$
DECLARE
  repaired_count integer := 0;
BEGIN
  IF EXISTS (
    SELECT 1
    FROM sarah_fleet_run_execution_events
    WHERE work_claim_ref IS NOT NULL
    GROUP BY work_claim_ref
    HAVING count(DISTINCT run_ref) > 1
  ) THEN
    RAISE EXCEPTION 'legacy fleet attempt ref is not globally unique'
      USING ERRCODE = '23505';
  END IF;

  CREATE TEMP TABLE IF NOT EXISTS sarah_fleet_attempt_repair_refs (
    run_ref text NOT NULL,
    attempt_ref text NOT NULL,
    PRIMARY KEY (run_ref, attempt_ref)
  ) ON COMMIT DROP;
  TRUNCATE sarah_fleet_attempt_repair_refs;

  CREATE TEMP TABLE IF NOT EXISTS sarah_fleet_attempt_repair_runs (
    run_ref text PRIMARY KEY
  ) ON COMMIT DROP;
  TRUNCATE sarah_fleet_attempt_repair_runs;

  CREATE TEMP TABLE IF NOT EXISTS sarah_fleet_attempt_repair_entities (
    scope text NOT NULL,
    entity_type text NOT NULL,
    entity_id text NOT NULL,
    post_image_json jsonb NOT NULL,
    PRIMARY KEY (scope, entity_type, entity_id)
  ) ON COMMIT DROP;
  TRUNCATE sarah_fleet_attempt_repair_entities;

  WITH inserted AS (
    INSERT INTO sarah_fleet_run_attempts
      (run_ref, attempt_ref, work_unit_ref, owner_user_id, intake_claim_ref,
       pylon_ref, worker_kind, state, progress_class, assignment_ref,
       account_ref_hash, capacity_class, marginal_cost_class,
       verification_json, artifact_refs_json, proof_refs_json,
       authority_receipt_refs_json, closeout_ref, usage_json, usage_truth,
       usage_evidence_ref, usage_provider, usage_model, usage_demand_kind,
       usage_demand_source, usage_input_tokens, usage_output_tokens,
       usage_reasoning_tokens, usage_cache_read_tokens, usage_total_tokens,
       usage_token_rows, token_usage_refs_json, blocker_refs_json,
       last_event_ref, first_remote_observed_at, remote_observed_at,
       last_observed_at, started_at, terminal_at, updated_at)
    SELECT
      closeout.run_ref,
      closeout.work_claim_ref,
      closeout.unit_ref,
      event.owner_user_id,
      event.intake_claim_ref,
      event.pylon_ref,
      closeout.worker_kind,
      CASE closeout.terminal_state
        WHEN 'accepted' THEN 'evidence_pending'
        ELSE closeout.terminal_state
      END,
      'terminal',
      closeout.assignment_ref,
      closeout.account_ref_hash,
      'owner_local',
      'not_measured',
      '{"truth":"not_reported"}',
      '[]',
      '[]',
      '[]',
      closeout.closeout_ref,
      '{"truth":"pending"}',
      'pending',
      NULL, NULL, NULL, NULL, NULL,
      NULL, NULL, NULL, NULL, NULL, NULL,
      '[]',
      CASE
        WHEN closeout.terminal_state = 'accepted' THEN '[]'
        WHEN closeout.blocker_refs_json = '[]' THEN
          '["blocker.pylon.fleet_run.legacy_terminal_without_blocker"]'
        ELSE closeout.blocker_refs_json
      END,
      closeout.event_ref,
      closeout.observed_at,
      closeout.observed_at,
      event.recorded_at,
      event.recorded_at,
      event.recorded_at,
      event.recorded_at
    FROM sarah_fleet_run_work_unit_closeouts AS closeout
    JOIN sarah_fleet_run_execution_events AS event
      ON event.event_ref = closeout.event_ref
    ON CONFLICT (attempt_ref) DO NOTHING
    RETURNING run_ref, attempt_ref
  )
  INSERT INTO sarah_fleet_attempt_repair_refs (run_ref, attempt_ref)
  SELECT run_ref, attempt_ref FROM inserted;

  WITH progress AS (
    SELECT DISTINCT ON (event.run_ref, event.work_claim_ref)
      event.*,
      request.execution_state AS run_execution_state,
      min(event.observed_at) OVER (
        PARTITION BY event.run_ref, event.work_claim_ref
      ) AS first_remote_observed_at,
      min(event.recorded_at) OVER (
        PARTITION BY event.run_ref, event.work_claim_ref
      ) AS first_recorded_at
    FROM sarah_fleet_run_execution_events AS event
    JOIN sarah_fleet_run_requests AS request
      ON request.run_ref = event.run_ref
    LEFT JOIN sarah_fleet_run_work_unit_closeouts AS closeout
      ON closeout.run_ref = event.run_ref
     AND closeout.work_claim_ref = event.work_claim_ref
    WHERE event.event_kind = 'work_progress'
      AND event.work_claim_ref IS NOT NULL
      AND closeout.work_claim_ref IS NULL
    ORDER BY event.run_ref, event.work_claim_ref, event.sequence DESC
  ), inserted AS (
    INSERT INTO sarah_fleet_run_attempts
      (run_ref, attempt_ref, work_unit_ref, owner_user_id, intake_claim_ref,
       pylon_ref, worker_kind, state, progress_class, assignment_ref,
       account_ref_hash, capacity_class, marginal_cost_class,
       verification_json, artifact_refs_json, proof_refs_json,
       authority_receipt_refs_json, closeout_ref, usage_json, usage_truth,
       usage_evidence_ref, usage_provider, usage_model, usage_demand_kind,
       usage_demand_source, usage_input_tokens, usage_output_tokens,
       usage_reasoning_tokens, usage_cache_read_tokens, usage_total_tokens,
       usage_token_rows, token_usage_refs_json, blocker_refs_json,
       last_event_ref, first_remote_observed_at, remote_observed_at,
       last_observed_at, started_at, terminal_at, updated_at)
    SELECT
      progress.run_ref,
      progress.work_claim_ref,
      progress.unit_ref,
      progress.owner_user_id,
      progress.intake_claim_ref,
      progress.pylon_ref,
      progress.event_json::jsonb ->> 'workerKind',
      CASE
        WHEN progress.run_execution_state IN ('completed', 'failed', 'stopped')
          THEN 'stale'
        ELSE 'running'
      END,
      CASE
        WHEN progress.run_execution_state IN ('completed', 'failed', 'stopped')
          THEN 'terminal'
        WHEN progress.event_json::jsonb -> 'blockerRefs' = '[]'::jsonb
          THEN 'active'
        ELSE 'blocked'
      END,
      progress.event_json::jsonb ->> 'assignmentRef',
      progress.event_json::jsonb ->> 'accountRefHash',
      'owner_local',
      'not_measured',
      CASE
        WHEN progress.run_execution_state IN ('completed', 'failed', 'stopped')
          THEN '{"truth":"not_reported"}'
        ELSE '{"truth":"pending"}'
      END,
      '[]', '[]', '[]', NULL,
      '{"truth":"pending"}', 'pending',
      NULL, NULL, NULL, NULL, NULL,
      NULL, NULL, NULL, NULL, NULL, NULL,
      '[]',
      CASE
        WHEN progress.run_execution_state IN ('completed', 'failed', 'stopped')
          THEN '["blocker.pylon.fleet_run.legacy_terminal_with_active_attempt"]'
        ELSE (progress.event_json::jsonb -> 'blockerRefs')::text
      END,
      progress.event_ref,
      progress.first_remote_observed_at,
      progress.observed_at,
      progress.recorded_at,
      progress.first_recorded_at,
      CASE
        WHEN progress.run_execution_state IN ('completed', 'failed', 'stopped')
          THEN progress.recorded_at
        ELSE NULL
      END,
      progress.recorded_at
    FROM progress
    ON CONFLICT (attempt_ref) DO NOTHING
    RETURNING run_ref, attempt_ref
  )
  INSERT INTO sarah_fleet_attempt_repair_refs (run_ref, attempt_ref)
  SELECT run_ref, attempt_ref FROM inserted
  ON CONFLICT DO NOTHING;

  SELECT count(*) INTO repaired_count
  FROM sarah_fleet_attempt_repair_refs;

  WITH latest AS (
    SELECT DISTINCT ON (attempt.run_ref, attempt.work_unit_ref)
      attempt.run_ref,
      attempt.work_unit_ref,
      attempt.attempt_ref,
      attempt.state,
      attempt.updated_at
    FROM sarah_fleet_run_attempts AS attempt
    JOIN sarah_fleet_run_execution_events AS event
      ON event.event_ref = attempt.last_event_ref
    ORDER BY attempt.run_ref, attempt.work_unit_ref, event.sequence DESC
  )
  UPDATE sarah_fleet_run_work_units AS unit
  SET state = CASE latest.state
        WHEN 'evidence_pending' THEN 'verification_pending'
        ELSE latest.state
      END,
      latest_attempt_ref = latest.attempt_ref,
      accepted_attempt_ref = CASE
        WHEN latest.state = 'succeeded' THEN latest.attempt_ref
        ELSE NULL
      END,
      updated_at = latest.updated_at
  FROM latest
  WHERE unit.run_ref = latest.run_ref
    AND unit.unit_ref = latest.work_unit_ref;

  INSERT INTO sarah_fleet_attempt_repair_runs (run_ref)
  SELECT DISTINCT request.run_ref
  FROM sarah_fleet_run_requests AS request
  JOIN sarah_fleet_attempt_repair_refs AS repaired
    ON repaired.run_ref = request.run_ref
  ON CONFLICT DO NOTHING;

  UPDATE sarah_fleet_run_requests AS request
  SET execution_started_at = COALESCE(
        (
          SELECT min(event.recorded_at)
          FROM sarah_fleet_run_execution_events AS event
          WHERE event.run_ref = request.run_ref
            AND event.event_kind = 'run_started'
        ),
        request.execution_started_at
      ),
      execution_updated_at = COALESCE(
        (
          SELECT max(event.recorded_at)
          FROM sarah_fleet_run_execution_events AS event
          WHERE event.run_ref = request.run_ref
        ),
        request.execution_updated_at
      )
  WHERE request.run_ref IN (SELECT DISTINCT run_ref FROM sarah_fleet_attempt_repair_refs);

  INSERT INTO sarah_fleet_attempt_repair_entities
    (scope, entity_type, entity_id, post_image_json)
  SELECT
    'scope.fleet_run.' || attempt.run_ref,
    'fleet_attempt',
    attempt.attempt_ref,
    jsonb_build_object(
      'attemptRef', attempt.attempt_ref,
      'workUnitRef', attempt.work_unit_ref,
      'intakeClaimRef', attempt.intake_claim_ref,
      'pylonRef', attempt.pylon_ref,
      'workerKind', attempt.worker_kind,
      'state', attempt.state,
      'progressClass', attempt.progress_class,
      'assignmentRef', attempt.assignment_ref,
      'accountRefHash', attempt.account_ref_hash,
      'capacityClass', attempt.capacity_class,
      'marginalCostClass', attempt.marginal_cost_class,
      'verification', attempt.verification_json::jsonb,
      'artifactRefs', attempt.artifact_refs_json::jsonb,
      'proofRefs', attempt.proof_refs_json::jsonb,
      'authorityReceiptRefs', attempt.authority_receipt_refs_json::jsonb,
      'closeoutRef', attempt.closeout_ref,
      'usageEvidence', attempt.usage_json::jsonb,
      'blockerRefs', attempt.blocker_refs_json::jsonb,
      'lastEventRef', attempt.last_event_ref,
      'startedAt', attempt.started_at,
      'lastObservedAt', attempt.last_observed_at,
      'remoteObservedAt', attempt.remote_observed_at,
      'terminalAt', attempt.terminal_at,
      'updatedAt', attempt.updated_at
    )
  FROM sarah_fleet_run_attempts AS attempt
  JOIN sarah_fleet_attempt_repair_refs AS repaired
    ON repaired.run_ref = attempt.run_ref
   AND repaired.attempt_ref = attempt.attempt_ref;

  INSERT INTO sarah_fleet_attempt_repair_entities
    (scope, entity_type, entity_id, post_image_json)
  SELECT
    'scope.fleet_run.' || unit.run_ref,
    'fleet_work_unit',
    unit.unit_ref,
    jsonb_build_object(
      'workUnitRef', unit.unit_ref,
      'issueRef', unit.issue_ref,
      'dependsOnRefs', unit.depends_on_refs_json::jsonb,
      'state', unit.state,
      'latestAttemptRef', unit.latest_attempt_ref,
      'acceptedAttemptRef', unit.accepted_attempt_ref,
      'updatedAt', unit.updated_at
    )
  FROM sarah_fleet_run_work_units AS unit
  WHERE unit.run_ref IN (
      SELECT run_ref FROM sarah_fleet_attempt_repair_refs
    )
    OR NOT EXISTS (
      SELECT 1 FROM khala_sync_changelog AS change
      WHERE change.scope = 'scope.fleet_run.' || unit.run_ref
        AND change.entity_type = 'fleet_work_unit'
        AND change.entity_id = unit.unit_ref
    )
  ON CONFLICT DO NOTHING;

  INSERT INTO sarah_fleet_attempt_repair_entities
    (scope, entity_type, entity_id, post_image_json)
  SELECT
    'scope.fleet_run.' || request.run_ref,
    'fleet_run',
    request.run_ref,
    jsonb_build_object(
      'runId', request.run_ref,
      'status', CASE request.execution_state
        WHEN 'pending' THEN 'draft'
        WHEN 'running' THEN 'running'
        WHEN 'completed' THEN 'completed'
        ELSE 'stopped'
      END,
      'desiredSlots', request.target_concurrency,
      'workerKind', request.worker_kind,
      'startedAt', request.execution_started_at,
      'counters', jsonb_build_object(
        'workUnitsTotal', (
          SELECT count(*) FROM sarah_fleet_run_work_units AS unit
          WHERE unit.run_ref = request.run_ref
        ),
        'activeAssignments', (
          SELECT count(*) FROM sarah_fleet_run_attempts AS attempt
          WHERE attempt.run_ref = request.run_ref AND attempt.state = 'running'
        ),
        'completedAssignments', (
          SELECT count(*) FROM sarah_fleet_run_work_units AS unit
          WHERE unit.run_ref = request.run_ref AND unit.state = 'succeeded'
        ),
        'failedAssignments', (
          SELECT count(*) FROM sarah_fleet_run_attempts AS attempt
          WHERE attempt.run_ref = request.run_ref AND attempt.state = 'failed'
        ),
        'blockedAssignments', (
          SELECT count(*) FROM sarah_fleet_run_attempts AS attempt
          WHERE attempt.run_ref = request.run_ref AND attempt.state = 'stale'
        )
      ),
      'updatedAt', request.execution_updated_at
    )
  FROM sarah_fleet_run_requests AS request
  JOIN sarah_fleet_attempt_repair_runs AS repaired
    ON repaired.run_ref = request.run_ref
  ON CONFLICT DO NOTHING;

  WITH repaired_scopes AS (
    SELECT DISTINCT scope FROM sarah_fleet_attempt_repair_entities
  ), bumped AS (
    UPDATE khala_sync_scopes AS scope_row
    SET last_version = scope_row.last_version + 1,
        updated_at = now()
    FROM repaired_scopes
    WHERE scope_row.scope = repaired_scopes.scope
    RETURNING scope_row.scope, scope_row.last_version
  )
  INSERT INTO khala_sync_changelog
    (scope, version, entity_type, entity_id, op, post_image_json, mutation_ref)
  SELECT
    entity.scope,
    bumped.last_version,
    entity.entity_type,
    entity.entity_id,
    'upsert',
    entity.post_image_json,
    'system:sarah_fleet_run_attempt_backfill.v1'
  FROM sarah_fleet_attempt_repair_entities AS entity
  JOIN bumped ON bumped.scope = entity.scope;

  RETURN repaired_count;
END;
$$ LANGUAGE plpgsql;

SELECT sarah_backfill_fleet_run_attempts_v1();
