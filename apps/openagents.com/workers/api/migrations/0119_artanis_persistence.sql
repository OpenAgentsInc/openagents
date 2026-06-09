CREATE TABLE IF NOT EXISTS artanis_runtime_snapshots (
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

CREATE INDEX IF NOT EXISTS idx_artanis_runtime_snapshots_agent_updated
  ON artanis_runtime_snapshots(agent_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS artanis_loop_records (
  id TEXT PRIMARY KEY,
  agent_id TEXT NOT NULL,
  record_ref TEXT NOT NULL UNIQUE,
  idempotency_key TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL,
  active INTEGER NOT NULL DEFAULT 0,
  source_kind TEXT NOT NULL,
  scope_ref TEXT NOT NULL,
  parent_ref TEXT,
  record_json TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  closeout_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  closed_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_artanis_loop_records_one_active_scope
  ON artanis_loop_records(agent_id, scope_ref)
  WHERE active = 1;

CREATE INDEX IF NOT EXISTS idx_artanis_loop_records_agent_updated
  ON artanis_loop_records(agent_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS artanis_loop_ticks (
  id TEXT PRIMARY KEY,
  agent_id TEXT NOT NULL,
  record_ref TEXT NOT NULL UNIQUE,
  idempotency_key TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL,
  active INTEGER NOT NULL DEFAULT 0,
  source_kind TEXT NOT NULL,
  scope_ref TEXT,
  parent_ref TEXT NOT NULL,
  record_json TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  closeout_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  closed_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_artanis_loop_ticks_loop_idempotency
  ON artanis_loop_ticks(parent_ref, idempotency_key);

CREATE INDEX IF NOT EXISTS idx_artanis_loop_ticks_parent_updated
  ON artanis_loop_ticks(parent_ref, updated_at DESC);

CREATE TABLE IF NOT EXISTS artanis_approval_gates (
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

CREATE INDEX IF NOT EXISTS idx_artanis_approval_gates_state_updated
  ON artanis_approval_gates(state, updated_at DESC);

CREATE TABLE IF NOT EXISTS artanis_health_snapshots (
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

CREATE INDEX IF NOT EXISTS idx_artanis_health_snapshots_loop_updated
  ON artanis_health_snapshots(parent_ref, updated_at DESC);

CREATE TABLE IF NOT EXISTS artanis_work_routing_proposals (
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

CREATE INDEX IF NOT EXISTS idx_artanis_work_routing_proposals_state_updated
  ON artanis_work_routing_proposals(state, updated_at DESC);

CREATE TABLE IF NOT EXISTS artanis_forum_publication_intents (
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

CREATE INDEX IF NOT EXISTS idx_artanis_forum_publication_intents_state_updated
  ON artanis_forum_publication_intents(state, updated_at DESC);
