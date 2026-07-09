-- CX-5 (#8549): Claude account parity — allow a Pylon local-auth/import
-- connection for `anthropic_claude` provider accounts, mirroring the
-- `codex_device_auth` / `pylon_local_codex_auth` shape #8237 added for
-- Codex. SQLite CHECK constraints require a table rebuild; the column sets
-- below are copied exactly from the last full rebuilds (0173 for
-- `provider_accounts`, 0237 for `provider_account_connection_attempts`) with
-- only the new enum members added.

DROP TABLE IF EXISTS provider_accounts_0313_new;
DROP TABLE IF EXISTS provider_account_connection_attempts_0313_data;

CREATE TABLE provider_accounts_0313_new (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  team_id TEXT,
  provider TEXT NOT NULL CHECK (
    provider IN ('chatgpt_codex', 'anthropic_claude', 'google_gemini')
  ),
  auth_mode TEXT NOT NULL CHECK (
    auth_mode IN (
      'chatgpt_device_code',
      'codex_device_auth',
      'manual_secret_ref',
      'api_key',
      'claude_local_auth'
    )
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
  last_sanity_check_at TEXT,
  last_sanity_check_result TEXT CHECK (
    last_sanity_check_result IS NULL
    OR last_sanity_check_result IN (
      'healthy',
      'requires_reauth',
      'low_credit',
      'rate_limited',
      'quota_exhausted',
      'provider_outage',
      'grant_resolution_failed',
      'launch_probe_failed',
      'unknown_failure'
    )
  ),
  operator_priority INTEGER NOT NULL DEFAULT 100,
  cooldown_until TEXT,
  low_credit_flag INTEGER NOT NULL DEFAULT 0,
  recent_failure_class TEXT,
  last_selected_at TEXT,
  operator_label TEXT,
  lease_limit INTEGER NOT NULL DEFAULT 1,
  last_parallel_probe_at TEXT,
  last_parallel_probe_result TEXT,
  last_successful_launch_at TEXT,
  last_failed_launch_at TEXT,
  reauth_required_reason TEXT,
  operator_note TEXT,
  refill_note TEXT,
  FOREIGN KEY (user_id) REFERENCES users(id),
  FOREIGN KEY (team_id) REFERENCES teams(id)
);

INSERT INTO provider_accounts_0313_new (
  id,
  user_id,
  team_id,
  provider,
  auth_mode,
  status,
  health,
  provider_account_ref,
  secret_ref,
  account_label,
  plan_type,
  connected_at,
  disconnected_at,
  denied_at,
  last_status_at,
  metadata_json,
  created_at,
  updated_at,
  deleted_at,
  last_sanity_check_at,
  last_sanity_check_result,
  operator_priority,
  cooldown_until,
  low_credit_flag,
  recent_failure_class,
  last_selected_at,
  operator_label,
  lease_limit,
  last_parallel_probe_at,
  last_parallel_probe_result,
  last_successful_launch_at,
  last_failed_launch_at,
  reauth_required_reason,
  operator_note,
  refill_note
)
SELECT
  id,
  user_id,
  team_id,
  provider,
  auth_mode,
  status,
  health,
  provider_account_ref,
  secret_ref,
  account_label,
  plan_type,
  connected_at,
  disconnected_at,
  denied_at,
  last_status_at,
  metadata_json,
  created_at,
  updated_at,
  deleted_at,
  last_sanity_check_at,
  last_sanity_check_result,
  operator_priority,
  cooldown_until,
  low_credit_flag,
  recent_failure_class,
  last_selected_at,
  operator_label,
  lease_limit,
  last_parallel_probe_at,
  last_parallel_probe_result,
  last_successful_launch_at,
  last_failed_launch_at,
  reauth_required_reason,
  operator_note,
  refill_note
FROM provider_accounts;

DROP TABLE provider_accounts;

ALTER TABLE provider_accounts_0313_new RENAME TO provider_accounts;

CREATE INDEX provider_accounts_user_provider_idx
  ON provider_accounts(user_id, provider);

CREATE INDEX provider_accounts_team_provider_idx
  ON provider_accounts(team_id, provider);

CREATE INDEX provider_accounts_status_health_idx
  ON provider_accounts(status, health);

CREATE TABLE provider_account_connection_attempts_0313_data AS
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
      'provider_api_key',
      'claude_local_auth'
    )
  ),
  source TEXT NOT NULL CHECK (
    source IN (
      'shc_broker',
      'worker_device_code',
      'manual_placeholder',
      'browser_api_key',
      'pylon_local_codex_auth',
      'pylon_local_claude_auth'
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

INSERT INTO provider_account_connection_attempts
SELECT * FROM provider_account_connection_attempts_0313_data;

DROP TABLE provider_account_connection_attempts_0313_data;

CREATE INDEX provider_connection_attempts_user_created_idx
  ON provider_account_connection_attempts(user_id, created_at);

CREATE INDEX provider_connection_attempts_provider_account_idx
  ON provider_account_connection_attempts(provider_account_id, created_at);

CREATE INDEX provider_connection_attempts_status_expiry_idx
  ON provider_account_connection_attempts(status, expires_at);
