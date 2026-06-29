ALTER TABLE provider_accounts
  ADD COLUMN last_sanity_check_at TEXT;

ALTER TABLE provider_accounts
  ADD COLUMN last_sanity_check_result TEXT CHECK (
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
  );

CREATE TABLE IF NOT EXISTS provider_account_sanity_checks (
  id TEXT PRIMARY KEY NOT NULL,
  provider_account_id TEXT NOT NULL REFERENCES provider_accounts(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  team_id TEXT REFERENCES teams(id) ON DELETE SET NULL,
  provider TEXT NOT NULL CHECK (provider IN ('chatgpt_codex')),
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

CREATE INDEX IF NOT EXISTS provider_account_sanity_checks_account_created_idx
  ON provider_account_sanity_checks(provider_account_id, created_at DESC);

CREATE INDEX IF NOT EXISTS provider_account_sanity_checks_result_created_idx
  ON provider_account_sanity_checks(classification, created_at DESC);
