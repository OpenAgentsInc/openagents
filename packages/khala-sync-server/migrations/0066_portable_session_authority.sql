-- PORT-01 #8746: host-independent portable coding-session authority.
--
-- The append-only event log and command ledger are authority. Current rows are
-- bounded derived projections repaired from those logs. Attachments are
-- generation fenced and the partial unique index admits at most one generation
-- capable of accepting work. Checkpoints and rows are refs/digests only; raw
-- credentials, paths, processes, sockets, provider payloads, and private
-- transcript bytes have no columns here.

CREATE TABLE IF NOT EXISTS khala_sync_portable_sessions (
  session_ref text PRIMARY KEY,
  owner_user_id text NOT NULL,
  owner_scope_ref text NOT NULL,
  work_context_ref text NOT NULL,
  event_log_ref text NOT NULL UNIQUE,
  current_projection_ref text NOT NULL UNIQUE,
  command_scope_ref text NOT NULL UNIQUE,
  root_agent_ref text NOT NULL,
  adopted_from_local_history boolean NOT NULL DEFAULT false,
  adoption_receipt_ref text,
  state text NOT NULL CHECK (state IN ('active', 'quiescing', 'detached', 'recovery_required', 'stopped')),
  latest_event_cursor bigint NOT NULL DEFAULT 0 CHECK (latest_event_cursor >= 0),
  current_attachment_ref text,
  current_attachment_generation bigint NOT NULL DEFAULT 0 CHECK (current_attachment_generation >= 0),
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  CHECK (owner_scope_ref = 'scope.user.' || owner_user_id),
  CHECK (NOT adopted_from_local_history OR adoption_receipt_ref IS NOT NULL)
);

CREATE TABLE IF NOT EXISTS khala_sync_portable_targets (
  target_ref text PRIMARY KEY,
  owner_user_id text NOT NULL,
  target_class text NOT NULL CHECK (target_class IN ('owner_local', 'owner_managed', 'openagents_managed', 'managed_provider')),
  adapter_ref text NOT NULL,
  compatibility_ref text NOT NULL,
  isolation text NOT NULL CHECK (isolation IN ('owner_host_process', 'owner_host_container', 'dedicated_microvm')),
  data_posture text NOT NULL CHECK (data_posture IN ('owner_device_only', 'owner_managed_region', 'openagents_managed_region')),
  health text NOT NULL CHECK (health IN ('ready', 'offline', 'incompatible', 'revoked', 'upgrading', 'draining', 'unavailable')),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS khala_sync_portable_session_targets (
  session_ref text NOT NULL REFERENCES khala_sync_portable_sessions(session_ref) ON DELETE CASCADE,
  target_ref text NOT NULL REFERENCES khala_sync_portable_targets(target_ref),
  authorized_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (session_ref, target_ref)
);

CREATE TABLE IF NOT EXISTS khala_sync_portable_agent_nodes (
  session_ref text NOT NULL REFERENCES khala_sync_portable_sessions(session_ref) ON DELETE CASCADE,
  agent_ref text NOT NULL,
  parent_agent_ref text,
  thread_ref text NOT NULL,
  transcript_ref text NOT NULL,
  activity_cursor bigint NOT NULL CHECK (activity_cursor >= 0),
  lifecycle text NOT NULL CHECK (lifecycle IN ('created', 'running', 'waiting', 'quiescing', 'quiesced', 'completed', 'failed', 'canceled', 'interrupted')),
  attachment_generation bigint NOT NULL CHECK (attachment_generation >= 0),
  PRIMARY KEY (session_ref, agent_ref),
  UNIQUE (session_ref, thread_ref),
  UNIQUE (session_ref, transcript_ref)
);

CREATE TABLE IF NOT EXISTS khala_sync_portable_attachments (
  attachment_ref text PRIMARY KEY,
  session_ref text NOT NULL REFERENCES khala_sync_portable_sessions(session_ref) ON DELETE CASCADE,
  target_ref text NOT NULL REFERENCES khala_sync_portable_targets(target_ref),
  generation bigint NOT NULL CHECK (generation > 0),
  state text NOT NULL CHECK (state IN ('preparing', 'active', 'quiescing', 'quiesced', 'detached', 'failed', 'reclaimed')),
  descendant_agent_refs_json jsonb NOT NULL,
  capability_lease_refs_json jsonb NOT NULL,
  checkpoint_ref text,
  evidence_refs_json jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (session_ref, generation)
);

CREATE UNIQUE INDEX IF NOT EXISTS khala_sync_portable_one_live_attachment
  ON khala_sync_portable_attachments(session_ref)
  WHERE state IN ('preparing', 'active', 'quiescing');

CREATE TABLE IF NOT EXISTS khala_sync_portable_checkpoints (
  checkpoint_ref text PRIMARY KEY,
  session_ref text NOT NULL REFERENCES khala_sync_portable_sessions(session_ref) ON DELETE CASCADE,
  source_attachment_ref text NOT NULL REFERENCES khala_sync_portable_attachments(attachment_ref),
  source_generation bigint NOT NULL CHECK (source_generation > 0),
  digest text NOT NULL CHECK (digest ~ '^sha256:[0-9a-f]{64}$'),
  parent_checkpoint_ref text,
  repository_ref text NOT NULL,
  repository_revision_ref text NOT NULL,
  repository_post_image_digest text NOT NULL CHECK (repository_post_image_digest ~ '^sha256:[0-9a-f]{64}$'),
  diff_digest text NOT NULL CHECK (diff_digest ~ '^sha256:[0-9a-f]{64}$'),
  event_log_cursor bigint NOT NULL CHECK (event_log_cursor >= 0),
  catalog_generation_ref text NOT NULL,
  graph_digest text NOT NULL CHECK (graph_digest ~ '^sha256:[0-9a-f]{64}$'),
  approval_refs_json jsonb NOT NULL,
  artifact_refs_json jsonb NOT NULL,
  receipt_refs_json jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS khala_sync_portable_events (
  event_seq bigserial PRIMARY KEY,
  session_ref text NOT NULL REFERENCES khala_sync_portable_sessions(session_ref) ON DELETE CASCADE,
  event_ref text NOT NULL UNIQUE,
  thread_ref text NOT NULL,
  thread_cursor bigint NOT NULL CHECK (thread_cursor > 0),
  attachment_ref text NOT NULL REFERENCES khala_sync_portable_attachments(attachment_ref),
  attachment_generation bigint NOT NULL CHECK (attachment_generation > 0),
  event_kind text NOT NULL CHECK (event_kind IN ('agent_lifecycle', 'activity_cursor', 'command_outcome', 'checkpoint_sealed', 'attachment_transition', 'projection_repaired')),
  event_json jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (session_ref, thread_ref, thread_cursor)
);

CREATE TABLE IF NOT EXISTS khala_sync_portable_thread_current (
  session_ref text NOT NULL REFERENCES khala_sync_portable_sessions(session_ref) ON DELETE CASCADE,
  thread_ref text NOT NULL,
  latest_cursor bigint NOT NULL CHECK (latest_cursor >= 0),
  current_json jsonb NOT NULL,
  repaired_from_event_seq bigint NOT NULL CHECK (repaired_from_event_seq >= 0),
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (session_ref, thread_ref)
);

CREATE TABLE IF NOT EXISTS khala_sync_portable_commands (
  command_ref text PRIMARY KEY,
  idempotency_key text NOT NULL UNIQUE,
  owner_user_id text NOT NULL,
  session_ref text NOT NULL REFERENCES khala_sync_portable_sessions(session_ref) ON DELETE CASCADE,
  kind text NOT NULL CHECK (kind IN ('stop', 'checkpoint', 'detach', 'attach', 'move', 'abort_move', 'resume', 'failback')),
  expected_attachment_ref text NOT NULL,
  expected_generation bigint NOT NULL CHECK (expected_generation > 0),
  destination_target_ref text,
  checkpoint_ref text,
  expires_at timestamptz NOT NULL,
  command_json jsonb NOT NULL,
  status text NOT NULL CHECK (status IN ('accepted', 'rejected', 'failed', 'unknown_pending_reconcile', 'completed', 'expired')),
  outcome_json jsonb,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS khala_sync_portable_events_session_seq
  ON khala_sync_portable_events(session_ref, event_seq);
CREATE INDEX IF NOT EXISTS khala_sync_portable_commands_pending
  ON khala_sync_portable_commands(session_ref, created_at)
  WHERE status IN ('accepted', 'unknown_pending_reconcile');
