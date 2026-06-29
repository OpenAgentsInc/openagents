CREATE TABLE IF NOT EXISTS site_commerce_payment_events (
  id TEXT PRIMARY KEY,
  site_id TEXT NOT NULL,
  site_version_id TEXT,
  software_order_id TEXT,
  product_id TEXT,
  paid_action_id TEXT,
  customer_ref TEXT,
  referral_source_ref TEXT,
  payment_evidence_ref TEXT,
  entitlement_ref TEXT,
  public_receipt_ref TEXT NOT NULL UNIQUE,
  event_kind TEXT NOT NULL CHECK (
    event_kind IN (
      'signup_attributed',
      'checkout_paid',
      'l402_redeemed',
      'credit_spent',
      'accepted_work_closed',
      'refund_or_reversal'
    )
  ),
  amount NUMERIC NOT NULL CHECK (amount >= 0),
  asset TEXT NOT NULL CHECK (asset IN ('credits', 'sats', 'usd')),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (site_id) REFERENCES site_projects(id) ON DELETE CASCADE,
  FOREIGN KEY (site_version_id) REFERENCES site_versions(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_site_commerce_payment_events_site
  ON site_commerce_payment_events(site_id, created_at);

CREATE INDEX IF NOT EXISTS idx_site_commerce_payment_events_referral
  ON site_commerce_payment_events(referral_source_ref, created_at)
  WHERE referral_source_ref IS NOT NULL;

CREATE TABLE IF NOT EXISTS site_commerce_revenue_share_links (
  id TEXT PRIMARY KEY,
  payment_event_id TEXT NOT NULL,
  accepted_work_ref TEXT,
  requested_contributor_asset TEXT NOT NULL CHECK (
    requested_contributor_asset IN ('credits', 'sats', 'usd')
  ),
  provider_payout_claimed INTEGER NOT NULL DEFAULT 0 CHECK (
    provider_payout_claimed IN (0, 1)
  ),
  nexus_receipt_ref TEXT,
  treasury_receipt_ref TEXT,
  ldk_settlement_receipt_ref TEXT,
  referral_reward_trigger TEXT NOT NULL CHECK (
    referral_reward_trigger IN ('none', 'paid_activity')
  ),
  provider_payout_eligibility_state TEXT NOT NULL CHECK (
    provider_payout_eligibility_state IN (
      'not_eligible',
      'eligible_pending_settlement_refs'
    )
  ),
  withdrawal_posture TEXT NOT NULL CHECK (
    withdrawal_posture IN (
      'bitcoin_withdrawable_after_settlement',
      'internal_credit_only',
      'fiat_or_credit_policy_required'
    )
  ),
  projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (payment_event_id)
    REFERENCES site_commerce_payment_events(id)
    ON DELETE CASCADE,
  CHECK (
    provider_payout_claimed = 0
    OR (
      accepted_work_ref IS NOT NULL
      AND nexus_receipt_ref IS NOT NULL
      AND treasury_receipt_ref IS NOT NULL
      AND ldk_settlement_receipt_ref IS NOT NULL
    )
  ),
  CHECK (
    NOT (
      requested_contributor_asset = 'sats'
      AND withdrawal_posture = 'internal_credit_only'
    )
  ),
  CHECK (
    NOT (
      requested_contributor_asset = 'credits'
      AND withdrawal_posture = 'bitcoin_withdrawable_after_settlement'
    )
  )
);

CREATE INDEX IF NOT EXISTS idx_site_commerce_revenue_share_links_event
  ON site_commerce_revenue_share_links(payment_event_id);

CREATE INDEX IF NOT EXISTS idx_site_commerce_revenue_share_links_accepted_work
  ON site_commerce_revenue_share_links(accepted_work_ref)
  WHERE accepted_work_ref IS NOT NULL;
