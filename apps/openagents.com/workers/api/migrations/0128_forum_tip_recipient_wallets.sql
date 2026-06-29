CREATE TABLE IF NOT EXISTS forum_tip_recipient_wallets (
  id TEXT PRIMARY KEY NOT NULL,
  actor_ref TEXT NOT NULL UNIQUE,
  provider_class TEXT NOT NULL CHECK (
    provider_class IN ('mdk_agent_wallet', 'hosted_mdk', 'external_lightning')
  ),
  wallet_ref TEXT NOT NULL,
  receive_capability_ref TEXT NOT NULL,
  payout_target_approval_ref TEXT,
  readiness_refs_json TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json TEXT NOT NULL DEFAULT '[]',
  custody_policy_refs_json TEXT NOT NULL DEFAULT '[]',
  claim_policy_refs_json TEXT NOT NULL DEFAULT '[]',
  source_ref TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('ready', 'disabled', 'blocked')),
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  disabled_at TEXT,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_forum_tip_recipient_wallets_state_updated
  ON forum_tip_recipient_wallets(state, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_forum_tip_recipient_wallets_actor_state
  ON forum_tip_recipient_wallets(actor_ref, state);
