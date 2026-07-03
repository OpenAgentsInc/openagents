CREATE TABLE IF NOT EXISTS business_commitment_ledger (
  id TEXT PRIMARY KEY NOT NULL,
  commitment_ref TEXT NOT NULL UNIQUE,
  engagement_ref TEXT NOT NULL,
  owner_ref TEXT NOT NULL,
  vertical_ref TEXT NOT NULL,
  promised_object_ref TEXT NOT NULL,
  commitment_kind TEXT NOT NULL CHECK (
    commitment_kind IN ('deliverable', 'send')
  ),
  due_state TEXT NOT NULL CHECK (
    due_state IN ('due', 'blocked', 'shipped', 'parked')
  ),
  due_at TEXT NOT NULL,
  shipped_at TEXT,
  weekly_review_ref TEXT NOT NULL,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  blocker_refs_json TEXT NOT NULL DEFAULT '[]',
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_business_commitment_ledger_weekly_review
  ON business_commitment_ledger(weekly_review_ref, due_state, due_at);

CREATE INDEX IF NOT EXISTS idx_business_commitment_ledger_engagement
  ON business_commitment_ledger(engagement_ref, due_state, due_at);

INSERT OR IGNORE INTO business_commitment_ledger (
  id,
  commitment_ref,
  engagement_ref,
  owner_ref,
  vertical_ref,
  promised_object_ref,
  commitment_kind,
  due_state,
  due_at,
  shipped_at,
  weekly_review_ref,
  source_refs_json,
  blocker_refs_json,
  evidence_refs_json,
  created_at,
  updated_at
) VALUES
(
  'business_commitment_owed_ecommerce_make_good_20260702',
  'business.commitment.owed.ecommerce_make_good.20260702',
  'business.engagement.opaque.ecommerce_make_good',
  'owner.business.ops',
  'vertical.ecommerce.bitcoin_retail',
  'deliverable.business.ecommerce.inventory_aware_campaign_receipt',
  'deliverable',
  'due',
  '2026-07-09T17:00:00.000Z',
  NULL,
  'business.pipeline_review.weekly',
  '["docs/fable/ROADMAP_BIZ.md#BF-9.1","docs/fable/2026-07-02-business-fulfillment-engine-meditations.md#commitment-ledger"]',
  '[]',
  '[]',
  '2026-07-02T00:00:00.000Z',
  '2026-07-02T00:00:00.000Z'
),
(
  'business_commitment_owed_settlement_make_good_20260702',
  'business.commitment.owed.settlement_make_good.20260702',
  'business.engagement.opaque.settlement_make_good',
  'owner.business.ops',
  'vertical.settlement_infrastructure',
  'deliverable.business.settlement.accepted_outcome_escrow_demo_receipt',
  'deliverable',
  'due',
  '2026-07-09T17:00:00.000Z',
  NULL,
  'business.pipeline_review.weekly',
  '["docs/fable/ROADMAP_BIZ.md#BF-9.1","docs/fable/2026-07-02-business-fulfillment-engine-meditations.md#commitment-ledger"]',
  '[]',
  '[]',
  '2026-07-02T00:00:00.000Z',
  '2026-07-02T00:00:00.000Z'
);
