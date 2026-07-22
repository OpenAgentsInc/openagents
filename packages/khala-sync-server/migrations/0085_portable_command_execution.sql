-- IDE-13 #9041: one durable execution claim for each accepted portable command.
--
-- The command row remains product authority. This table only serializes the
-- consumer that can invoke the canonical portable move runtime. An expired
-- claim cannot be replaced automatically. A separately fenced recovery
-- transition is required before a different executor can continue.

CREATE TABLE IF NOT EXISTS khala_sync_portable_command_executions (
  command_ref text PRIMARY KEY
    REFERENCES khala_sync_portable_commands(command_ref) ON DELETE CASCADE,
  claim_ref text NOT NULL UNIQUE,
  owner_user_id text NOT NULL,
  session_ref text NOT NULL
    REFERENCES khala_sync_portable_sessions(session_ref) ON DELETE CASCADE,
  command_kind text NOT NULL
    CHECK (command_kind IN ('attach', 'move', 'failback')),
  command_fingerprint text NOT NULL
    CHECK (command_fingerprint ~ '^sha256:[0-9a-f]{64}$'),
  claim_fingerprint text NOT NULL
    CHECK (claim_fingerprint ~ '^sha256:[0-9a-f]{64}$'),
  source_attachment_ref text NOT NULL
    REFERENCES khala_sync_portable_attachments(attachment_ref),
  source_generation bigint NOT NULL CHECK (source_generation > 0),
  destination_target_ref text NOT NULL
    REFERENCES khala_sync_portable_targets(target_ref),
  executor_environment_ref text NOT NULL
    REFERENCES khala_sync_portable_targets(target_ref),
  worker_instance_ref text NOT NULL,
  claim_generation bigint NOT NULL DEFAULT 1 CHECK (claim_generation > 0),
  lease_revision bigint NOT NULL DEFAULT 1 CHECK (lease_revision > 0),
  state text NOT NULL
    CHECK (state IN ('claimed', 'pending_reconcile', 'terminal', 'expired')),
  claimed_at timestamptz NOT NULL,
  lease_expires_at timestamptz NOT NULL,
  terminal_status text
    CHECK (terminal_status IN ('completed', 'failed', 'rejected', 'expired')),
  pending_reconcile_ref text,
  outcome_ref text,
  evidence_refs_json jsonb NOT NULL DEFAULT '[]'::jsonb,
  updated_at timestamptz NOT NULL,
  CHECK (lease_expires_at > claimed_at),
  CHECK (
    (state = 'claimed' AND terminal_status IS NULL AND pending_reconcile_ref IS NULL AND outcome_ref IS NULL)
    OR
    (state = 'pending_reconcile' AND terminal_status IS NULL AND pending_reconcile_ref IS NOT NULL AND outcome_ref IS NULL)
    OR
    (state IN ('terminal', 'expired') AND terminal_status IS NOT NULL AND pending_reconcile_ref IS NULL AND outcome_ref IS NOT NULL)
  )
);

CREATE INDEX IF NOT EXISTS khala_sync_portable_command_executions_active
  ON khala_sync_portable_command_executions(session_ref, lease_expires_at)
  WHERE state IN ('claimed', 'pending_reconcile');
