-- LG-9 partner-org routing bookkeeping (#8270).
--
-- This is not the BF-8.5 marketplace implementation. It records only opaque
-- refs for manual, operator-approved partner routing on existing pipeline rows.
-- No automation, settlement path, payout copy, public marketplace copy, or
-- customer-identifying/private peer data belongs in this table.

ALTER TABLE business_pipeline_rows
  ADD COLUMN partner_route_state TEXT NOT NULL DEFAULT 'none' CHECK (
    partner_route_state IN ('none', 'candidate', 'offered', 'accepted', 'declined')
  );

ALTER TABLE business_pipeline_rows
  ADD COLUMN partner_peer_ref TEXT;

ALTER TABLE business_pipeline_rows
  ADD COLUMN partner_approval_receipt_ref TEXT;

ALTER TABLE business_pipeline_rows
  ADD COLUMN partner_offer_ref TEXT;

ALTER TABLE business_pipeline_rows
  ADD COLUMN partner_scope_summary_ref TEXT;

ALTER TABLE business_pipeline_rows
  ADD COLUMN partner_due_window_ref TEXT;

ALTER TABLE business_pipeline_rows
  ADD COLUMN partner_budget_range_ref TEXT;

ALTER TABLE business_pipeline_rows
  ADD COLUMN partner_privacy_tier_ref TEXT;

ALTER TABLE business_pipeline_rows
  ADD COLUMN partner_route_updated_at TEXT;

UPDATE business_pipeline_rows
   SET partner_route_state = 'candidate',
       partner_route_updated_at = COALESCE(updated_at, stage_updated_at, created_at)
 WHERE partner_route_flag = 1
   AND partner_route_state = 'none';

CREATE INDEX IF NOT EXISTS idx_business_pipeline_rows_partner_route_state
  ON business_pipeline_rows(partner_route_state, updated_at);
