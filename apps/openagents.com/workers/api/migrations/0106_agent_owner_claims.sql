ALTER TABLE agent_credentials ADD COLUMN expires_at TEXT;

CREATE INDEX IF NOT EXISTS agent_credentials_active_expiry_idx
  ON agent_credentials(status, expires_at);

CREATE TABLE agent_owner_claims (
  id TEXT PRIMARY KEY,
  claim_token_hash TEXT NOT NULL UNIQUE,
  claim_token_prefix TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN ('pending', 'approved', 'rejected', 'expired', 'revoked')
  ),
  display_name TEXT NOT NULL,
  slug TEXT,
  external_id TEXT,
  primary_email TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  owner_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  agent_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  credential_id TEXT REFERENCES agent_credentials(id) ON DELETE SET NULL,
  token_prefix TEXT,
  receipt_ref TEXT NOT NULL UNIQUE,
  requested_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  decided_at TEXT,
  token_issued_at TEXT,
  rejected_reason TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX agent_owner_claims_status_expires_idx
  ON agent_owner_claims(status, expires_at);

CREATE INDEX agent_owner_claims_owner_status_idx
  ON agent_owner_claims(owner_user_id, status);

CREATE INDEX agent_owner_claims_agent_user_idx
  ON agent_owner_claims(agent_user_id);
