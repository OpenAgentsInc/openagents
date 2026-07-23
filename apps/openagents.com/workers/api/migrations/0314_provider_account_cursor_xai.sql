-- #9193: admit Cursor and xAI Grok as provider-account principals for exact,
-- owner-scoped Agent Computer runtime-secret grants. Raw keys remain worker
-- secrets. SQLite cannot alter CHECK constraints, so rebuild only the three
-- provider-bearing tables used by managed dispatch.

CREATE TABLE provider_accounts_0314_data AS SELECT * FROM provider_accounts;
CREATE TABLE provider_account_auth_grants_0314_data AS
  SELECT * FROM provider_account_auth_grants;
CREATE TABLE provider_account_leases_0314_data AS
  SELECT * FROM provider_account_leases;

DROP TABLE provider_account_leases;
DROP TABLE provider_account_auth_grants;
DROP TABLE provider_accounts;

CREATE TABLE provider_accounts (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  team_id TEXT,
  provider TEXT NOT NULL CHECK (
    provider IN (
      'chatgpt_codex',
      'anthropic_claude',
      'google_gemini',
      'cursor',
      'xai_grok'
    )
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

INSERT INTO provider_accounts SELECT * FROM provider_accounts_0314_data;
DROP TABLE provider_accounts_0314_data;

CREATE TABLE provider_account_auth_grants (
  id TEXT PRIMARY KEY,
  provider_account_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  team_id TEXT,
  thread_id TEXT,
  workroom_id TEXT,
  runner_session_id TEXT,
  provider TEXT NOT NULL CHECK (
    provider IN (
      'chatgpt_codex',
      'anthropic_claude',
      'google_gemini',
      'cursor',
      'xai_grok'
    )
  ),
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

INSERT INTO provider_account_auth_grants
  SELECT * FROM provider_account_auth_grants_0314_data;
DROP TABLE provider_account_auth_grants_0314_data;

CREATE TABLE provider_account_leases (
  id TEXT PRIMARY KEY NOT NULL,
  lease_ref TEXT NOT NULL UNIQUE,
  provider_account_id TEXT NOT NULL REFERENCES provider_accounts(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  team_id TEXT REFERENCES teams(id) ON DELETE SET NULL,
  provider TEXT NOT NULL CHECK (
    provider IN (
      'chatgpt_codex',
      'anthropic_claude',
      'google_gemini',
      'cursor',
      'xai_grok'
    )
  ),
  provider_account_ref TEXT NOT NULL,
  requested_action TEXT NOT NULL,
  run_id TEXT,
  assignment_id TEXT,
  selected_by_policy_version TEXT NOT NULL,
  selection_reason TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN ('active', 'expired', 'released', 'succeeded', 'failed')
  ),
  started_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  released_at TEXT,
  terminal_outcome TEXT,
  metadata_json TEXT,
  order_id TEXT,
  selected_by_actor TEXT,
  last_touched_at TEXT,
  failure_class TEXT
);

INSERT INTO provider_account_leases
  SELECT * FROM provider_account_leases_0314_data;
DROP TABLE provider_account_leases_0314_data;

CREATE INDEX provider_accounts_user_provider_idx
  ON provider_accounts(user_id, provider);
CREATE INDEX provider_accounts_team_provider_idx
  ON provider_accounts(team_id, provider);
CREATE INDEX provider_accounts_status_health_idx
  ON provider_accounts(status, health);
CREATE INDEX provider_grants_user_created_idx
  ON provider_account_auth_grants(user_id, created_at);
CREATE INDEX provider_grants_runner_session_idx
  ON provider_account_auth_grants(runner_session_id);
CREATE INDEX provider_grants_status_expiry_idx
  ON provider_account_auth_grants(status, expires_at);
CREATE INDEX provider_leases_active_idx
  ON provider_account_leases(provider_account_id, status, expires_at);
CREATE INDEX provider_leases_user_idx
  ON provider_account_leases(user_id, started_at);
CREATE INDEX provider_leases_order_idx
  ON provider_account_leases(order_id, started_at);
