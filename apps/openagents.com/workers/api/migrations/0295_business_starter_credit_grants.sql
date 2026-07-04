-- LG-3 receipted starter-credit grants (#8264).
--
-- The credit itself is still the existing USD-origin credit ledger:
-- `pay_ins.pay_in_type = 'usd_credit_grant'` plus
-- `agent_balances.usd_credit_msat`, so a starter credit is inference-/
-- engagement-spendable but never Bitcoin-withdrawable. This table records the
-- sales instrument metadata, hard-cap values, opportunity binding, and
-- redemption receipt links without storing prospect names, emails, domains, raw
-- CRM payloads, or payment material.

CREATE TABLE IF NOT EXISTS business_starter_credit_grants (
  grant_ref TEXT PRIMARY KEY NOT NULL,
  pipeline_ref TEXT NOT NULL REFERENCES business_pipeline_rows(pipeline_ref),
  account_ref TEXT NOT NULL,
  engagement_ref TEXT NOT NULL,
  attribution_kind TEXT NOT NULL DEFAULT 'sales_starter_credit' CHECK (
    attribution_kind = 'sales_starter_credit'
  ),
  transfer_policy TEXT NOT NULL DEFAULT 'non_transferable' CHECK (
    transfer_policy = 'non_transferable'
  ),
  amount_usd_cents INTEGER NOT NULL CHECK (amount_usd_cents > 0),
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  amount_cap_usd_cents INTEGER NOT NULL CHECK (amount_cap_usd_cents > 0),
  window_ref TEXT NOT NULL,
  window_grant_cap INTEGER NOT NULL CHECK (window_grant_cap > 0),
  credit_receipt_ref TEXT NOT NULL UNIQUE,
  redemption_receipt_refs_json TEXT NOT NULL DEFAULT '[]',
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (amount_usd_cents <= amount_cap_usd_cents)
);

CREATE INDEX IF NOT EXISTS idx_business_starter_credit_grants_pipeline
  ON business_starter_credit_grants(pipeline_ref, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_business_starter_credit_grants_window
  ON business_starter_credit_grants(window_ref, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_business_starter_credit_grants_account
  ON business_starter_credit_grants(account_ref, created_at DESC);

CREATE TRIGGER IF NOT EXISTS business_starter_credit_window_cap
BEFORE INSERT ON business_starter_credit_grants
WHEN (
  SELECT COUNT(*)
    FROM business_starter_credit_grants
   WHERE window_ref = NEW.window_ref
) >= NEW.window_grant_cap
BEGIN
  SELECT RAISE(ABORT, 'business_starter_credit_window_cap_exceeded');
END;
