CREATE TABLE IF NOT EXISTS revenue_event_provenance (
  event_ref TEXT PRIMARY KEY,
  evidence_bundle_ref TEXT NOT NULL UNIQUE,
  idempotency_key TEXT NOT NULL UNIQUE,
  product_ref TEXT NOT NULL CHECK (product_ref IN ('khala_code', 'qa_swarm')),
  revenue_surface_ref TEXT NOT NULL,
  receipt_ref TEXT NOT NULL,
  ledger_table TEXT NOT NULL CHECK (
    ledger_table IN (
      'khala_code_paid_plan_payment_intents',
      'qa_swarm_first_engagements'
    )
  ),
  ledger_row_ref TEXT NOT NULL,
  demand_provenance TEXT NOT NULL CHECK (demand_provenance IN ('internal', 'external')),
  payment_state TEXT NOT NULL CHECK (
    payment_state IN (
      'requires_payment',
      'payment_evidence_recorded',
      'fulfilled',
      'settled'
    )
  ),
  amount_cents INTEGER CHECK (amount_cents IS NULL OR amount_cents >= 0),
  amount_sats INTEGER CHECK (amount_sats IS NULL OR amount_sats >= 0),
  public_evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json TEXT NOT NULL DEFAULT '[]',
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  recorded_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (amount_cents IS NOT NULL OR amount_sats IS NOT NULL)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_revenue_event_provenance_product_receipt
  ON revenue_event_provenance (product_ref, receipt_ref);

CREATE INDEX IF NOT EXISTS idx_revenue_event_provenance_recorded_at
  ON revenue_event_provenance (recorded_at);

CREATE INDEX IF NOT EXISTS idx_revenue_event_provenance_product_demand
  ON revenue_event_provenance (
    product_ref,
    demand_provenance,
    payment_state,
    recorded_at
  );
