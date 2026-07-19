-- SBX-04: exact managed-sandbox runtime turn identity and ordered event replay.
--
-- The lifecycle event log remains the native sequence authority. Runtime event
-- coordinates add an exact per-turn replay fence without a second event log.

ALTER TABLE khala_sync_managed_sandbox_turns
  DROP CONSTRAINT IF EXISTS khala_sync_managed_sandbox_turns_status_check;

ALTER TABLE khala_sync_managed_sandbox_turns
  ADD CONSTRAINT khala_sync_managed_sandbox_turns_status_check
  CHECK (status IN (
    'pending', 'running', 'interrupting', 'settled', 'failed', 'interrupted'
  ));

ALTER TABLE khala_sync_managed_sandbox_turns
  ADD COLUMN IF NOT EXISTS provider text NOT NULL DEFAULT 'codex'
    CHECK (provider IN ('codex', 'claude')),
  ADD COLUMN IF NOT EXISTS model_ref text NOT NULL DEFAULT 'model.unknown',
  ADD COLUMN IF NOT EXISTS harness_ref text NOT NULL DEFAULT 'harness.unknown',
  ADD COLUMN IF NOT EXISTS reasoning_effort text,
  ADD COLUMN IF NOT EXISTS prompt_digest text NOT NULL
    DEFAULT 'sha256:0000000000000000000000000000000000000000000000000000000000000000'
    CHECK (prompt_digest ~ '^sha256:[a-f0-9]{64}$'),
  ADD COLUMN IF NOT EXISTS last_event_sequence bigint NOT NULL DEFAULT 0
    CHECK (last_event_sequence >= 0),
  ADD COLUMN IF NOT EXISTS turn_json jsonb,
  ADD COLUMN IF NOT EXISTS turn_receipt_json jsonb,
  ADD COLUMN IF NOT EXISTS interrupt_command_ref text;

UPDATE khala_sync_managed_sandbox_turns t
SET turn_json = jsonb_build_object(
  'schema', 'openagents.managed_sandbox_turn.v1',
  'turnRef', t.turn_ref,
  'sandboxRef', t.sandbox_ref,
  'ownerRef', s.owner_user_id,
  'tenantRef', s.tenant_ref,
  'workUnitRef', s.work_unit_ref,
  'attachmentRef', s.attachment_ref,
  'attachmentGeneration', s.attachment_generation,
  'resourceGeneration', t.resource_generation,
  'turnSequence', t.turn_sequence,
  'lastEventSequence', t.last_event_sequence,
  'commandRef', t.command_ref,
  'capabilityRef', COALESCE(c.command_json ->> 'capabilityRef', 'capability.unknown'),
  'promptDigest', t.prompt_digest,
  'runtime', jsonb_build_object(
    'provider', t.provider,
    'modelRef', t.model_ref,
    'harnessRef', t.harness_ref
  ),
  'status', t.status,
  'createdAt', to_char(t.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
)
FROM khala_sync_managed_sandboxes s,
     khala_sync_managed_sandbox_commands c
WHERE t.turn_json IS NULL
  AND s.sandbox_ref = t.sandbox_ref
  AND c.command_ref = t.command_ref;

ALTER TABLE khala_sync_managed_sandbox_turns
  ALTER COLUMN turn_json SET NOT NULL,
  ADD CONSTRAINT khala_sync_managed_sandbox_turn_json_object
    CHECK (jsonb_typeof(turn_json) = 'object'),
  ADD CONSTRAINT khala_sync_managed_sandbox_turn_receipt_json_object
    CHECK (turn_receipt_json IS NULL OR jsonb_typeof(turn_receipt_json) = 'object');

ALTER TABLE khala_sync_managed_sandbox_events
  ADD COLUMN IF NOT EXISTS turn_ref text,
  ADD COLUMN IF NOT EXISTS turn_event_sequence bigint
    CHECK (turn_event_sequence IS NULL OR turn_event_sequence > 0),
  ADD CONSTRAINT khala_sync_managed_sandbox_event_turn_coordinates
    CHECK ((turn_ref IS NULL) = (turn_event_sequence IS NULL));

CREATE UNIQUE INDEX IF NOT EXISTS khala_sync_managed_sandbox_turn_event_order
  ON khala_sync_managed_sandbox_events(sandbox_ref, turn_ref, turn_event_sequence)
  WHERE turn_ref IS NOT NULL;

CREATE INDEX IF NOT EXISTS khala_sync_managed_sandbox_active_turns
  ON khala_sync_managed_sandbox_turns(sandbox_ref, status, turn_sequence)
  WHERE status IN ('pending', 'running', 'interrupting');
