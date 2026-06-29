CREATE TABLE IF NOT EXISTS agent_claim_reward_ledger (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  campaign_ref TEXT NOT NULL,
  agent_claim_ref TEXT NOT NULL,
  owner_ref TEXT NOT NULL,
  x_account_ref TEXT NOT NULL,
  tweet_ref TEXT NOT NULL,
  state TEXT NOT NULL CHECK (
    state IN (
      'pending',
      'verified',
      'approved',
      'payout_intent_created',
      'dispatched',
      'settled',
      'rejected',
      'reversed',
      'expired'
    )
  ),
  amount_sats INTEGER NOT NULL CHECK (amount_sats = 1000),
  destination_kind TEXT NOT NULL CHECK (
    destination_kind IN (
      'lightning_address',
      'lnurl',
      'bolt12',
      'bolt11_invoice',
      'unknown'
    )
  ),
  redacted_destination_ref TEXT,
  payout_intent_ref TEXT,
  dispatch_attempt_ref TEXT,
  settlement_ref TEXT,
  rejection_reason TEXT,
  policy_refs_json TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS agent_claim_reward_campaign_x_account_idx
  ON agent_claim_reward_ledger(campaign_ref, x_account_ref)
  WHERE state NOT IN ('rejected', 'reversed');

CREATE UNIQUE INDEX IF NOT EXISTS agent_claim_reward_campaign_owner_idx
  ON agent_claim_reward_ledger(campaign_ref, owner_ref)
  WHERE state NOT IN ('rejected', 'reversed');

CREATE UNIQUE INDEX IF NOT EXISTS agent_claim_reward_campaign_claim_idx
  ON agent_claim_reward_ledger(campaign_ref, agent_claim_ref)
  WHERE state NOT IN ('rejected', 'reversed');

CREATE INDEX IF NOT EXISTS agent_claim_reward_state_updated_idx
  ON agent_claim_reward_ledger(state, updated_at DESC);
