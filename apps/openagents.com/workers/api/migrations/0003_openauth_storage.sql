CREATE TABLE openauth_storage (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  expires_at INTEGER,
  updated_at TEXT NOT NULL
);

CREATE INDEX openauth_storage_expires_at_idx
  ON openauth_storage(expires_at);
