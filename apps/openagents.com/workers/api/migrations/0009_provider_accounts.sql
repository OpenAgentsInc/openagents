CREATE TABLE provider_accounts (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  team_id TEXT,
  provider TEXT NOT NULL CHECK (provider IN ('chatgpt_codex')),
  auth_mode TEXT NOT NULL CHECK (
    auth_mode IN ('chatgpt_device_code', 'codex_device_auth', 'manual_secret_ref')
  ),
  status TEXT NOT NULL CHECK (
    status IN ('pending', 'connected', 'expired', 'denied', 'disconnected', 'unhealthy')
  ),
  health TEXT NOT NULL CHECK (
    health IN ('unknown', 'healthy', 'unhealthy', 'requires_reauth')
  ),
  provider_account_ref TEXT NOT NULL UNIQUE,
  secret_ref TEXT,
  account_label TEXT,
  plan_type TEXT,
  connected_at TEXT,
  disconnected_at TEXT,
  denied_at TEXT,
  last_status_at TEXT NOT NULL,
  metadata_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  FOREIGN KEY (user_id) REFERENCES users(id),
  FOREIGN KEY (team_id) REFERENCES teams(id)
);

CREATE INDEX provider_accounts_user_provider_idx
  ON provider_accounts(user_id, provider);

CREATE INDEX provider_accounts_team_provider_idx
  ON provider_accounts(team_id, provider);

CREATE INDEX provider_accounts_status_health_idx
  ON provider_accounts(status, health);

CREATE TABLE provider_account_connection_attempts (
  id TEXT PRIMARY KEY,
  provider_account_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  team_id TEXT,
  provider TEXT NOT NULL CHECK (provider IN ('chatgpt_codex')),
  method TEXT NOT NULL CHECK (method IN ('chatgpt_device_code')),
  source TEXT NOT NULL CHECK (
    source IN ('shc_broker', 'worker_device_code', 'manual_placeholder')
  ),
  login_ref TEXT,
  verification_url TEXT,
  user_code TEXT,
  status TEXT NOT NULL CHECK (
    status IN ('pending', 'connected', 'expired', 'denied', 'failed')
  ),
  expires_at TEXT NOT NULL,
  completed_at TEXT,
  failed_at TEXT,
  metadata_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (provider_account_id) REFERENCES provider_accounts(id),
  FOREIGN KEY (user_id) REFERENCES users(id),
  FOREIGN KEY (team_id) REFERENCES teams(id)
);

CREATE INDEX provider_connection_attempts_user_created_idx
  ON provider_account_connection_attempts(user_id, created_at);

CREATE INDEX provider_connection_attempts_provider_account_idx
  ON provider_account_connection_attempts(provider_account_id, created_at);

CREATE INDEX provider_connection_attempts_status_expiry_idx
  ON provider_account_connection_attempts(status, expires_at);

CREATE TABLE provider_account_auth_grants (
  id TEXT PRIMARY KEY,
  provider_account_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  team_id TEXT,
  thread_id TEXT,
  workroom_id TEXT,
  runner_session_id TEXT,
  provider TEXT NOT NULL CHECK (provider IN ('chatgpt_codex')),
  provider_account_ref TEXT NOT NULL,
  provider_secret_ref TEXT NOT NULL,
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
  FOREIGN KEY (provider_account_id) REFERENCES provider_accounts(id),
  FOREIGN KEY (user_id) REFERENCES users(id),
  FOREIGN KEY (team_id) REFERENCES teams(id)
);

CREATE INDEX provider_grants_user_created_idx
  ON provider_account_auth_grants(user_id, created_at);

CREATE INDEX provider_grants_runner_session_idx
  ON provider_account_auth_grants(runner_session_id);

CREATE INDEX provider_grants_status_expiry_idx
  ON provider_account_auth_grants(status, expires_at);

CREATE TABLE provider_account_events (
  id TEXT PRIMARY KEY,
  provider_account_id TEXT,
  auth_grant_id TEXT,
  user_id TEXT NOT NULL,
  team_id TEXT,
  thread_id TEXT,
  workroom_id TEXT,
  runner_session_id TEXT,
  kind TEXT NOT NULL CHECK (
    kind IN (
      'login_started',
      'login_connected',
      'login_denied',
      'login_expired',
      'login_failed',
      'account_disconnected',
      'account_health_updated',
      'auth_grant_issued',
      'auth_grant_used',
      'auth_grant_revoked',
      'auth_grant_failed'
    )
  ),
  summary TEXT NOT NULL,
  source_refs_json TEXT NOT NULL,
  evidence_refs_json TEXT NOT NULL,
  target_ref TEXT,
  metadata_json TEXT,
  actor_id TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY (provider_account_id) REFERENCES provider_accounts(id),
  FOREIGN KEY (auth_grant_id) REFERENCES provider_account_auth_grants(id),
  FOREIGN KEY (user_id) REFERENCES users(id),
  FOREIGN KEY (team_id) REFERENCES teams(id)
);

CREATE INDEX provider_account_events_user_created_idx
  ON provider_account_events(user_id, created_at);

CREATE INDEX provider_account_events_target_idx
  ON provider_account_events(target_ref);

CREATE INDEX provider_account_events_account_created_idx
  ON provider_account_events(provider_account_id, created_at);

CREATE TABLE runner_sessions (
  id TEXT PRIMARY KEY,
  runner_id TEXT NOT NULL,
  lane TEXT NOT NULL,
  backend TEXT NOT NULL,
  status TEXT NOT NULL,
  team_id TEXT,
  thread_id TEXT,
  workroom_id TEXT,
  provider_account_ref TEXT,
  active_auth_grant_ref TEXT,
  opencode_server_url TEXT,
  opencode_server_auth_ref TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  started_at TEXT,
  completed_at TEXT,
  failed_at TEXT,
  FOREIGN KEY (team_id) REFERENCES teams(id)
);

CREATE INDEX runner_sessions_thread_created_idx
  ON runner_sessions(thread_id, created_at);

CREATE INDEX runner_sessions_status_idx
  ON runner_sessions(status);
