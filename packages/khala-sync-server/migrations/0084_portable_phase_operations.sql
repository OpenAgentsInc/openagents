-- IDE-13 #9041: durable refs-only exchange for remote move phases.
--
-- The server owns movement authority. A bound Pylon can claim only the exact
-- operation row that names it and its target. This table contains no payload
-- bytes, local roots, credentials, processes, or native transport handles.

CREATE TABLE IF NOT EXISTS khala_sync_portable_phase_operations (
  operation_ref text PRIMARY KEY,
  request_fingerprint text NOT NULL
    CHECK (request_fingerprint ~ '^sha256:[0-9a-f]{64}$'),
  command_ref text NOT NULL
    REFERENCES khala_sync_portable_commands(command_ref) ON DELETE CASCADE,
  command_execution_claim_ref text NOT NULL
    REFERENCES khala_sync_portable_command_executions(claim_ref) ON DELETE CASCADE,
  owner_user_id text NOT NULL,
  session_ref text NOT NULL
    REFERENCES khala_sync_portable_sessions(session_ref) ON DELETE CASCADE,
  attachment_ref text NOT NULL,
  attachment_generation bigint NOT NULL CHECK (attachment_generation > 0),
  target_ref text NOT NULL REFERENCES khala_sync_portable_targets(target_ref),
  pylon_ref text NOT NULL,
  kind text NOT NULL CHECK (kind IN (
    'quiesce', 'checkpoint-create', 'source-cleanup', 'checkpoint-stage',
    'destination-activate', 'staged-abort'
  )),
  UNIQUE (command_execution_claim_ref, kind),
  checkpoint_ref text,
  checkpoint_object_ref text,
  checkpoint_digest text CHECK (
    checkpoint_digest IS NULL OR checkpoint_digest ~ '^sha256:[0-9a-f]{64}$'
  ),
  request_evidence_refs_json jsonb NOT NULL DEFAULT '[]'::jsonb,
  request_json jsonb NOT NULL,
  expires_at timestamptz NOT NULL,
  state text NOT NULL CHECK (state IN ('pending', 'claimed', 'completed', 'failed', 'expired')),
  claim_ref text UNIQUE,
  claim_fingerprint text CHECK (
    claim_fingerprint IS NULL OR claim_fingerprint ~ '^sha256:[0-9a-f]{64}$'
  ),
  worker_instance_ref text,
  claim_generation bigint CHECK (claim_generation IS NULL OR claim_generation > 0),
  lease_revision bigint CHECK (lease_revision IS NULL OR lease_revision > 0),
  claimed_at timestamptz,
  lease_expires_at timestamptz,
  result_ref text,
  result_fingerprint text CHECK (
    result_fingerprint IS NULL OR result_fingerprint ~ '^sha256:[0-9a-f]{64}$'
  ),
  result_status text CHECK (result_status IN ('completed', 'failed', 'expired')),
  result_checkpoint_ref text,
  result_checkpoint_object_ref text,
  result_checkpoint_digest text CHECK (
    result_checkpoint_digest IS NULL OR result_checkpoint_digest ~ '^sha256:[0-9a-f]{64}$'
  ),
  result_evidence_refs_json jsonb NOT NULL DEFAULT '[]'::jsonb,
  error_ref text,
  completed_at timestamptz,
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  CHECK (
    (kind = 'checkpoint-create'
      AND checkpoint_ref IS NOT NULL
      AND checkpoint_object_ref IS NULL
      AND checkpoint_digest IS NULL)
    OR
    (kind IN ('checkpoint-stage', 'destination-activate')
      AND checkpoint_ref IS NOT NULL
      AND checkpoint_object_ref IS NOT NULL
      AND checkpoint_digest IS NOT NULL)
    OR
    (kind NOT IN ('checkpoint-create', 'checkpoint-stage', 'destination-activate')
      AND checkpoint_ref IS NULL
      AND checkpoint_object_ref IS NULL
      AND checkpoint_digest IS NULL)
  ),
  CHECK (
    (state = 'pending'
      AND claim_ref IS NULL AND claim_fingerprint IS NULL
      AND worker_instance_ref IS NULL AND claim_generation IS NULL
      AND lease_revision IS NULL AND claimed_at IS NULL AND lease_expires_at IS NULL
      AND result_ref IS NULL AND result_fingerprint IS NULL
      AND result_status IS NULL AND error_ref IS NULL AND completed_at IS NULL)
    OR
    (state = 'claimed'
      AND claim_ref IS NOT NULL AND claim_fingerprint IS NOT NULL
      AND worker_instance_ref IS NOT NULL AND claim_generation IS NOT NULL
      AND lease_revision IS NOT NULL AND claimed_at IS NOT NULL AND lease_expires_at IS NOT NULL
      AND lease_expires_at > claimed_at
      AND result_ref IS NULL AND result_fingerprint IS NULL
      AND result_status IS NULL AND error_ref IS NULL AND completed_at IS NULL)
    OR
    (state IN ('completed', 'failed')
      AND claim_ref IS NOT NULL AND claim_fingerprint IS NOT NULL
      AND worker_instance_ref IS NOT NULL AND claim_generation IS NOT NULL
      AND lease_revision IS NOT NULL AND claimed_at IS NOT NULL AND lease_expires_at IS NOT NULL
      AND result_ref IS NOT NULL AND result_fingerprint IS NOT NULL
      AND result_status = state AND completed_at IS NOT NULL)
    OR
    (state = 'expired' AND result_ref IS NOT NULL AND result_fingerprint IS NULL
      AND result_status = 'expired' AND completed_at IS NOT NULL)
  ),
  CHECK (
    (state = 'completed' AND error_ref IS NULL) OR
    (state = 'failed' AND error_ref IS NOT NULL) OR
    (state NOT IN ('completed', 'failed'))
  ),
  CHECK (
    (state = 'completed' AND kind = 'checkpoint-create'
      AND result_checkpoint_ref IS NOT NULL
      AND result_checkpoint_object_ref IS NOT NULL
      AND result_checkpoint_digest IS NOT NULL)
    OR
    (NOT (state = 'completed' AND kind = 'checkpoint-create')
      AND result_checkpoint_ref IS NULL
      AND result_checkpoint_object_ref IS NULL
      AND result_checkpoint_digest IS NULL)
  )
);

CREATE INDEX IF NOT EXISTS khala_sync_portable_phase_operations_pending
  ON khala_sync_portable_phase_operations(pylon_ref, target_ref, created_at)
  WHERE state = 'pending';
