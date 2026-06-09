ALTER TABLE provider_account_leases
  ADD COLUMN order_id TEXT;

ALTER TABLE provider_account_leases
  ADD COLUMN selected_by_actor TEXT;

ALTER TABLE provider_account_leases
  ADD COLUMN last_touched_at TEXT;

ALTER TABLE provider_account_leases
  ADD COLUMN failure_class TEXT;

CREATE INDEX IF NOT EXISTS provider_account_leases_order_idx
  ON provider_account_leases(order_id, started_at DESC);
