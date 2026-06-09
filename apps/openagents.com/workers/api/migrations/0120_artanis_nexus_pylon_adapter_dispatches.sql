CREATE TABLE IF NOT EXISTS artanis_nexus_pylon_adapter_dispatches (
  id TEXT PRIMARY KEY,
  agent_id TEXT NOT NULL,
  record_ref TEXT NOT NULL UNIQUE,
  idempotency_key TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL,
  active INTEGER NOT NULL DEFAULT 0,
  source_kind TEXT NOT NULL,
  scope_ref TEXT,
  parent_ref TEXT,
  record_json TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  closeout_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  closed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_artanis_nexus_pylon_adapter_dispatches_state_updated
  ON artanis_nexus_pylon_adapter_dispatches(state, updated_at DESC);
