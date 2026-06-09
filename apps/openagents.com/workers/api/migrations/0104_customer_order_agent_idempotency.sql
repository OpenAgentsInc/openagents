ALTER TABLE software_orders
  ADD COLUMN agent_idempotency_key TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS software_orders_agent_idempotency_idx
  ON software_orders(user_id, agent_idempotency_key)
  WHERE agent_idempotency_key IS NOT NULL
    AND archived_at IS NULL;
