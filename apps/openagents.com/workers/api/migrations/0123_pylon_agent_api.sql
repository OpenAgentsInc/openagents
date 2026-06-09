CREATE TABLE IF NOT EXISTS pylon_api_registrations (
  id TEXT PRIMARY KEY,
  pylon_ref TEXT NOT NULL UNIQUE,
  owner_agent_user_id TEXT NOT NULL,
  owner_agent_credential_id TEXT NOT NULL,
  owner_agent_token_prefix TEXT NOT NULL,
  display_name TEXT NOT NULL,
  status TEXT NOT NULL,
  resource_mode TEXT NOT NULL,
  capability_refs_json TEXT NOT NULL,
  wallet_ref TEXT,
  wallet_ready INTEGER NOT NULL DEFAULT 0,
  latest_heartbeat_at TEXT,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_pylon_api_registrations_owner_updated
  ON pylon_api_registrations(owner_agent_user_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_pylon_api_registrations_status_updated
  ON pylon_api_registrations(status, updated_at DESC);

CREATE TABLE IF NOT EXISTS pylon_api_events (
  id TEXT PRIMARY KEY,
  event_ref TEXT NOT NULL UNIQUE,
  pylon_ref TEXT NOT NULL,
  owner_agent_user_id TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  event_kind TEXT NOT NULL,
  assignment_ref TEXT,
  status TEXT NOT NULL,
  event_body_json TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (pylon_ref) REFERENCES pylon_api_registrations(pylon_ref)
);

CREATE INDEX IF NOT EXISTS idx_pylon_api_events_pylon_created
  ON pylon_api_events(pylon_ref, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_pylon_api_events_assignment_created
  ON pylon_api_events(assignment_ref, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_pylon_api_events_kind_created
  ON pylon_api_events(event_kind, created_at DESC);
