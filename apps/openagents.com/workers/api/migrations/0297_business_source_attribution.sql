-- LG-6 source attribution for /business intake and pipeline (#8267).
--
-- Store only bounded, public-safe sourceRef tokens such as `direct`,
-- `apollo_agent_readiness_a`, `partner_expansion`, and `affiliate_<code>`.
-- Raw UTMs, URLs, contact details, and provider payloads do not belong here.

ALTER TABLE business_signup_requests
  ADD COLUMN source_ref TEXT NOT NULL DEFAULT 'direct';

ALTER TABLE business_signup_requests
  ADD COLUMN linked_pipeline_ref TEXT;

CREATE INDEX IF NOT EXISTS business_signup_requests_source_ref_idx
  ON business_signup_requests(source_ref, created_at DESC);

CREATE INDEX IF NOT EXISTS business_signup_requests_linked_pipeline_ref_idx
  ON business_signup_requests(linked_pipeline_ref)
  WHERE linked_pipeline_ref IS NOT NULL;

ALTER TABLE business_pipeline_rows
  ADD COLUMN business_signup_request_id TEXT;

CREATE INDEX IF NOT EXISTS idx_business_pipeline_rows_signup
  ON business_pipeline_rows(business_signup_request_id)
  WHERE business_signup_request_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS business_funnel_events_source_ref_stage_idx
  ON business_funnel_events(source_ref, stage, occurred_at DESC);
