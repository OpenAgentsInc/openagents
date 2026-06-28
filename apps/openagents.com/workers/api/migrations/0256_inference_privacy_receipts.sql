-- Paid-privacy entitlement purchase receipts and confidential-compute execution
-- receipts for privacy.khala_paid_capture_optout.v1.
--
-- These rows are public-safe receipt substrates only. They record bounded refs,
-- account refs, receipt refs, capture-exclusion reason refs, and timestamps.
-- They must never store prompts, completions, payment material, provider
-- payloads, wallet material, raw tokens, or secrets.

CREATE TABLE IF NOT EXISTS inference_privacy_entitlement_receipts (
  receipt_ref TEXT PRIMARY KEY,
  entitlement_ref TEXT NOT NULL UNIQUE,
  account_ref TEXT NOT NULL,
  purchase_ref TEXT NOT NULL UNIQUE,
  idempotency_key TEXT NOT NULL UNIQUE,
  privacy_tier TEXT NOT NULL DEFAULT 'paid_privacy',
  capture_excluded INTEGER NOT NULL DEFAULT 1 CHECK (capture_excluded IN (0, 1)),
  reason_ref TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_inference_privacy_entitlement_receipts_account
  ON inference_privacy_entitlement_receipts(account_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS inference_confidential_compute_execution_receipts (
  receipt_ref TEXT PRIMARY KEY,
  execution_ref TEXT NOT NULL UNIQUE,
  account_ref TEXT NOT NULL,
  request_ref TEXT NOT NULL UNIQUE,
  idempotency_key TEXT NOT NULL UNIQUE,
  capture_excluded INTEGER NOT NULL DEFAULT 1 CHECK (capture_excluded IN (0, 1)),
  reason_ref TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_inference_confidential_compute_receipts_account
  ON inference_confidential_compute_execution_receipts(account_ref, created_at DESC);
