DROP TABLE IF EXISTS provider_account_connection_attempts_0237_data;

CREATE TABLE provider_account_connection_attempts_0237_data AS
  SELECT * FROM provider_account_connection_attempts;

DROP TABLE provider_account_connection_attempts;

CREATE TABLE provider_account_connection_attempts (
  id TEXT PRIMARY KEY,
  provider_account_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  team_id TEXT,
  provider TEXT NOT NULL CHECK (
    provider IN ('chatgpt_codex', 'anthropic_claude', 'google_gemini')
  ),
  method TEXT NOT NULL CHECK (
    method IN (
      'chatgpt_device_code',
      'codex_device_auth',
      'provider_api_key'
    )
  ),
  source TEXT NOT NULL CHECK (
    source IN (
      'shc_broker',
      'worker_device_code',
      'manual_placeholder',
      'browser_api_key',
      'pylon_local_codex_auth'
    )
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

INSERT INTO provider_account_connection_attempts (
  id,
  provider_account_id,
  user_id,
  team_id,
  provider,
  method,
  source,
  login_ref,
  verification_url,
  user_code,
  status,
  expires_at,
  completed_at,
  failed_at,
  metadata_json,
  created_at,
  updated_at
)
SELECT
  id,
  provider_account_id,
  user_id,
  team_id,
  provider,
  method,
  source,
  login_ref,
  verification_url,
  user_code,
  status,
  expires_at,
  completed_at,
  failed_at,
  metadata_json,
  created_at,
  updated_at
FROM provider_account_connection_attempts_0237_data;

DROP TABLE provider_account_connection_attempts_0237_data;

CREATE INDEX provider_connection_attempts_user_created_idx
  ON provider_account_connection_attempts(user_id, created_at);

CREATE INDEX provider_connection_attempts_provider_account_idx
  ON provider_account_connection_attempts(provider_account_id, created_at);

CREATE INDEX provider_connection_attempts_status_expiry_idx
  ON provider_account_connection_attempts(status, expires_at);
