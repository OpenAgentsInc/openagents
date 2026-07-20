-- IDE-13 #9041: owner-private target to Pylon execution authority.

CREATE TABLE IF NOT EXISTS khala_sync_portable_target_pylon_bindings (
  binding_ref text NOT NULL UNIQUE,
  owner_user_id text NOT NULL,
  owner_agent_user_id text NOT NULL,
  session_ref text NOT NULL REFERENCES khala_sync_portable_sessions(session_ref) ON DELETE CASCADE,
  target_ref text NOT NULL REFERENCES khala_sync_portable_targets(target_ref),
  pylon_ref text NOT NULL REFERENCES pylon_registrations(pylon_ref),
  worker_instance_ref text NOT NULL,
  binding_digest text NOT NULL CHECK (binding_digest ~ '^sha256:[0-9a-f]{64}$'),
  revision bigint NOT NULL CHECK (revision > 0),
  state text NOT NULL CHECK (state IN ('active', 'revoked')),
  health text NOT NULL CHECK (health IN ('ready', 'draining', 'offline', 'revoked')),
  evidence_refs_json jsonb NOT NULL DEFAULT '[]'::jsonb,
  last_renewed_at timestamptz NOT NULL,
  expires_at timestamptz NOT NULL,
  revoked_at timestamptz,
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  PRIMARY KEY (owner_user_id, session_ref, target_ref),
  CHECK (expires_at > last_renewed_at),
  CHECK (
    (state = 'active' AND health IN ('ready', 'draining') AND revoked_at IS NULL) OR
    (state = 'revoked' AND health = 'revoked' AND revoked_at IS NOT NULL)
  )
);

CREATE INDEX IF NOT EXISTS khala_sync_portable_target_pylon_bindings_active
  ON khala_sync_portable_target_pylon_bindings
    (owner_user_id, session_ref, target_ref, state, expires_at);

CREATE INDEX IF NOT EXISTS khala_sync_portable_target_pylon_bindings_pylon
  ON khala_sync_portable_target_pylon_bindings(pylon_ref, state, expires_at);

CREATE TABLE IF NOT EXISTS khala_sync_portable_target_pylon_binding_events (
  event_ref text PRIMARY KEY,
  idempotency_key_hash text NOT NULL UNIQUE,
  binding_ref text NOT NULL,
  revision bigint NOT NULL CHECK (revision > 0),
  owner_user_id text NOT NULL,
  owner_agent_user_id text NOT NULL,
  session_ref text NOT NULL,
  target_ref text NOT NULL,
  pylon_ref text NOT NULL,
  worker_instance_ref text NOT NULL,
  binding_digest text NOT NULL CHECK (binding_digest ~ '^sha256:[0-9a-f]{64}$'),
  event_kind text NOT NULL CHECK (event_kind IN ('admitted', 'renewed', 'rebound', 'revoked', 'expired')),
  health text NOT NULL CHECK (health IN ('ready', 'draining', 'offline', 'revoked')),
  evidence_refs_json jsonb NOT NULL DEFAULT '[]'::jsonb,
  created_at timestamptz NOT NULL
);

CREATE INDEX IF NOT EXISTS khala_sync_portable_target_pylon_binding_events_binding
  ON khala_sync_portable_target_pylon_binding_events(binding_ref, revision);
