CREATE TABLE IF NOT EXISTS targeted_site_sales_reward_policy_events (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  campaign_id TEXT NOT NULL
    REFERENCES targeted_site_campaigns(id) ON DELETE CASCADE,
  agent_ref TEXT NOT NULL,
  prospect_id TEXT
    REFERENCES targeted_site_prospects(id) ON DELETE SET NULL,
  outcome_kind TEXT NOT NULL CHECK (
    outcome_kind IN (
      'lead_proposed',
      'meeting_accepted',
      'customer_accepted',
      'reward_eligible',
      'payout_intent_created',
      'reward_held',
      'reward_disputed',
      'reward_reversed',
      'refund_recorded',
      'complaint_recorded',
      'settlement_caveat_recorded'
    )
  ),
  policy_state TEXT NOT NULL CHECK (
    policy_state IN (
      'proposed',
      'accepted',
      'held',
      'disputed',
      'reversed',
      'eligible'
    )
  ),
  reward_asset TEXT NOT NULL CHECK (
    reward_asset IN ('credits', 'sats', 'internal_payable')
  ),
  reward_amount INTEGER NOT NULL DEFAULT 0 CHECK (reward_amount >= 0),
  buyer_payment_ref TEXT,
  referral_attribution_ref TEXT,
  accepted_work_ref TEXT,
  payout_intent_ref TEXT,
  settlement_caveat_ref TEXT,
  dispute_ref TEXT,
  public_receipt_ref TEXT NOT NULL,
  related_event_id TEXT
    REFERENCES targeted_site_sales_reward_policy_events(id) ON DELETE SET NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  occurred_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_targeted_site_sales_reward_policy_campaign
  ON targeted_site_sales_reward_policy_events(
    campaign_id,
    agent_ref,
    occurred_at DESC
  )
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_sales_reward_policy_prospect
  ON targeted_site_sales_reward_policy_events(
    prospect_id,
    occurred_at DESC
  )
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_sales_reward_policy_related
  ON targeted_site_sales_reward_policy_events(related_event_id)
  WHERE archived_at IS NULL;
