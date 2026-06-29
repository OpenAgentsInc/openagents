CREATE TABLE IF NOT EXISTS x_claim_reward_ledger (
  id TEXT PRIMARY KEY,
  challenge_id TEXT NOT NULL UNIQUE REFERENCES agent_owner_x_claim_challenges(id) ON DELETE CASCADE,
  claim_id TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  agent_user_id TEXT,
  x_account_ref TEXT NOT NULL UNIQUE,
  amount_sats INTEGER NOT NULL DEFAULT 1000,
  state TEXT NOT NULL CHECK (
    state IN (
      'eligible',
      'dispatch_requested',
      'dispatched',
      'settled',
      'failed',
      'refused'
    )
  ),
  state_reason_ref TEXT,
  receipt_ref TEXT NOT NULL UNIQUE,
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS x_claim_reward_ledger_state_idx
  ON x_claim_reward_ledger(state, updated_at DESC);

CREATE INDEX IF NOT EXISTS x_claim_reward_ledger_owner_idx
  ON x_claim_reward_ledger(owner_user_id, created_at DESC);
