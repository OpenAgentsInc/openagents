ALTER TABLE autopilot_work_orders
  ADD COLUMN execution_closeout_json TEXT;

CREATE INDEX IF NOT EXISTS idx_autopilot_work_orders_delivered_closeouts
  ON autopilot_work_orders(state, updated_at DESC)
  WHERE execution_closeout_json IS NOT NULL
    AND archived_at IS NULL;
