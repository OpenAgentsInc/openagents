-- IDE-13 #9041: refs-only owner-managed environment enrollment authority.
-- Owner-held checkpoint key bytes stay on the enrolled Pylon. This table
-- stores only the revocable key reference and public-safe policy facts.

CREATE TABLE IF NOT EXISTS khala_sync_owner_managed_environment_enrollments (
  enrollment_ref text NOT NULL UNIQUE,
  owner_user_id text NOT NULL,
  owner_agent_user_id text NOT NULL,
  target_ref text NOT NULL REFERENCES khala_sync_portable_targets(target_ref),
  pylon_ref text NOT NULL REFERENCES pylon_registrations(pylon_ref),
  worker_instance_ref text NOT NULL,
  adapter_ref text NOT NULL,
  compatibility_ref text NOT NULL,
  isolation text NOT NULL CHECK (isolation IN ('owner_host_process', 'owner_host_container')),
  checkpoint_key_ref text NOT NULL,
  region_ref text NOT NULL,
  network_destination_refs_json jsonb NOT NULL DEFAULT '[]'::jsonb,
  data_destination_refs_json jsonb NOT NULL DEFAULT '[]'::jsonb,
  retention_seconds bigint NOT NULL CHECK (retention_seconds BETWEEN 0 AND 31536000),
  cost_policy_ref text NOT NULL,
  generation bigint NOT NULL CHECK (generation > 0),
  revision bigint NOT NULL CHECK (revision > 0),
  state text NOT NULL CHECK (state IN ('active', 'revoked')),
  health text NOT NULL CHECK (health IN ('ready', 'draining', 'offline', 'revoked')),
  evidence_refs_json jsonb NOT NULL DEFAULT '[]'::jsonb,
  observed_at timestamptz NOT NULL,
  expires_at timestamptz NOT NULL,
  revoked_at timestamptz,
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  PRIMARY KEY (owner_user_id, target_ref),
  CHECK (expires_at > observed_at),
  CHECK (
    (state = 'active' AND health IN ('ready', 'draining') AND revoked_at IS NULL) OR
    (state = 'revoked' AND health = 'revoked' AND revoked_at IS NOT NULL)
  )
);

CREATE INDEX IF NOT EXISTS khala_sync_owner_managed_enrollments_active
  ON khala_sync_owner_managed_environment_enrollments
    (owner_user_id, target_ref, state, expires_at);

CREATE TABLE IF NOT EXISTS khala_sync_owner_managed_environment_enrollment_events (
  event_ref text PRIMARY KEY,
  idempotency_key_hash text NOT NULL UNIQUE,
  request_digest text NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),
  enrollment_ref text NOT NULL,
  owner_user_id text NOT NULL,
  owner_agent_user_id text NOT NULL,
  target_ref text NOT NULL,
  pylon_ref text NOT NULL,
  generation bigint NOT NULL CHECK (generation > 0),
  revision bigint NOT NULL CHECK (revision > 0),
  event_kind text NOT NULL CHECK (event_kind IN ('admitted', 'renewed', 'revoked', 'expired')),
  health text NOT NULL CHECK (health IN ('ready', 'draining', 'offline', 'revoked')),
  evidence_refs_json jsonb NOT NULL DEFAULT '[]'::jsonb,
  created_at timestamptz NOT NULL
);

CREATE INDEX IF NOT EXISTS khala_sync_owner_managed_enrollment_events_enrollment
  ON khala_sync_owner_managed_environment_enrollment_events(enrollment_ref, revision);
