CREATE TABLE provider_account_token_custody_audit_next (
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

INSERT INTO provider_account_token_custody_audit_next
  (id, provider_account_ref, owner_user_id, provider, event_kind, status,
   actor_ref, source_ref, error_tag, error_message, metadata_json, created_at)
SELECT
  id, provider_account_ref, owner_user_id, provider, event_kind, status,
  actor_ref, source_ref, error_tag, error_message, metadata_json, created_at
FROM provider_account_token_custody_audit;

DROP TABLE provider_account_token_custody_audit;

ALTER TABLE provider_account_token_custody_audit_next
  RENAME TO provider_account_token_custody_audit;

CREATE INDEX provider_account_token_custody_audit_owner_idx
  ON provider_account_token_custody_audit(owner_user_id, created_at);

CREATE INDEX provider_account_token_custody_audit_account_idx
  ON provider_account_token_custody_audit(provider_account_ref, created_at);
