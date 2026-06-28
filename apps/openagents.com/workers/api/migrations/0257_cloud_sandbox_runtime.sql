CREATE TABLE IF NOT EXISTS cloud_sandbox_sessions (
  sandbox_id TEXT PRIMARY KEY,
  account_ref TEXT NOT NULL,
  image TEXT NOT NULL,
  ttl_seconds INTEGER NOT NULL CHECK (ttl_seconds > 0),
  status TEXT NOT NULL CHECK (status IN ('provisioning', 'ready', 'stopped', 'expired', 'failed')),
  connection_ref TEXT,
  usage_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  expires_at_hint TEXT,
  completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_cloud_sandbox_sessions_account
  ON cloud_sandbox_sessions (account_ref, created_at DESC);
