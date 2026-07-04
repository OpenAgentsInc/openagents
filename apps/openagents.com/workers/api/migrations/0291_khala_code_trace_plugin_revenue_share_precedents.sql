CREATE TABLE IF NOT EXISTS khala_code_trace_plugin_revenue_share_precedents (
  receipt_ref TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  consented_trace_receipt_ref TEXT NOT NULL,
  trace_digest_ref TEXT NOT NULL,
  plugin_admission_receipt_ref TEXT NOT NULL,
  plugin_registry_receipt_ref TEXT NOT NULL,
  plugin_ref TEXT NOT NULL,
  plugin_digest_ref TEXT NOT NULL,
  plugin_route_ref TEXT NOT NULL,
  routed_request_ref TEXT NOT NULL,
  usage_event_ref TEXT NOT NULL,
  usage_idempotency_ref TEXT NOT NULL,
  contributor_attribution_ref TEXT NOT NULL,
  gross_revenue_msats INTEGER NOT NULL CHECK (gross_revenue_msats > 0),
  contributor_share_msats INTEGER NOT NULL CHECK (
    contributor_share_msats > 0
    AND contributor_share_msats <= gross_revenue_msats
    AND contributor_share_msats % 1000 = 0
  ),
  amount_envelope_ref TEXT NOT NULL,
  payout_rail TEXT NOT NULL CHECK (payout_rail IN ('spark')),
  payout_receipt_ref TEXT NOT NULL,
  settlement_receipt_ref TEXT NOT NULL,
  recorded_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_khala_code_trace_plugin_revenue_share_recorded_at
  ON khala_code_trace_plugin_revenue_share_precedents (recorded_at);

CREATE INDEX IF NOT EXISTS idx_khala_code_trace_plugin_revenue_share_plugin_ref
  ON khala_code_trace_plugin_revenue_share_precedents (plugin_ref, recorded_at);
