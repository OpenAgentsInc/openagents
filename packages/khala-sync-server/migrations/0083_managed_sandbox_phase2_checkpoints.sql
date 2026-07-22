-- SBX-10: durable content-checkpoint metadata and byte-idempotent Phase 2 replay.
--
-- Checkpoint content stays in the admitted object store. Cloud SQL stores only
-- bounded verified metadata, exact command/result bytes, and content digests.
-- A retained checkpoint is independent of source-sandbox deletion.

CREATE TABLE IF NOT EXISTS khala_sync_managed_sandbox_phase2_operations (
  command_ref text PRIMARY KEY,
  owner_user_id text NOT NULL,
  tenant_ref text NOT NULL,
  idempotency_ref text NOT NULL,
  command_kind text NOT NULL CHECK (command_kind IN (
    'CreateCheckpoint', 'ArchiveWithCheckpoint', 'ForkFromCheckpoint',
    'RestoreCheckpoint', 'DeleteCheckpoint'
  )),
  command_fingerprint text NOT NULL CHECK (command_fingerprint ~ '^sha256:[a-f0-9]{64}$'),
  result_fingerprint text NOT NULL CHECK (result_fingerprint ~ '^sha256:[a-f0-9]{64}$'),
  command_json jsonb NOT NULL CHECK (jsonb_typeof(command_json) = 'object'),
  result_json jsonb NOT NULL CHECK (jsonb_typeof(result_json) = 'object'),
  requested_at timestamptz NOT NULL,
  settled_at timestamptz NOT NULL,
  UNIQUE (owner_user_id, tenant_ref, idempotency_ref)
);

CREATE TABLE IF NOT EXISTS khala_sync_managed_sandbox_checkpoints (
  checkpoint_ref text PRIMARY KEY,
  owner_user_id text NOT NULL,
  tenant_ref text NOT NULL,
  source_sandbox_ref text NOT NULL,
  source_resource_generation bigint NOT NULL CHECK (source_resource_generation >= 0),
  content_digest text NOT NULL CHECK (content_digest ~ '^sha256:[a-f0-9]{64}$'),
  checkpoint_fingerprint text NOT NULL CHECK (checkpoint_fingerprint ~ '^sha256:[a-f0-9]{64}$'),
  created_by_command_ref text NOT NULL
    REFERENCES khala_sync_managed_sandbox_phase2_operations(command_ref),
  retained_until timestamptz NOT NULL,
  checkpoint_json jsonb NOT NULL CHECK (jsonb_typeof(checkpoint_json) = 'object'),
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  UNIQUE (owner_user_id, tenant_ref, checkpoint_ref)
);

CREATE INDEX IF NOT EXISTS khala_sync_managed_sandbox_checkpoint_retention
  ON khala_sync_managed_sandbox_checkpoints(retained_until);

CREATE INDEX IF NOT EXISTS khala_sync_managed_sandbox_checkpoint_source
  ON khala_sync_managed_sandbox_checkpoints(
    owner_user_id, tenant_ref, source_sandbox_ref, source_resource_generation
  );
