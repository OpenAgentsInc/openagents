CREATE TABLE provider_account_token_custody (
  provider_account_ref TEXT PRIMARY KEY,
  owner_user_id TEXT NOT NULL,
  provider TEXT NOT NULL CHECK (provider = 'chatgpt_codex'),
  secret_ref TEXT NOT NULL,
  refresh_ciphertext_b64 TEXT NOT NULL,
  refresh_iv_b64 TEXT NOT NULL,
  refresh_key_id TEXT NOT NULL,
  access_ciphertext_b64 TEXT NOT NULL,
  access_iv_b64 TEXT NOT NULL,
  access_key_id TEXT NOT NULL,
  access_expires_at TEXT NOT NULL,
  account_id TEXT,
  id_token_ciphertext_b64 TEXT,
  id_token_iv_b64 TEXT,
  id_token_key_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  last_refreshed_at TEXT
);

CREATE INDEX provider_account_token_custody_owner_idx
  ON provider_account_token_custody(owner_user_id, provider_account_ref);

CREATE TABLE provider_account_token_custody_audit (
  id TEXT PRIMARY KEY,
  provider_account_ref TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  provider TEXT NOT NULL CHECK (provider = 'chatgpt_codex'),
  event_kind TEXT NOT NULL CHECK (
    event_kind IN (
      'auth_stored',
      'access_issued',
      'auth_deleted',
      'refresh_succeeded',
      'refresh_failed'
    )
  ),
  status TEXT NOT NULL CHECK (status IN ('succeeded', 'failed')),
  actor_ref TEXT,
  source_ref TEXT,
  error_tag TEXT,
  error_message TEXT,
  metadata_json TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX provider_account_token_custody_audit_owner_idx
  ON provider_account_token_custody_audit(owner_user_id, created_at);

CREATE INDEX provider_account_token_custody_audit_account_idx
  ON provider_account_token_custody_audit(provider_account_ref, created_at);
