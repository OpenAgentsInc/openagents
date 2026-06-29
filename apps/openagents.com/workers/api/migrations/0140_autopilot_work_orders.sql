CREATE TABLE IF NOT EXISTS autopilot_work_orders (
  id TEXT PRIMARY KEY,
  work_order_ref TEXT NOT NULL UNIQUE,
  owner_user_id TEXT NOT NULL,
  agent_user_id TEXT NOT NULL,
  agent_credential_id TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL,
  client_request_ref TEXT NOT NULL,
  request_json TEXT NOT NULL,
  state TEXT NOT NULL,
  task_refs_json TEXT NOT NULL,
  access_request_refs_json TEXT NOT NULL,
  payment_challenge_ref TEXT,
  status_url_ref TEXT NOT NULL,
  event_stream_ref TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE(owner_user_id, idempotency_key_hash)
);

CREATE INDEX IF NOT EXISTS idx_autopilot_work_orders_owner_created
  ON autopilot_work_orders(owner_user_id, created_at DESC)
  WHERE archived_at IS NULL;
