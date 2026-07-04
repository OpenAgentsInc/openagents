CREATE TABLE IF NOT EXISTS khala_code_outside_user_run_receipts (
  receipt_ref TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  app_version TEXT NOT NULL,
  platform TEXT NOT NULL CHECK (platform IN ('darwin', 'linux', 'win32', 'other')),
  arch TEXT NOT NULL CHECK (arch IN ('arm64', 'x64', 'other')),
  distribution_channel TEXT NOT NULL CHECK (distribution_channel IN ('desktop_dmg', 'npm_cli', 'source_build', 'unknown')),
  codex_cli_state TEXT NOT NULL CHECK (codex_cli_state IN ('ready', 'missing', 'unknown')),
  codex_auth_state TEXT NOT NULL CHECK (codex_auth_state IN ('ready', 'credentials_missing', 'invalid', 'error', 'unknown')),
  pylon_state TEXT NOT NULL CHECK (pylon_state IN ('ready', 'unavailable', 'not_configured', 'unknown')),
  submitted_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_khala_code_outside_user_run_receipts_submitted_at
  ON khala_code_outside_user_run_receipts (submitted_at);
