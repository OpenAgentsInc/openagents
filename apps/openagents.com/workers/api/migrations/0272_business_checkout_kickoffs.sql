CREATE TABLE IF NOT EXISTS business_checkout_kickoffs (
  checkout_session_id TEXT PRIMARY KEY NOT NULL
    REFERENCES stripe_checkout_sessions(session_id) ON DELETE CASCADE,
  business_signup_request_id TEXT NOT NULL
    REFERENCES business_signup_requests(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  total_amount_cents INTEGER NOT NULL CHECK (total_amount_cents >= 0),
  setup_fee_cents INTEGER NOT NULL CHECK (setup_fee_cents >= 0),
  credit_grant_cents INTEGER NOT NULL CHECK (credit_grant_cents >= 0),
  workspace_id TEXT NOT NULL REFERENCES prefilled_workspaces(id) ON DELETE CASCADE,
  service_promise_contract_id TEXT NOT NULL
    REFERENCES omni_accepted_outcome_contracts(id) ON DELETE CASCADE,
  public_receipt_ref TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (setup_fee_cents + credit_grant_cents = total_amount_cents)
);

CREATE INDEX IF NOT EXISTS business_checkout_kickoffs_signup_idx
  ON business_checkout_kickoffs(business_signup_request_id, created_at DESC);

CREATE INDEX IF NOT EXISTS business_checkout_kickoffs_user_idx
  ON business_checkout_kickoffs(user_id, created_at DESC);
