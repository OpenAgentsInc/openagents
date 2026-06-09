CREATE TABLE IF NOT EXISTS site_builder_sessions (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  site_id TEXT,
  order_id TEXT,
  workroom_id TEXT,
  owner_user_id TEXT NOT NULL,
  customer_user_id TEXT,
  created_by_actor_ref TEXT NOT NULL,
  status TEXT NOT NULL,
  prompt_summary TEXT NOT NULL,
  source_site_version_id TEXT,
  source_revision_id TEXT,
  active_preview_id TEXT,
  active_artifact_id TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_site_builder_sessions_owner_created
  ON site_builder_sessions(owner_user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_site_builder_sessions_order_created
  ON site_builder_sessions(order_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_site_builder_sessions_site_created
  ON site_builder_sessions(site_id, created_at DESC);

CREATE TABLE IF NOT EXISTS site_builder_messages (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  session_id TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  actor_kind TEXT NOT NULL,
  visibility TEXT NOT NULL,
  body TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (session_id) REFERENCES site_builder_sessions(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_site_builder_messages_session_sequence
  ON site_builder_messages(session_id, sequence);

CREATE INDEX IF NOT EXISTS idx_site_builder_messages_session_created
  ON site_builder_messages(session_id, created_at);

CREATE TABLE IF NOT EXISTS site_builder_phase_runs (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  session_id TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  phase_kind TEXT NOT NULL,
  status TEXT NOT NULL,
  title TEXT NOT NULL,
  summary TEXT NOT NULL,
  started_at TEXT,
  completed_at TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (session_id) REFERENCES site_builder_sessions(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_site_builder_phase_runs_session_sequence
  ON site_builder_phase_runs(session_id, sequence);

CREATE TABLE IF NOT EXISTS site_builder_events (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  session_id TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  event_kind TEXT NOT NULL,
  phase_kind TEXT,
  visibility TEXT NOT NULL,
  status TEXT NOT NULL,
  title TEXT NOT NULL,
  summary TEXT NOT NULL,
  source_ref TEXT,
  payload_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (session_id) REFERENCES site_builder_sessions(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_site_builder_events_session_sequence
  ON site_builder_events(session_id, sequence);

CREATE INDEX IF NOT EXISTS idx_site_builder_events_session_created
  ON site_builder_events(session_id, created_at);

CREATE TABLE IF NOT EXISTS site_builder_file_snapshots (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  session_id TEXT NOT NULL,
  path TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  language TEXT,
  content_hash TEXT NOT NULL,
  byte_size INTEGER NOT NULL,
  source_ref TEXT,
  artifact_ref TEXT,
  preview_text TEXT,
  visibility TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (session_id) REFERENCES site_builder_sessions(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_site_builder_file_snapshots_session_path_sequence
  ON site_builder_file_snapshots(session_id, path, sequence);

CREATE INDEX IF NOT EXISTS idx_site_builder_file_snapshots_session_path
  ON site_builder_file_snapshots(session_id, path);

CREATE TABLE IF NOT EXISTS site_builder_previews (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  session_id TEXT NOT NULL,
  preview_kind TEXT NOT NULL,
  status TEXT NOT NULL,
  preview_url TEXT,
  version_ref TEXT,
  artifact_ref TEXT,
  health_ref TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (session_id) REFERENCES site_builder_sessions(id)
);

CREATE INDEX IF NOT EXISTS idx_site_builder_previews_session_created
  ON site_builder_previews(session_id, created_at DESC);

CREATE TABLE IF NOT EXISTS site_builder_artifacts (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  session_id TEXT NOT NULL,
  artifact_kind TEXT NOT NULL,
  artifact_ref TEXT NOT NULL,
  content_hash TEXT,
  byte_size INTEGER,
  manifest_ref TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (session_id) REFERENCES site_builder_sessions(id)
);

CREATE INDEX IF NOT EXISTS idx_site_builder_artifacts_session_created
  ON site_builder_artifacts(session_id, created_at DESC);
