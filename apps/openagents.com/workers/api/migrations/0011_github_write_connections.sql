CREATE TABLE github_write_connections (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  github_id TEXT NOT NULL,
  github_login TEXT NOT NULL,
  connection_ref TEXT NOT NULL UNIQUE,
  secret_ref TEXT,
  scopes_json TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN ('connected', 'disconnected', 'unhealthy')
  ),
  health TEXT NOT NULL CHECK (
    health IN ('healthy', 'unhealthy', 'requires_reauth')
  ),
  connected_at TEXT,
  disconnected_at TEXT,
  last_status_at TEXT NOT NULL,
  metadata_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  FOREIGN KEY (user_id) REFERENCES users(id),
  UNIQUE (user_id, github_id)
);

CREATE INDEX github_write_connections_user_idx
  ON github_write_connections(user_id, status, health);

CREATE INDEX github_write_connections_ref_idx
  ON github_write_connections(connection_ref);

CREATE TABLE github_write_connection_attempts (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  state TEXT NOT NULL UNIQUE,
  expected_github_id TEXT NOT NULL,
  expected_github_login TEXT NOT NULL,
  redirect_after TEXT,
  scopes_json TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN ('pending', 'connected', 'expired', 'denied', 'failed')
  ),
  expires_at TEXT NOT NULL,
  completed_at TEXT,
  failed_at TEXT,
  failure_reason TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE INDEX github_write_attempts_user_created_idx
  ON github_write_connection_attempts(user_id, created_at);

CREATE INDEX github_write_attempts_status_expiry_idx
  ON github_write_connection_attempts(status, expires_at);

CREATE TABLE github_write_auth_grants (
  id TEXT PRIMARY KEY,
  connection_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  runner_session_id TEXT,
  connection_ref TEXT NOT NULL,
  secret_ref TEXT NOT NULL,
  grant_ref TEXT NOT NULL UNIQUE,
  status TEXT NOT NULL CHECK (
    status IN ('issued', 'used', 'expired', 'revoked', 'failed')
  ),
  requested_action TEXT,
  metadata_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  used_at TEXT,
  revoked_at TEXT,
  failed_at TEXT,
  FOREIGN KEY (connection_id) REFERENCES github_write_connections(id),
  FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE INDEX github_write_grants_user_created_idx
  ON github_write_auth_grants(user_id, created_at);

CREATE INDEX github_write_grants_runner_session_idx
  ON github_write_auth_grants(runner_session_id);

CREATE INDEX github_write_grants_status_expiry_idx
  ON github_write_auth_grants(status, expires_at);
