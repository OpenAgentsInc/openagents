-- BF-9.2 sales pipeline queue + BF-9.1 commitment linkage (#8263).
--
-- The queue records only opaque refs, coarse vertical/source descriptors, stage
-- receipts, quoted amount bands, and operator ownership labels. Prospect names,
-- emails, domains, raw CRM payloads, call notes, and private payment material do
-- not belong here.

CREATE TABLE IF NOT EXISTS business_pipeline_rows (
  pipeline_ref TEXT PRIMARY KEY NOT NULL,
  vertical TEXT NOT NULL,
  source_ref TEXT NOT NULL,
  stage TEXT NOT NULL CHECK (
    stage IN (
      'intake_received',
      'scope_scheduled',
      'scope_completed',
      'receipt_plan_sent',
      'closed_won',
      'closed_lost',
      'quick_win_started'
    )
  ),
  quoted_min_usd_cents INTEGER NOT NULL DEFAULT 0 CHECK (quoted_min_usd_cents >= 0),
  quoted_max_usd_cents INTEGER NOT NULL DEFAULT 0 CHECK (
    quoted_max_usd_cents >= quoted_min_usd_cents
  ),
  quoted_band_label TEXT NOT NULL DEFAULT 'unquoted',
  owner_role TEXT NOT NULL CHECK (
    owner_role IN ('operator', 'reviewer', 'fulfillment_agent', 'owner')
  ),
  next_action_due_at TEXT,
  blocker_ref TEXT,
  receipt_refs_json TEXT NOT NULL DEFAULT '[]',
  partner_route_flag INTEGER NOT NULL DEFAULT 0 CHECK (partner_route_flag IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  stage_updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_business_pipeline_rows_stage_due
  ON business_pipeline_rows(stage, next_action_due_at, updated_at);

CREATE INDEX IF NOT EXISTS idx_business_pipeline_rows_source
  ON business_pipeline_rows(source_ref, stage, updated_at);

CREATE INDEX IF NOT EXISTS idx_business_pipeline_rows_partner_route
  ON business_pipeline_rows(partner_route_flag, stage, updated_at);

ALTER TABLE business_commitment_ledger
  ADD COLUMN pipeline_ref TEXT;

CREATE INDEX IF NOT EXISTS idx_business_commitment_ledger_pipeline
  ON business_commitment_ledger(pipeline_ref, due_state, due_at);
