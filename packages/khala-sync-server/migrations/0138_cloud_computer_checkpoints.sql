-- Durable workspace heads, resumable checkpoint operations, encrypted GCS
-- object bindings, reachability, metering, and deletion evidence. Runtime files
-- remain in local copy-on-write storage.

-- Reservation rows are immutable generation evidence. The original composite
-- foreign key prevented the logical computer row from advancing generations
-- once any reservation existed; the existing computer_ref foreign key retains
-- referential integrity without rewriting historical reservation generations.
DO $$
DECLARE
  generation_constraint text;
BEGIN
  SELECT constraint_row.conname
  INTO generation_constraint
  FROM pg_constraint AS constraint_row
  JOIN pg_class AS relation ON relation.oid = constraint_row.conrelid
  WHERE relation.relname = 'khala_sync_cloud_capacity_reservations'
    AND constraint_row.contype = 'f'
    AND pg_get_constraintdef(constraint_row.oid)
      LIKE 'FOREIGN KEY (computer_ref, generation, owner_ref, tenant_ref, conversation_ref, runtime_class)%';

  IF generation_constraint IS NOT NULL THEN
    EXECUTE format(
      'ALTER TABLE khala_sync_cloud_capacity_reservations DROP CONSTRAINT %I',
      generation_constraint
    );
  END IF;
END
$$;

CREATE OR REPLACE FUNCTION khala_sync_cloud_capacity_reservation_scope_guard()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM khala_sync_cloud_computers AS computer
    WHERE computer.computer_ref = NEW.computer_ref
      AND computer.generation = NEW.generation
      AND computer.owner_ref = NEW.owner_ref
      AND computer.tenant_ref = NEW.tenant_ref
      AND computer.conversation_ref = NEW.conversation_ref
      AND computer.runtime_class = NEW.runtime_class
  ) THEN
    RAISE EXCEPTION 'cloud capacity reservation scope differs from computer generation'
      USING ERRCODE = '23503';
  END IF;
  RETURN NEW;
END
$$;

DROP TRIGGER IF EXISTS khala_sync_cloud_capacity_reservation_scope_guard_trigger
  ON khala_sync_cloud_capacity_reservations;

CREATE TRIGGER khala_sync_cloud_capacity_reservation_scope_guard_trigger
BEFORE INSERT OR UPDATE OF computer_ref, generation, owner_ref, tenant_ref,
  conversation_ref, runtime_class
ON khala_sync_cloud_capacity_reservations
FOR EACH ROW
EXECUTE FUNCTION khala_sync_cloud_capacity_reservation_scope_guard();

CREATE TABLE IF NOT EXISTS khala_sync_cloud_computer_workspaces (
  workspace_ref text PRIMARY KEY,
  computer_ref text NOT NULL UNIQUE REFERENCES khala_sync_cloud_computers (computer_ref),
  runtime_generation bigint NOT NULL CHECK (runtime_generation > 0),
  owner_ref text NOT NULL,
  tenant_ref text NOT NULL,
  conversation_ref text NOT NULL,
  workspace_revision bigint NOT NULL DEFAULT 0 CHECK (workspace_revision >= 0),
  current_checkpoint_ref text,
  base_image_digest text NOT NULL CHECK (base_image_digest ~ '^sha256:[0-9a-f]{64}$'),
  base_image_signature_ref text NOT NULL,
  workspace_key_ref text NOT NULL,
  workspace_key_version bigint NOT NULL CHECK (workspace_key_version > 0),
  state text NOT NULL DEFAULT 'active' CHECK (state IN ('active', 'destroying', 'destroyed')),
  version bigint NOT NULL DEFAULT 1 CHECK (version > 0),
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  destroyed_at timestamptz,
  UNIQUE (workspace_ref, owner_ref, tenant_ref),
  CHECK ((state = 'destroyed') = (destroyed_at IS NOT NULL)),
  CHECK (updated_at >= created_at)
);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_checkpoint_operations (
  operation_ref text PRIMARY KEY,
  idempotency_ref text NOT NULL,
  request_digest text NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),
  workspace_ref text NOT NULL REFERENCES khala_sync_cloud_computer_workspaces (workspace_ref),
  owner_ref text NOT NULL,
  tenant_ref text NOT NULL,
  computer_ref text NOT NULL,
  expected_runtime_generation bigint NOT NULL CHECK (expected_runtime_generation > 0),
  expected_workspace_revision bigint NOT NULL CHECK (expected_workspace_revision >= 0),
  expected_parent_checkpoint_ref text,
  boundary text NOT NULL CHECK (boundary IN ('explicit', 'interval', 'stop', 'host_replacement', 'fork')),
  status text NOT NULL CHECK (status IN (
    'prepared', 'uploading', 'upload_uncertain', 'uploaded', 'verifying',
    'verification_failed', 'commit_ready', 'committed', 'stale_generation',
    'refused', 'orphaned'
  )),
  upload_session_ref text,
  uploaded_byte_count bigint NOT NULL DEFAULT 0 CHECK (uploaded_byte_count >= 0),
  object_ref text,
  object_generation bigint CHECK (object_generation IS NULL OR object_generation > 0),
  content_manifest_digest text CHECK (
    content_manifest_digest IS NULL OR content_manifest_digest ~ '^sha256:[0-9a-f]{64}$'
  ),
  storage_manifest_digest text CHECK (
    storage_manifest_digest IS NULL OR storage_manifest_digest ~ '^sha256:[0-9a-f]{64}$'
  ),
  checkpoint_ref text,
  result_digest text CHECK (result_digest IS NULL OR result_digest ~ '^sha256:[0-9a-f]{64}$'),
  receipt_json jsonb,
  failure_code text,
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  completed_at timestamptz,
  revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
  UNIQUE (workspace_ref, idempotency_ref),
  FOREIGN KEY (workspace_ref, owner_ref, tenant_ref)
    REFERENCES khala_sync_cloud_computer_workspaces (workspace_ref, owner_ref, tenant_ref),
  CHECK ((object_ref IS NULL) = (object_generation IS NULL)),
  CHECK ((status = 'committed') = (checkpoint_ref IS NOT NULL)),
  CHECK ((status = 'committed') = (receipt_json IS NOT NULL)),
  CHECK ((status = 'committed') = (completed_at IS NOT NULL)),
  CHECK (updated_at >= created_at)
);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_checkpoint_objects (
  object_ref text PRIMARY KEY,
  workspace_ref text NOT NULL REFERENCES khala_sync_cloud_computer_workspaces (workspace_ref),
  operation_ref text NOT NULL UNIQUE REFERENCES khala_sync_cloud_checkpoint_operations (operation_ref),
  owner_ref text NOT NULL,
  tenant_ref text NOT NULL,
  object_uri text NOT NULL,
  object_generation bigint NOT NULL CHECK (object_generation > 0),
  content_digest text NOT NULL CHECK (content_digest ~ '^sha256:[0-9a-f]{64}$'),
  content_manifest_digest text NOT NULL CHECK (content_manifest_digest ~ '^sha256:[0-9a-f]{64}$'),
  storage_manifest_digest text NOT NULL CHECK (storage_manifest_digest ~ '^sha256:[0-9a-f]{64}$'),
  ciphertext_digest text NOT NULL CHECK (ciphertext_digest ~ '^sha256:[0-9a-f]{64}$'),
  crc32c text NOT NULL,
  workspace_key_ref text NOT NULL,
  workspace_key_version bigint NOT NULL CHECK (workspace_key_version > 0),
  wrapped_dek_ref text NOT NULL,
  encrypted_byte_count bigint NOT NULL CHECK (encrypted_byte_count > 0),
  state text NOT NULL CHECK (state IN ('uploaded', 'verified', 'reachable', 'tombstoned', 'deleted')),
  created_at timestamptz NOT NULL,
  verified_at timestamptz,
  retain_until timestamptz NOT NULL,
  tombstoned_at timestamptz,
  deleted_at timestamptz,
  revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
  UNIQUE (workspace_ref, content_manifest_digest, storage_manifest_digest),
  UNIQUE (object_uri, object_generation),
  FOREIGN KEY (workspace_ref, owner_ref, tenant_ref)
    REFERENCES khala_sync_cloud_computer_workspaces (workspace_ref, owner_ref, tenant_ref),
  CHECK ((state IN ('verified', 'reachable', 'tombstoned', 'deleted')) = (verified_at IS NOT NULL)),
  CHECK ((state IN ('tombstoned', 'deleted')) = (tombstoned_at IS NOT NULL)),
  CHECK ((state = 'deleted') = (deleted_at IS NOT NULL)),
  CHECK (retain_until >= created_at)
);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_computer_checkpoints (
  checkpoint_ref text PRIMARY KEY,
  operation_ref text NOT NULL UNIQUE REFERENCES khala_sync_cloud_checkpoint_operations (operation_ref),
  workspace_ref text NOT NULL REFERENCES khala_sync_cloud_computer_workspaces (workspace_ref),
  owner_ref text NOT NULL,
  tenant_ref text NOT NULL,
  computer_ref text NOT NULL,
  source_runtime_generation bigint NOT NULL CHECK (source_runtime_generation > 0),
  workspace_revision bigint NOT NULL CHECK (workspace_revision > 0),
  parent_checkpoint_ref text REFERENCES khala_sync_cloud_computer_checkpoints (checkpoint_ref),
  object_ref text NOT NULL REFERENCES khala_sync_cloud_checkpoint_objects (object_ref),
  content_digest text NOT NULL CHECK (content_digest ~ '^sha256:[0-9a-f]{64}$'),
  workspace_state_digest text NOT NULL CHECK (workspace_state_digest ~ '^sha256:[0-9a-f]{64}$'),
  content_manifest_digest text NOT NULL CHECK (content_manifest_digest ~ '^sha256:[0-9a-f]{64}$'),
  storage_manifest_digest text NOT NULL CHECK (storage_manifest_digest ~ '^sha256:[0-9a-f]{64}$'),
  base_image_digest text NOT NULL CHECK (base_image_digest ~ '^sha256:[0-9a-f]{64}$'),
  checkpoint_kind text NOT NULL CHECK (checkpoint_kind IN ('full', 'delta')),
  deleted_paths_json jsonb NOT NULL CHECK (jsonb_typeof(deleted_paths_json) = 'array'),
  plaintext_byte_count bigint NOT NULL CHECK (plaintext_byte_count >= 0),
  encrypted_byte_count bigint NOT NULL CHECK (encrypted_byte_count > 0),
  retention_policy_ref text NOT NULL,
  status text NOT NULL CHECK (status IN ('committed', 'superseded', 'tombstoned', 'deleted')),
  verified_at timestamptz NOT NULL,
  committed_at timestamptz NOT NULL,
  retain_until timestamptz NOT NULL,
  tombstoned_at timestamptz,
  deleted_at timestamptz,
  content_manifest_json jsonb NOT NULL CHECK (jsonb_typeof(content_manifest_json) = 'object'),
  storage_manifest_json jsonb NOT NULL CHECK (jsonb_typeof(storage_manifest_json) = 'object'),
  revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
  UNIQUE (workspace_ref, workspace_revision),
  FOREIGN KEY (workspace_ref, owner_ref, tenant_ref)
    REFERENCES khala_sync_cloud_computer_workspaces (workspace_ref, owner_ref, tenant_ref),
  CHECK ((checkpoint_kind = 'full' AND parent_checkpoint_ref IS NULL)
      OR (checkpoint_kind = 'delta' AND parent_checkpoint_ref IS NOT NULL)),
  CHECK ((status IN ('tombstoned', 'deleted')) = (tombstoned_at IS NOT NULL)),
  CHECK ((status = 'deleted') = (deleted_at IS NOT NULL)),
  CHECK (retain_until >= committed_at AND committed_at >= verified_at)
);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_checkpoint_references (
  reference_ref text PRIMARY KEY,
  workspace_ref text NOT NULL REFERENCES khala_sync_cloud_computer_workspaces (workspace_ref),
  checkpoint_ref text NOT NULL REFERENCES khala_sync_cloud_computer_checkpoints (checkpoint_ref),
  source_checkpoint_ref text REFERENCES khala_sync_cloud_computer_checkpoints (checkpoint_ref),
  kind text NOT NULL CHECK (kind IN ('current_head', 'parent', 'fork_source', 'rollback', 'legal_hold')),
  state text NOT NULL CHECK (state IN ('live', 'released')),
  created_at timestamptz NOT NULL,
  released_at timestamptz,
  CHECK ((state = 'released') = (released_at IS NOT NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS khala_sync_cloud_checkpoint_one_current_head
  ON khala_sync_cloud_checkpoint_references (workspace_ref)
  WHERE kind = 'current_head' AND state = 'live';

ALTER TABLE khala_sync_cloud_computer_workspaces
  ADD CONSTRAINT khala_sync_cloud_computer_workspace_current_checkpoint
  FOREIGN KEY (current_checkpoint_ref)
  REFERENCES khala_sync_cloud_computer_checkpoints (checkpoint_ref)
  DEFERRABLE INITIALLY DEFERRED;

CREATE TABLE IF NOT EXISTS khala_sync_cloud_computer_workspace_forks (
  fork_ref text PRIMARY KEY,
  source_workspace_ref text NOT NULL REFERENCES khala_sync_cloud_computer_workspaces (workspace_ref),
  source_checkpoint_ref text NOT NULL REFERENCES khala_sync_cloud_computer_checkpoints (checkpoint_ref),
  target_workspace_ref text NOT NULL UNIQUE REFERENCES khala_sync_cloud_computer_workspaces (workspace_ref),
  owner_ref text NOT NULL,
  tenant_ref text NOT NULL,
  actor_ref text NOT NULL,
  authorization_digest text NOT NULL CHECK (authorization_digest ~ '^sha256:[0-9a-f]{64}$'),
  state text NOT NULL CHECK (state IN ('authorized', 'resealed', 'failed')),
  created_at timestamptz NOT NULL,
  resealed_checkpoint_ref text REFERENCES khala_sync_cloud_computer_checkpoints (checkpoint_ref),
  completed_at timestamptz,
  CHECK ((state = 'resealed') = (resealed_checkpoint_ref IS NOT NULL)),
  CHECK ((state = 'resealed') = (completed_at IS NOT NULL))
);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_checkpoint_deletion_evidence (
  deletion_ref text PRIMARY KEY,
  workspace_ref text NOT NULL REFERENCES khala_sync_cloud_computer_workspaces (workspace_ref),
  checkpoint_ref text NOT NULL DEFAULT '',
  object_ref text NOT NULL DEFAULT '',
  object_generation bigint,
  action text NOT NULL CHECK (action IN ('tombstoned', 'deleted', 'retained')),
  generation_precondition_met boolean NOT NULL,
  all_versions_absent boolean NOT NULL,
  key_disposition text NOT NULL CHECK (key_disposition IN ('retained', 'destroyed', 'not_applicable')),
  evidence_digest text NOT NULL CHECK (evidence_digest ~ '^sha256:[0-9a-f]{64}$'),
  observed_at timestamptz NOT NULL,
  CHECK (checkpoint_ref <> '' OR object_ref <> ''),
  CHECK ((object_ref = '') = (object_generation IS NULL)),
  UNIQUE (workspace_ref, checkpoint_ref, object_ref, action)
);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_checkpoint_usage_events (
  event_ref text PRIMARY KEY,
  workspace_ref text NOT NULL REFERENCES khala_sync_cloud_computer_workspaces (workspace_ref),
  checkpoint_ref text,
  operation_ref text,
  kind text NOT NULL CHECK (kind IN ('uploaded', 'reused', 'restored', 'retained', 'collected', 'storage_age_sample')),
  byte_count bigint NOT NULL CHECK (byte_count >= 0),
  duration_ms bigint NOT NULL CHECK (duration_ms >= 0),
  storage_age_ms bigint NOT NULL DEFAULT 0 CHECK (storage_age_ms >= 0),
  object_count bigint NOT NULL DEFAULT 1 CHECK (object_count >= 0),
  observed_at timestamptz NOT NULL,
  UNIQUE (workspace_ref, event_ref)
);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_computer_host_loss_evidence (
  evidence_ref text PRIMARY KEY,
  computer_ref text NOT NULL REFERENCES khala_sync_cloud_computers (computer_ref),
  workspace_ref text NOT NULL REFERENCES khala_sync_cloud_computer_workspaces (workspace_ref),
  owner_ref text NOT NULL,
  tenant_ref text NOT NULL,
  runtime_generation bigint NOT NULL CHECK (runtime_generation > 0),
  provider_lease_ref text NOT NULL,
  evidence_digest text NOT NULL CHECK (evidence_digest ~ '^sha256:[0-9a-f]{64}$'),
  observed_at timestamptz NOT NULL,
  UNIQUE (computer_ref, runtime_generation, provider_lease_ref),
  FOREIGN KEY (workspace_ref, owner_ref, tenant_ref)
    REFERENCES khala_sync_cloud_computer_workspaces (workspace_ref, owner_ref, tenant_ref)
);

CREATE INDEX IF NOT EXISTS khala_sync_cloud_checkpoint_resumable_operations
  ON khala_sync_cloud_checkpoint_operations (status, updated_at, workspace_ref);

CREATE INDEX IF NOT EXISTS khala_sync_cloud_checkpoint_gc_candidates
  ON khala_sync_cloud_checkpoint_objects (state, retain_until, workspace_ref);

CREATE INDEX IF NOT EXISTS khala_sync_cloud_checkpoint_restore_scope
  ON khala_sync_cloud_computer_checkpoints
    (workspace_ref, owner_ref, tenant_ref, workspace_revision, status);
