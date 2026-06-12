DROP TABLE IF EXISTS provider_accounts_0173_new;
DROP TABLE IF EXISTS provider_account_connection_attempts_0173_data;
DROP TABLE IF EXISTS provider_account_auth_grants_0173_data;
DROP TABLE IF EXISTS provider_account_events_0173_data;
DROP TABLE IF EXISTS provider_account_sanity_checks_0173_data;
DROP TABLE IF EXISTS provider_account_parallel_probe_receipts_0173_data;
DROP TABLE IF EXISTS provider_account_leases_0173_data;

CREATE TABLE provider_accounts_0173_new (
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
      'api_key'
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

INSERT INTO provider_accounts_0173_new (
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

CREATE TABLE provider_account_connection_attempts_0173_data AS
  SELECT * FROM provider_account_connection_attempts;

CREATE TABLE provider_account_auth_grants_0173_data AS
  SELECT * FROM provider_account_auth_grants;

CREATE TABLE provider_account_events_0173_data AS
  SELECT * FROM provider_account_events;

CREATE TABLE provider_account_sanity_checks_0173_data AS
  SELECT * FROM provider_account_sanity_checks;

CREATE TABLE provider_account_parallel_probe_receipts_0173_data AS
  SELECT * FROM provider_account_parallel_probe_receipts;

CREATE TABLE provider_account_leases_0173_data AS
  SELECT * FROM provider_account_leases;

DROP TABLE provider_account_events;
DROP TABLE provider_account_sanity_checks;
DROP TABLE provider_account_parallel_probe_receipts;
DROP TABLE provider_account_leases;
DROP TABLE provider_account_connection_attempts;
DROP TABLE provider_account_auth_grants;
DROP TABLE provider_accounts;

ALTER TABLE provider_accounts_0173_new RENAME TO provider_accounts;

CREATE TABLE provider_account_connection_attempts (
  id TEXT PRIMARY KEY,
  provider_account_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  team_id TEXT,
  provider TEXT NOT NULL CHECK (
    provider IN ('chatgpt_codex', 'anthropic_claude', 'google_gemini')
  ),
  method TEXT NOT NULL CHECK (
    method IN ('chatgpt_device_code', 'provider_api_key')
  ),
  source TEXT NOT NULL CHECK (
    source IN (
      'shc_broker',
      'worker_device_code',
      'manual_placeholder',
      'browser_api_key'
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
SELECT * FROM provider_account_connection_attempts_0173_data;

CREATE TABLE provider_account_auth_grants (
  id TEXT PRIMARY KEY,
  provider_account_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  team_id TEXT,
  thread_id TEXT,
  workroom_id TEXT,
  runner_session_id TEXT,
  provider TEXT NOT NULL CHECK (
    provider IN ('chatgpt_codex', 'anthropic_claude', 'google_gemini')
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
SELECT * FROM provider_account_auth_grants_0173_data;

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

INSERT INTO provider_account_events
SELECT * FROM provider_account_events_0173_data;

CREATE TABLE provider_account_sanity_checks (
  id TEXT PRIMARY KEY NOT NULL,
  provider_account_id TEXT NOT NULL REFERENCES provider_accounts(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  team_id TEXT REFERENCES teams(id) ON DELETE SET NULL,
  provider TEXT NOT NULL CHECK (
    provider IN ('chatgpt_codex', 'anthropic_claude', 'google_gemini')
  ),
  provider_account_ref TEXT NOT NULL,
  classification TEXT NOT NULL CHECK (
    classification IN (
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
  summary TEXT NOT NULL,
  grant_ref TEXT,
  created_at TEXT NOT NULL,
  metadata_json TEXT
);

INSERT INTO provider_account_sanity_checks
SELECT * FROM provider_account_sanity_checks_0173_data;

CREATE TABLE provider_account_parallel_probe_receipts (
  id TEXT PRIMARY KEY NOT NULL,
  probe_run_id TEXT NOT NULL,
  probe_id TEXT NOT NULL,
  lease_id TEXT NOT NULL,
  provider_account_id TEXT NOT NULL REFERENCES provider_accounts(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  team_id TEXT REFERENCES teams(id) ON DELETE SET NULL,
  provider_account_ref TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT NOT NULL,
  terminal_status TEXT NOT NULL CHECK (
    terminal_status IN ('passed', 'failed')
  ),
  classification TEXT NOT NULL,
  collision_class TEXT NOT NULL,
  metadata_json TEXT
);

INSERT INTO provider_account_parallel_probe_receipts
SELECT * FROM provider_account_parallel_probe_receipts_0173_data;

CREATE TABLE provider_account_leases (
  id TEXT PRIMARY KEY NOT NULL,
  lease_ref TEXT NOT NULL UNIQUE,
  provider_account_id TEXT NOT NULL REFERENCES provider_accounts(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  team_id TEXT REFERENCES teams(id) ON DELETE SET NULL,
  provider TEXT NOT NULL CHECK (
    provider IN ('chatgpt_codex', 'anthropic_claude', 'google_gemini')
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
SELECT * FROM provider_account_leases_0173_data;

DROP TABLE provider_account_connection_attempts_0173_data;
DROP TABLE provider_account_auth_grants_0173_data;
DROP TABLE provider_account_events_0173_data;
DROP TABLE provider_account_sanity_checks_0173_data;
DROP TABLE provider_account_parallel_probe_receipts_0173_data;
DROP TABLE provider_account_leases_0173_data;

CREATE INDEX provider_accounts_user_provider_idx
  ON provider_accounts(user_id, provider);

CREATE INDEX provider_accounts_team_provider_idx
  ON provider_accounts(team_id, provider);

CREATE INDEX provider_accounts_status_health_idx
  ON provider_accounts(status, health);

CREATE INDEX provider_connection_attempts_user_created_idx
  ON provider_account_connection_attempts(user_id, created_at);

CREATE INDEX provider_connection_attempts_provider_account_idx
  ON provider_account_connection_attempts(provider_account_id, created_at);

CREATE INDEX provider_connection_attempts_status_expiry_idx
  ON provider_account_connection_attempts(status, expires_at);

CREATE INDEX provider_grants_user_created_idx
  ON provider_account_auth_grants(user_id, created_at);

CREATE INDEX provider_grants_runner_session_idx
  ON provider_account_auth_grants(runner_session_id);

CREATE INDEX provider_grants_status_expiry_idx
  ON provider_account_auth_grants(status, expires_at);

CREATE INDEX provider_account_events_user_created_idx
  ON provider_account_events(user_id, created_at);

CREATE INDEX provider_account_events_target_idx
  ON provider_account_events(target_ref);

CREATE INDEX provider_account_events_account_created_idx
  ON provider_account_events(provider_account_id, created_at);

CREATE INDEX provider_account_sanity_checks_account_created_idx
  ON provider_account_sanity_checks(provider_account_id, created_at DESC);

CREATE INDEX provider_account_sanity_checks_result_created_idx
  ON provider_account_sanity_checks(classification, created_at DESC);

CREATE INDEX provider_account_parallel_probe_receipts_run_idx
  ON provider_account_parallel_probe_receipts(probe_run_id, started_at);

CREATE INDEX provider_account_parallel_probe_receipts_account_idx
  ON provider_account_parallel_probe_receipts(provider_account_id, started_at DESC);

CREATE INDEX provider_account_leases_active_idx
  ON provider_account_leases(provider_account_id, status, expires_at);

CREATE INDEX provider_account_leases_user_idx
  ON provider_account_leases(user_id, started_at DESC);

CREATE INDEX provider_account_leases_order_idx
  ON provider_account_leases(order_id, started_at DESC);
