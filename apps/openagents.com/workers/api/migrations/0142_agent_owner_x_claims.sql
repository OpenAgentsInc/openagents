CREATE TABLE IF NOT EXISTS agent_owner_x_claim_challenges (
  id TEXT PRIMARY KEY,
  agent_claim_id TEXT NOT NULL REFERENCES agent_owner_claims(id) ON DELETE CASCADE,
  owner_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  agent_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  x_account_ref TEXT NOT NULL,
  x_handle TEXT NOT NULL,
  nonce TEXT NOT NULL UNIQUE,
  required_text TEXT NOT NULL,
  required_url TEXT NOT NULL,
  state TEXT NOT NULL CHECK (
    state IN (
      'pending_owner_session',
      'pending_x_connection',
      'pending_tweet',
      'verified',
      'approved',
      'rejected',
      'expired',
      'revoked'
    )
  ),
  receipt_ref TEXT NOT NULL UNIQUE,
  tweet_ref TEXT,
  tweet_url TEXT,
  policy_refs_json TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json TEXT NOT NULL DEFAULT '[]',
  rejected_reason TEXT,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  verified_at TEXT,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS agent_owner_x_claim_active_claim_idx
  ON agent_owner_x_claim_challenges(agent_claim_id)
  WHERE state IN (
    'pending_owner_session',
    'pending_x_connection',
    'pending_tweet',
    'verified',
    'approved'
  );

CREATE UNIQUE INDEX IF NOT EXISTS agent_owner_x_claim_verified_account_idx
  ON agent_owner_x_claim_challenges(x_account_ref)
  WHERE state IN ('verified', 'approved');

CREATE UNIQUE INDEX IF NOT EXISTS agent_owner_x_claim_verified_tweet_idx
  ON agent_owner_x_claim_challenges(tweet_ref)
  WHERE state IN ('verified', 'approved') AND tweet_ref IS NOT NULL;

CREATE INDEX IF NOT EXISTS agent_owner_x_claim_owner_state_idx
  ON agent_owner_x_claim_challenges(owner_user_id, state, updated_at DESC);
