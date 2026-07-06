-- MM-E2 (#8482): IAP credit-pack purchase rail (RevenueCat webhook-driven).
--
-- Follows the same shape as khala_code_paid_plan_payment_intents
-- (inference/khala-code-paid-plan-payments.ts) but for a DIFFERENT product:
-- a store purchase of a spendable credit pack, fulfilled into Pool B
-- (agent_balances.balance_msat / usd_credit_msat) rather than a plan
-- entitlement receipt. `store_transaction_id` is UNIQUE so a replayed
-- purchase webhook for the same transaction is a no-op fulfillment.

CREATE TABLE IF NOT EXISTS iap_credit_pack_purchase_intents (
  purchase_ref TEXT PRIMARY KEY,
  account_ref TEXT NOT NULL,
  user_id TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  rail TEXT NOT NULL CHECK (rail = 'iap_revenuecat'),
  store TEXT NOT NULL CHECK (store IN ('app_store', 'play_store')),
  sku TEXT NOT NULL,
  store_transaction_id TEXT NOT NULL UNIQUE,
  amount_usd_cents INTEGER NOT NULL CHECK (amount_usd_cents > 0),
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  status TEXT NOT NULL CHECK (status IN ('fulfilled', 'refunded')),
  credit_grant_ref TEXT,
  refund_receipt_ref TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  fulfilled_at TEXT,
  refunded_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_iap_credit_pack_purchase_intents_user
  ON iap_credit_pack_purchase_intents (user_id, created_at DESC);

-- Standalone webhook-event replay guard (MM-E2 "replay resistance"):
-- distinct from the transaction-level idempotency above because a purchase
-- and its later refund are DIFFERENT RevenueCat event ids referencing the
-- SAME transaction_id. Recording the event id here lets the route
-- short-circuit an exact-duplicate delivery (RevenueCat retries on a
-- non-2xx, or an operator manually replays a webhook) before touching any
-- ledger logic at all.
CREATE TABLE IF NOT EXISTS iap_webhook_events_processed (
  event_id TEXT PRIMARY KEY,
  event_type TEXT NOT NULL,
  processed_at TEXT NOT NULL
);
