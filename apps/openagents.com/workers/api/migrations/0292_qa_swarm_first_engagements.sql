CREATE TABLE IF NOT EXISTS qa_swarm_first_engagements (
  receipt_ref TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  package_kind TEXT NOT NULL CHECK (package_kind IN ('swarm_audit')),
  payment_path TEXT NOT NULL CHECK (
    payment_path IN (
      'operator_sales_deposit_invoice',
      'checkout_kickoff_receipt'
    )
  ),
  business_signup_request_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  committed_amount_cents INTEGER NOT NULL CHECK (
    committed_amount_cents >= 100000
    AND committed_amount_cents <= 500000
  ),
  intake_receipt_ref TEXT NOT NULL,
  checkout_or_deposit_receipt_ref TEXT NOT NULL,
  target_adapter_review_ref TEXT NOT NULL,
  package_contract_ref TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  service_promise_contract_id TEXT NOT NULL,
  commitment_ref TEXT NOT NULL UNIQUE,
  first_report_due_at TEXT NOT NULL,
  recorded_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_qa_swarm_first_engagements_recorded_at
  ON qa_swarm_first_engagements (recorded_at);

CREATE INDEX IF NOT EXISTS idx_qa_swarm_first_engagements_signup
  ON qa_swarm_first_engagements (
    business_signup_request_id,
    recorded_at
  );
