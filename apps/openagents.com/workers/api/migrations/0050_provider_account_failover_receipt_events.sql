ALTER TABLE provider_account_failover_receipts
  ADD COLUMN order_id TEXT;

ALTER TABLE provider_account_failover_receipts
  ADD COLUMN policy_version TEXT NOT NULL DEFAULT 'provider-account-lease-policy:v1';

ALTER TABLE provider_account_failover_receipts
  ADD COLUMN cooldown_until TEXT;

ALTER TABLE provider_account_failover_receipts
  ADD COLUMN operator_summary TEXT NOT NULL DEFAULT 'Provider account failover was recorded.';

ALTER TABLE provider_account_failover_receipts
  ADD COLUMN customer_safe_summary TEXT;

CREATE INDEX IF NOT EXISTS provider_account_failover_receipts_order_idx
  ON provider_account_failover_receipts(order_id, created_at DESC);

CREATE INDEX IF NOT EXISTS provider_account_failover_receipts_created_idx
  ON provider_account_failover_receipts(created_at DESC);
