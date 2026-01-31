-- Social API claim link support.

CREATE TABLE IF NOT EXISTS social_claims (
  claim_token TEXT PRIMARY KEY,
  api_key TEXT NOT NULL,
  agent_name TEXT NOT NULL,
  verification_code TEXT,
  created_at TEXT NOT NULL,
  claimed_at TEXT,
  status TEXT NOT NULL
);
