CREATE TABLE agent_scoped_grant_receipts (
  id TEXT PRIMARY KEY,
  receipt_ref TEXT NOT NULL UNIQUE,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  owner_user_id TEXT NOT NULL,
  agent_user_id TEXT NOT NULL,
  agent_credential_id TEXT,
  grant_id TEXT NOT NULL,
  grant_kind TEXT NOT NULL CHECK (grant_kind IN ('customer_orders', 'agent_sites')),
  action TEXT NOT NULL CHECK (action IN ('grant', 'revoke')),
  status TEXT NOT NULL CHECK (status IN ('applied', 'idempotent_replay')),
  scopes_json TEXT NOT NULL,
  target_json TEXT NOT NULL,
  expires_at TEXT,
  reason TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX agent_scoped_grant_receipts_owner_created_idx
  ON agent_scoped_grant_receipts(owner_user_id, created_at);

CREATE INDEX agent_scoped_grant_receipts_agent_created_idx
  ON agent_scoped_grant_receipts(agent_user_id, created_at);
