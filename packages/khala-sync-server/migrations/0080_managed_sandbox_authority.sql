-- SBX-01: canonical managed-sandbox lifecycle, event, and retry authority.
--
-- Native resources and events remain distinct from compatibility projections.
-- Provider effects are admitted only after an exact command fingerprint enters
-- this store. One partial unique index prevents two active commands, and a
-- second prevents two accepting generations for one sandbox.

CREATE TABLE IF NOT EXISTS khala_sync_managed_sandboxes (
  sandbox_ref text PRIMARY KEY,
  owner_user_id text NOT NULL,
  tenant_ref text NOT NULL,
  program_ref text NOT NULL DEFAULT 'program.managed_agent_sandboxes'
    CHECK (program_ref = 'program.managed_agent_sandboxes'),
  work_unit_ref text NOT NULL,
  attachment_ref text NOT NULL,
  attachment_generation bigint NOT NULL CHECK (attachment_generation >= 0),
  resource_generation bigint NOT NULL CHECK (resource_generation > 0),
  version bigint NOT NULL CHECK (version >= 0),
  last_event_sequence bigint NOT NULL CHECK (last_event_sequence >= 0),
  next_turn_sequence bigint NOT NULL DEFAULT 1 CHECK (next_turn_sequence > 0),
  target_ref text NOT NULL,
  lifecycle text NOT NULL CHECK (lifecycle IN (
    'provisioning', 'ready', 'idle', 'running', 'stopping', 'stopped',
    'resuming', 'deleting', 'deleted', 'failed', 'recovery_required'
  )),
  lease_state text NOT NULL CHECK (lease_state IN (
    'pending', 'active', 'expiring', 'expired', 'released'
  )),
  lease_expires_at timestamptz NOT NULL,
  guest_state text NOT NULL CHECK (guest_state IN (
    'absent', 'starting', 'present', 'stopping', 'unknown'
  )),
  filesystem_state text NOT NULL CHECK (filesystem_state IN (
    'unallocated', 'attached', 'checkpointing', 'durable', 'deleted', 'unknown'
  )),
  ingress_state text NOT NULL CHECK (ingress_state IN (
    'closed', 'broker_only', 'owner_tunnel', 'revoked', 'unknown'
  )),
  runtime_state text NOT NULL CHECK (runtime_state IN (
    'none', 'starting', 'running', 'interrupting', 'settled', 'failed', 'unknown'
  )),
  accepting_work boolean NOT NULL,
  cleanup_complete boolean NOT NULL,
  active_command_ref text,
  resource_json jsonb NOT NULL CHECK (jsonb_typeof(resource_json) = 'object'),
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  UNIQUE (owner_user_id, tenant_ref, sandbox_ref),
  CHECK (NOT (cleanup_complete AND lifecycle <> 'deleted')),
  CHECK (NOT (accepting_work AND lifecycle NOT IN ('ready', 'idle', 'running')))
);

CREATE TABLE IF NOT EXISTS khala_sync_managed_sandbox_generations (
  sandbox_ref text NOT NULL REFERENCES khala_sync_managed_sandboxes(sandbox_ref) ON DELETE CASCADE,
  resource_generation bigint NOT NULL CHECK (resource_generation > 0),
  lifecycle text NOT NULL,
  accepting_work boolean NOT NULL DEFAULT FALSE,
  opened_at timestamptz NOT NULL,
  fenced_at timestamptz,
  PRIMARY KEY (sandbox_ref, resource_generation)
);

CREATE UNIQUE INDEX IF NOT EXISTS khala_sync_managed_sandbox_one_accepting_generation
  ON khala_sync_managed_sandbox_generations(sandbox_ref)
  WHERE accepting_work;

CREATE TABLE IF NOT EXISTS khala_sync_managed_sandbox_commands (
  command_ref text PRIMARY KEY,
  sandbox_ref text NOT NULL REFERENCES khala_sync_managed_sandboxes(sandbox_ref) ON DELETE CASCADE,
  owner_user_id text NOT NULL,
  tenant_ref text NOT NULL,
  idempotency_ref text NOT NULL,
  command_kind text NOT NULL CHECK (command_kind IN (
    'Create', 'Inspect', 'Update', 'Stop', 'Resume', 'Delete', 'Dispatch', 'Interrupt'
  )),
  command_fingerprint text NOT NULL CHECK (command_fingerprint ~ '^sha256:[a-f0-9]{64}$'),
  settlement_fingerprint text CHECK (settlement_fingerprint ~ '^sha256:[a-f0-9]{64}$'),
  command_json jsonb NOT NULL CHECK (jsonb_typeof(command_json) = 'object'),
  resource_generation bigint NOT NULL CHECK (resource_generation > 0),
  claimed_version bigint NOT NULL CHECK (claimed_version >= 0),
  status text NOT NULL CHECK (status IN ('pending', 'settled', 'recovery_required', 'refused')),
  receipt_ref text,
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  UNIQUE (owner_user_id, tenant_ref, idempotency_ref)
);

CREATE UNIQUE INDEX IF NOT EXISTS khala_sync_managed_sandbox_one_pending_command
  ON khala_sync_managed_sandbox_commands(sandbox_ref)
  WHERE status = 'pending';

CREATE TABLE IF NOT EXISTS khala_sync_managed_sandbox_events (
  sandbox_ref text NOT NULL REFERENCES khala_sync_managed_sandboxes(sandbox_ref) ON DELETE CASCADE,
  sequence bigint NOT NULL CHECK (sequence > 0),
  event_ref text NOT NULL UNIQUE,
  command_ref text NOT NULL REFERENCES khala_sync_managed_sandbox_commands(command_ref),
  resource_generation bigint NOT NULL CHECK (resource_generation > 0),
  event_kind text NOT NULL,
  event_json jsonb NOT NULL CHECK (jsonb_typeof(event_json) = 'object'),
  observed_at timestamptz NOT NULL,
  PRIMARY KEY (sandbox_ref, sequence),
  FOREIGN KEY (sandbox_ref, resource_generation)
    REFERENCES khala_sync_managed_sandbox_generations(sandbox_ref, resource_generation)
);

CREATE TABLE IF NOT EXISTS khala_sync_managed_sandbox_receipts (
  receipt_ref text PRIMARY KEY,
  command_ref text NOT NULL UNIQUE REFERENCES khala_sync_managed_sandbox_commands(command_ref),
  sandbox_ref text NOT NULL REFERENCES khala_sync_managed_sandboxes(sandbox_ref) ON DELETE CASCADE,
  owner_user_id text NOT NULL,
  tenant_ref text NOT NULL,
  resource_generation bigint NOT NULL CHECK (resource_generation > 0),
  version bigint NOT NULL CHECK (version >= 0),
  outcome text NOT NULL CHECK (outcome IN ('accepted', 'succeeded', 'refused', 'failed', 'replayed')),
  receipt_json jsonb NOT NULL CHECK (jsonb_typeof(receipt_json) = 'object'),
  observed_at timestamptz NOT NULL
);

CREATE TABLE IF NOT EXISTS khala_sync_managed_sandbox_turns (
  sandbox_ref text NOT NULL REFERENCES khala_sync_managed_sandboxes(sandbox_ref) ON DELETE CASCADE,
  turn_sequence bigint NOT NULL CHECK (turn_sequence > 0),
  turn_ref text NOT NULL,
  command_ref text NOT NULL UNIQUE REFERENCES khala_sync_managed_sandbox_commands(command_ref),
  resource_generation bigint NOT NULL CHECK (resource_generation > 0),
  status text NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'running', 'settled', 'failed', 'interrupted')),
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  PRIMARY KEY (sandbox_ref, turn_sequence),
  UNIQUE (sandbox_ref, turn_ref),
  FOREIGN KEY (sandbox_ref, resource_generation)
    REFERENCES khala_sync_managed_sandbox_generations(sandbox_ref, resource_generation)
);

CREATE TABLE IF NOT EXISTS khala_sync_managed_sandbox_projection_cursors (
  sandbox_ref text NOT NULL REFERENCES khala_sync_managed_sandboxes(sandbox_ref) ON DELETE CASCADE,
  translator_ref text NOT NULL,
  projection_version bigint NOT NULL CHECK (projection_version > 0),
  native_event_sequence bigint NOT NULL CHECK (native_event_sequence >= 0),
  cursor_json jsonb NOT NULL CHECK (jsonb_typeof(cursor_json) = 'object'),
  updated_at timestamptz NOT NULL,
  PRIMARY KEY (sandbox_ref, translator_ref)
);

CREATE INDEX IF NOT EXISTS khala_sync_managed_sandbox_owner_lifecycle
  ON khala_sync_managed_sandboxes(owner_user_id, tenant_ref, lifecycle, updated_at DESC);

CREATE INDEX IF NOT EXISTS khala_sync_managed_sandbox_lease_expiry
  ON khala_sync_managed_sandboxes(lease_expires_at)
  WHERE lifecycle NOT IN ('deleted', 'failed');

CREATE INDEX IF NOT EXISTS khala_sync_managed_sandbox_pending_commands
  ON khala_sync_managed_sandbox_commands(created_at)
  WHERE status = 'pending';
