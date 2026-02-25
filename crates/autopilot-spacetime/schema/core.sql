CREATE TABLE IF NOT EXISTS sync_stream (
  stream_id TEXT PRIMARY KEY,
  stream_class TEXT NOT NULL,
  owner_scope TEXT NOT NULL,
  created_at_unix_ms BIGINT NOT NULL,
  updated_at_unix_ms BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_event (
  stream_id TEXT NOT NULL,
  seq BIGINT NOT NULL,
  idempotency_key TEXT NOT NULL,
  payload_hash TEXT NOT NULL,
  payload_bytes BLOB NOT NULL,
  committed_at_unix_ms BIGINT NOT NULL,
  durable_offset BIGINT DEFAULT 0,
  PRIMARY KEY (stream_id, seq)
);

CREATE TABLE IF NOT EXISTS sync_checkpoint (
  client_id TEXT NOT NULL,
  stream_id TEXT NOT NULL,
  last_applied_seq BIGINT NOT NULL,
  durable_offset BIGINT NOT NULL,
  updated_at_unix_ms BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS session_presence (
  node_id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  status TEXT NOT NULL,
  region TEXT NOT NULL,
  last_seen_unix_ms BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS provider_capability (
  provider_id TEXT PRIMARY KEY,
  capability_json TEXT NOT NULL,
  price_hint_sats BIGINT NOT NULL,
  updated_at_unix_ms BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS compute_assignment (
  request_id TEXT PRIMARY KEY,
  provider_id TEXT NOT NULL,
  status TEXT NOT NULL,
  assignment_json TEXT NOT NULL,
  updated_at_unix_ms BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS bridge_outbox (
  event_id TEXT PRIMARY KEY,
  transport TEXT NOT NULL,
  status TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  created_at_unix_ms BIGINT NOT NULL,
  updated_at_unix_ms BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS presence_event (
  event_id TEXT PRIMARY KEY,
  node_id TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  committed_at_unix_ms BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS coordination_event (
  event_id TEXT PRIMARY KEY,
  stream_id TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  committed_at_unix_ms BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS conflict_event (
  event_id TEXT PRIMARY KEY,
  stream_id TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  committed_at_unix_ms BIGINT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_sync_stream_stream_id
  ON sync_stream(stream_id);
CREATE UNIQUE INDEX IF NOT EXISTS ux_sync_event_stream_seq
  ON sync_event(stream_id, seq);
CREATE UNIQUE INDEX IF NOT EXISTS ux_sync_event_stream_idempotency
  ON sync_event(stream_id, idempotency_key);
CREATE INDEX IF NOT EXISTS ix_sync_event_commit_ts
  ON sync_event(committed_at_unix_ms);

CREATE UNIQUE INDEX IF NOT EXISTS ux_sync_checkpoint_client_stream
  ON sync_checkpoint(client_id, stream_id);
CREATE INDEX IF NOT EXISTS ix_session_presence_last_seen
  ON session_presence(last_seen_unix_ms);
CREATE UNIQUE INDEX IF NOT EXISTS ux_provider_capability_provider
  ON provider_capability(provider_id);
CREATE UNIQUE INDEX IF NOT EXISTS ux_compute_assignment_request
  ON compute_assignment(request_id);
CREATE INDEX IF NOT EXISTS ix_compute_assignment_status_updated
  ON compute_assignment(status, updated_at_unix_ms);
CREATE INDEX IF NOT EXISTS ix_bridge_outbox_status_created
  ON bridge_outbox(status, created_at_unix_ms);
CREATE INDEX IF NOT EXISTS ix_presence_event_committed
  ON presence_event(committed_at_unix_ms);
CREATE INDEX IF NOT EXISTS ix_coordination_event_committed
  ON coordination_event(committed_at_unix_ms);
CREATE INDEX IF NOT EXISTS ix_conflict_event_committed
  ON conflict_event(committed_at_unix_ms);

