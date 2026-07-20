-- IDE-13 #9041: durable refs-only owner-local capability operation exchange.
--
-- A registered Pylon can poll and claim an operation only for its current
-- owner-private target binding. This table stores references and public-safe
-- metadata. It has no material, encoded material, endpoint, or bearer column.

CREATE TABLE IF NOT EXISTS khala_sync_portable_owner_local_capability_operations (
  operation_ref text PRIMARY KEY,
  request_fingerprint text NOT NULL
    CHECK (request_fingerprint ~ '^sha256:[0-9a-f]{64}$'),
  action text NOT NULL CHECK (action IN ('install', 'wipe')),
  command_execution_claim_ref text NOT NULL
    REFERENCES khala_sync_portable_command_executions(claim_ref) ON DELETE CASCADE,
  owner_user_id text NOT NULL,
  pylon_ref text NOT NULL,
  session_ref text NOT NULL
    REFERENCES khala_sync_portable_sessions(session_ref) ON DELETE CASCADE,
  attachment_ref text NOT NULL,
  attachment_generation bigint NOT NULL CHECK (attachment_generation > 0),
  target_ref text NOT NULL REFERENCES khala_sync_portable_targets(target_ref),
  source_lease_ref text NOT NULL,
  source_grant_ref text NOT NULL,
  destination_lease_ref text NOT NULL,
  destination_grant_ref text NOT NULL,
  permission_refs_json jsonb NOT NULL,
  permission_fingerprint text NOT NULL
    CHECK (permission_fingerprint ~ '^sha256:[0-9a-f]{64}$'),
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
  receipt_ref text,
  result_evidence_refs_json jsonb NOT NULL DEFAULT '[]'::jsonb,
  error_ref text,
  completed_at timestamptz,
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  UNIQUE (
    command_execution_claim_ref,
    action,
    source_lease_ref,
    destination_lease_ref,
    target_ref
  ),
  CHECK (source_lease_ref <> destination_lease_ref),
  CHECK (source_grant_ref <> destination_grant_ref),
  CHECK (jsonb_typeof(permission_refs_json) = 'array'),
  CHECK (
    (state = 'pending'
      AND claim_ref IS NULL AND claim_fingerprint IS NULL
      AND worker_instance_ref IS NULL AND claim_generation IS NULL
      AND lease_revision IS NULL AND claimed_at IS NULL AND lease_expires_at IS NULL
      AND result_ref IS NULL AND result_fingerprint IS NULL AND result_status IS NULL
      AND receipt_ref IS NULL AND error_ref IS NULL AND completed_at IS NULL)
    OR
    (state = 'claimed'
      AND claim_ref IS NOT NULL AND claim_fingerprint IS NOT NULL
      AND worker_instance_ref IS NOT NULL AND claim_generation IS NOT NULL
      AND lease_revision IS NOT NULL AND claimed_at IS NOT NULL AND lease_expires_at IS NOT NULL
      AND lease_expires_at > claimed_at
      AND result_ref IS NULL AND result_fingerprint IS NULL AND result_status IS NULL
      AND receipt_ref IS NULL AND error_ref IS NULL AND completed_at IS NULL)
    OR
    (state IN ('completed', 'failed')
      AND claim_ref IS NOT NULL AND claim_fingerprint IS NOT NULL
      AND worker_instance_ref IS NOT NULL AND claim_generation IS NOT NULL
      AND lease_revision IS NOT NULL AND claimed_at IS NOT NULL AND lease_expires_at IS NOT NULL
      AND result_ref IS NOT NULL AND result_fingerprint IS NOT NULL
      AND result_status = state AND completed_at IS NOT NULL)
    OR
    (state = 'expired'
      AND result_ref IS NOT NULL AND result_fingerprint IS NULL
      AND result_status = 'expired' AND receipt_ref IS NULL
      AND error_ref IS NULL AND completed_at IS NOT NULL)
  ),
  CHECK (
    (state = 'completed' AND receipt_ref IS NOT NULL AND error_ref IS NULL) OR
    (state = 'failed' AND receipt_ref IS NULL AND error_ref IS NOT NULL) OR
    (state NOT IN ('completed', 'failed'))
  )
);

CREATE INDEX IF NOT EXISTS khala_sync_portable_owner_local_capability_pending
  ON khala_sync_portable_owner_local_capability_operations
    (pylon_ref, target_ref, created_at, operation_ref)
  WHERE state = 'pending';

CREATE INDEX IF NOT EXISTS khala_sync_portable_owner_local_capability_claim
  ON khala_sync_portable_owner_local_capability_operations(claim_ref)
  WHERE claim_ref IS NOT NULL;
