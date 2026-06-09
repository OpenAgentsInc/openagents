CREATE TABLE IF NOT EXISTS referral_workflow_events (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  event_kind TEXT NOT NULL CHECK (
    event_kind IN (
      'paid_usage',
      'site_checkout',
      'l402_redemption',
      'accepted_outcome',
      'refund',
      'reversal',
      'eligibility_hold',
      'dispute_hold',
      'operator_adjustment'
    )
  ),
  referral_attribution_id TEXT NOT NULL
    REFERENCES referral_attributions(id) ON DELETE RESTRICT,
  referral_source_id TEXT NOT NULL
    REFERENCES site_referral_sources(id) ON DELETE RESTRICT,
  referral_invite_id TEXT REFERENCES referral_invites(id) ON DELETE SET NULL,
  public_source_ref TEXT NOT NULL,
  public_invite_ref TEXT,
  software_order_id TEXT REFERENCES software_orders(id) ON DELETE SET NULL,
  site_id TEXT REFERENCES site_projects(id) ON DELETE SET NULL,
  site_version_id TEXT REFERENCES site_versions(id) ON DELETE SET NULL,
  product_id TEXT,
  paid_action_id TEXT,
  payment_event_id TEXT,
  payment_evidence_ref TEXT,
  entitlement_ref TEXT,
  accepted_work_ref TEXT,
  related_event_id TEXT
    REFERENCES referral_workflow_events(id) ON DELETE SET NULL,
  public_receipt_ref TEXT NOT NULL,
  policy_state TEXT NOT NULL CHECK (
    policy_state IN (
      'recorded',
      'eligible',
      'held',
      'disputed',
      'refunded',
      'reversed',
      'ignored'
    )
  ),
  amount NUMERIC NOT NULL DEFAULT 0 CHECK (amount >= 0),
  asset TEXT NOT NULL CHECK (asset IN ('none', 'credits', 'sats', 'usd')),
  metadata_json TEXT NOT NULL DEFAULT '{}',
  occurred_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT,
  CHECK (
    event_kind NOT IN ('refund', 'reversal') OR related_event_id IS NOT NULL
  )
);

CREATE INDEX IF NOT EXISTS idx_referral_workflow_events_attribution
  ON referral_workflow_events(referral_attribution_id, occurred_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_referral_workflow_events_source
  ON referral_workflow_events(referral_source_id, occurred_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_referral_workflow_events_order
  ON referral_workflow_events(software_order_id, occurred_at DESC)
  WHERE software_order_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_referral_workflow_events_site
  ON referral_workflow_events(site_id, occurred_at DESC)
  WHERE site_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_referral_workflow_events_related
  ON referral_workflow_events(related_event_id, occurred_at DESC)
  WHERE related_event_id IS NOT NULL AND archived_at IS NULL;
