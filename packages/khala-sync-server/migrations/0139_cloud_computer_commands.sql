-- Durable cloud-computer command dispatch journal, bounded event history, and
-- opaque artifact references. Provider dispatch is never the source of truth.

CREATE TABLE IF NOT EXISTS khala_sync_cloud_computer_command_sessions (
  session_ref text PRIMARY KEY,
  computer_ref text NOT NULL REFERENCES khala_sync_cloud_computers (computer_ref),
  workspace_ref text NOT NULL REFERENCES khala_sync_cloud_computer_workspaces (workspace_ref),
  owner_ref text NOT NULL,
  tenant_ref text NOT NULL,
  runtime_generation bigint NOT NULL CHECK (runtime_generation > 0),
  runtime_ref text NOT NULL,
  provider_lease_ref text NOT NULL,
  authority_digest text NOT NULL CHECK (authority_digest ~ '^sha256:[0-9a-f]{64}$'),
  attachment_epoch bigint NOT NULL DEFAULT 0 CHECK (attachment_epoch >= 0),
  connection_ref text,
  last_session_sequence bigint NOT NULL DEFAULT 0 CHECK (last_session_sequence >= 0),
  retained_through_session_sequence bigint NOT NULL DEFAULT 0 CHECK (retained_through_session_sequence >= 0),
  retention_epoch bigint NOT NULL DEFAULT 0 CHECK (retention_epoch >= 0),
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  UNIQUE (computer_ref, runtime_generation, runtime_ref),
  FOREIGN KEY (workspace_ref, owner_ref, tenant_ref)
    REFERENCES khala_sync_cloud_computer_workspaces (workspace_ref, owner_ref, tenant_ref),
  CHECK (retained_through_session_sequence <= last_session_sequence)
);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_computer_command_credential_nonces (
  nonce text PRIMARY KEY,
  session_ref text NOT NULL REFERENCES khala_sync_cloud_computer_command_sessions (session_ref),
  authority_digest text NOT NULL CHECK (authority_digest ~ '^sha256:[0-9a-f]{64}$'),
  attachment_epoch bigint NOT NULL CHECK (attachment_epoch > 0),
  consumed_at timestamptz NOT NULL,
  UNIQUE (session_ref, attachment_epoch)
);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_computer_commands (
  command_ref text PRIMARY KEY,
  idempotency_ref text NOT NULL,
  request_digest text NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),
  computer_ref text NOT NULL REFERENCES khala_sync_cloud_computers (computer_ref),
  workspace_ref text NOT NULL REFERENCES khala_sync_cloud_computer_workspaces (workspace_ref),
  session_ref text NOT NULL REFERENCES khala_sync_cloud_computer_command_sessions (session_ref),
  owner_ref text NOT NULL,
  tenant_ref text NOT NULL,
  runtime_generation bigint NOT NULL CHECK (runtime_generation > 0),
  runtime_ref text NOT NULL,
  provider_lease_ref text NOT NULL,
  working_directory text NOT NULL CHECK (working_directory ~ '^/'),
  capability_refs_json jsonb NOT NULL CHECK (jsonb_typeof(capability_refs_json) = 'array'),
  capability_digest text NOT NULL CHECK (capability_digest ~ '^sha256:[0-9a-f]{64}$'),
  budget_snapshot_digest text NOT NULL CHECK (budget_snapshot_digest ~ '^sha256:[0-9a-f]{64}$'),
  budget_limits_json jsonb NOT NULL CHECK (jsonb_typeof(budget_limits_json) = 'object'),
  deadline_at timestamptz NOT NULL,
  request_json jsonb NOT NULL CHECK (jsonb_typeof(request_json) = 'object'),
  status text NOT NULL CHECK (status IN (
    'admitted', 'not_dispatched', 'dispatched', 'may_have_started', 'running',
    'completed', 'failed', 'cancelled', 'timed_out', 'lost'
  )),
  provider_command_ref text,
  reservation_ref text,
  provider_execution_ref text,
  acknowledgement_event_ref text,
  acknowledgement_event_digest text CHECK (
    acknowledgement_event_digest IS NULL OR acknowledgement_event_digest ~ '^sha256:[0-9a-f]{64}$'
  ),
  dispatch_ref text,
  terminal_ref text,
  terminal_digest text CHECK (terminal_digest IS NULL OR terminal_digest ~ '^sha256:[0-9a-f]{64}$'),
  terminal_reason text,
  terminal_exit_code integer CHECK (terminal_exit_code >= 0),
  terminal_output_digest text CHECK (
    terminal_output_digest IS NULL OR terminal_output_digest ~ '^sha256:[0-9a-f]{64}$'
  ),
  terminal_session_sequence bigint,
  terminal_command_sequence bigint,
  fence bigint NOT NULL DEFAULT 1 CHECK (fence > 0),
  next_command_sequence bigint NOT NULL DEFAULT 1 CHECK (next_command_sequence > 0),
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  dispatched_at timestamptz,
  started_at timestamptz,
  completed_at timestamptz,
  revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
  UNIQUE (computer_ref, runtime_generation, idempotency_ref),
  UNIQUE (provider_lease_ref, provider_command_ref),
  FOREIGN KEY (workspace_ref, owner_ref, tenant_ref)
    REFERENCES khala_sync_cloud_computer_workspaces (workspace_ref, owner_ref, tenant_ref),
  CHECK ((status IN ('dispatched', 'may_have_started', 'running', 'completed', 'failed',
                     'cancelled', 'timed_out', 'lost')) = (dispatch_ref IS NOT NULL)),
  CHECK (status NOT IN ('dispatched', 'running', 'completed', 'failed', 'cancelled', 'timed_out')
      OR provider_command_ref IS NOT NULL),
  CHECK (provider_command_ref IS NULL
      OR status IN ('dispatched', 'may_have_started', 'running', 'completed', 'failed',
                    'cancelled', 'timed_out', 'lost')),
  CHECK (status NOT IN ('dispatched', 'running', 'completed', 'failed', 'cancelled', 'timed_out')
      OR (reservation_ref IS NOT NULL AND provider_execution_ref IS NOT NULL
          AND acknowledgement_event_ref IS NOT NULL AND acknowledgement_event_digest IS NOT NULL)),
  CHECK ((status IN ('completed', 'failed', 'cancelled', 'timed_out', 'lost'))
      = (terminal_ref IS NOT NULL)),
  CHECK ((status IN ('completed', 'failed', 'cancelled', 'timed_out', 'lost'))
      = (terminal_digest IS NOT NULL)),
  CHECK ((status IN ('completed', 'failed', 'cancelled', 'timed_out', 'lost'))
      = (terminal_reason IS NOT NULL)),
  CHECK (terminal_exit_code IS NULL
      OR status IN ('completed', 'failed', 'cancelled', 'timed_out')),
  CHECK (terminal_output_digest IS NULL
      OR status IN ('completed', 'failed', 'cancelled', 'timed_out')),
  CHECK (status <> 'completed' OR terminal_exit_code = 0),
  CHECK ((status IN ('completed', 'failed', 'cancelled', 'timed_out', 'lost'))
      = (completed_at IS NOT NULL)),
  CHECK ((status IN ('completed', 'failed', 'cancelled', 'timed_out'))
      = (terminal_session_sequence IS NOT NULL)),
  CHECK ((status IN ('completed', 'failed', 'cancelled', 'timed_out'))
      = (terminal_command_sequence IS NOT NULL)),
  CHECK (updated_at >= created_at)
);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_computer_command_dispatch_attempts (
  attempt_ref text PRIMARY KEY,
  command_ref text NOT NULL REFERENCES khala_sync_cloud_computer_commands (command_ref),
  runtime_generation bigint NOT NULL CHECK (runtime_generation > 0),
  runtime_ref text NOT NULL,
  provider_lease_ref text NOT NULL,
  status text NOT NULL CHECK (status IN (
    'prepared', 'not_exposed', 'write_exposed', 'may_have_started',
    'reservation_recorded', 'acknowledged'
  )),
  provider_command_ref text,
  reservation_ref text,
  provider_execution_ref text,
  acknowledgement_event_ref text,
  acknowledgement_event_digest text CHECK (
    acknowledgement_event_digest IS NULL OR acknowledgement_event_digest ~ '^sha256:[0-9a-f]{64}$'
  ),
  prepared_at timestamptz NOT NULL,
  write_exposed_at timestamptz,
  settled_at timestamptz,
  revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
  UNIQUE (command_ref, attempt_ref),
  CHECK ((status IN ('write_exposed', 'may_have_started', 'reservation_recorded', 'acknowledged'))
      = (write_exposed_at IS NOT NULL)),
  CHECK ((status IN ('not_exposed', 'may_have_started', 'acknowledged'))
      = (settled_at IS NOT NULL)),
  CHECK ((status IN ('reservation_recorded', 'acknowledged')) = (provider_command_ref IS NOT NULL)),
  CHECK ((status IN ('reservation_recorded', 'acknowledged')) = (reservation_ref IS NOT NULL)),
  CHECK ((status IN ('reservation_recorded', 'acknowledged')) = (provider_execution_ref IS NOT NULL)),
  CHECK ((status = 'acknowledged') = (acknowledgement_event_ref IS NOT NULL)),
  CHECK ((status = 'acknowledged') = (acknowledgement_event_digest IS NOT NULL))
);

CREATE INDEX IF NOT EXISTS khala_sync_cloud_computer_commands_recovery
  ON khala_sync_cloud_computer_commands (status, updated_at, computer_ref, runtime_generation);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_computer_command_recovery_evidence (
  evidence_ref text PRIMARY KEY,
  kind text NOT NULL CHECK (kind IN (
    'runtime_lost', 'host_lost', 'checkpoint_failed', 'cleanup_failed'
  )),
  evidence_digest text NOT NULL CHECK (evidence_digest ~ '^sha256:[0-9a-f]{64}$'),
  computer_ref text NOT NULL REFERENCES khala_sync_cloud_computers (computer_ref),
  workspace_ref text NOT NULL REFERENCES khala_sync_cloud_computer_workspaces (workspace_ref),
  runtime_generation bigint NOT NULL CHECK (runtime_generation > 0),
  runtime_ref text NOT NULL,
  provider_lease_ref text NOT NULL,
  observed_at timestamptz NOT NULL,
  affected_command_count bigint NOT NULL DEFAULT 0 CHECK (affected_command_count >= 0),
  UNIQUE (kind, computer_ref, workspace_ref, runtime_generation, runtime_ref, provider_lease_ref,
          evidence_digest)
);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_computer_command_events (
  command_ref text NOT NULL REFERENCES khala_sync_cloud_computer_commands (command_ref),
  session_ref text NOT NULL REFERENCES khala_sync_cloud_computer_command_sessions (session_ref),
  session_sequence bigint NOT NULL CHECK (session_sequence > 0),
  command_sequence bigint NOT NULL CHECK (command_sequence > 0),
  attachment_epoch bigint NOT NULL CHECK (attachment_epoch > 0),
  fence bigint NOT NULL CHECK (fence > 0),
  event_ref text NOT NULL UNIQUE,
  event_digest text NOT NULL CHECK (event_digest ~ '^sha256:[0-9a-f]{64}$'),
  kind text NOT NULL,
  payload_json jsonb NOT NULL CHECK (jsonb_typeof(payload_json) = 'object'),
  artifact_refs_json jsonb NOT NULL CHECK (jsonb_typeof(artifact_refs_json) = 'array'),
  observed_at timestamptz NOT NULL,
  PRIMARY KEY (session_ref, session_sequence),
  UNIQUE (command_ref, command_sequence)
);

CREATE INDEX IF NOT EXISTS khala_sync_cloud_computer_command_events_retention
  ON khala_sync_cloud_computer_command_events (session_ref, session_sequence DESC);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_computer_command_settlements (
  settlement_ref text PRIMARY KEY,
  command_ref text NOT NULL REFERENCES khala_sync_cloud_computer_commands (command_ref),
  kind text NOT NULL CHECK (kind IN ('cancel', 'timeout')),
  expected_fence bigint NOT NULL CHECK (expected_fence > 0),
  settled_fence bigint NOT NULL CHECK (settled_fence > expected_fence),
  request_digest text NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),
  reason text NOT NULL,
  requested_at timestamptz NOT NULL,
  UNIQUE (command_ref, kind),
  UNIQUE (command_ref, settled_fence)
);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_computer_command_artifacts (
  artifact_ref text PRIMARY KEY,
  command_ref text NOT NULL REFERENCES khala_sync_cloud_computer_commands (command_ref),
  runtime_generation bigint NOT NULL CHECK (runtime_generation > 0),
  kind text NOT NULL CHECK (kind IN ('stdout', 'stderr', 'result', 'diagnostic')),
  object_ref text NOT NULL,
  object_generation text NOT NULL CHECK (object_generation ~ '^[0-9]+$'),
  content_digest text NOT NULL CHECK (content_digest ~ '^sha256:[0-9a-f]{64}$'),
  byte_count bigint NOT NULL CHECK (byte_count >= 0),
  created_at timestamptz NOT NULL,
  retain_until timestamptz NOT NULL CHECK (retain_until >= created_at),
  UNIQUE (command_ref, artifact_ref)
);
