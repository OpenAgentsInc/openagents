CREATE TABLE IF NOT EXISTS pylon_api_quarantines (
  id TEXT PRIMARY KEY,
  quarantine_ref TEXT NOT NULL UNIQUE,
  pylon_ref TEXT NOT NULL,
  owner_agent_user_id TEXT,
  state TEXT NOT NULL,
  reason_refs_json TEXT NOT NULL,
  action_refs_json TEXT NOT NULL,
  source_refs_json TEXT NOT NULL,
  expires_at TEXT,
  released_at TEXT,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (pylon_ref) REFERENCES pylon_api_registrations(pylon_ref)
);

CREATE INDEX IF NOT EXISTS idx_pylon_api_quarantines_active
  ON pylon_api_quarantines(pylon_ref, state, released_at, expires_at, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_pylon_api_quarantines_updated
  ON pylon_api_quarantines(updated_at DESC);
