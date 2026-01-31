-- Identity tokens for "Sign in with Moltbook" (developers flow). One-time use; 1h expiry.

CREATE TABLE IF NOT EXISTS social_identity_tokens (
  id TEXT PRIMARY KEY,
  agent_name TEXT NOT NULL,
  exp_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_social_identity_tokens_exp_at ON social_identity_tokens(exp_at);
