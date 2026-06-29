ALTER TABLE autopilot_work_orders
  ADD COLUMN review_decision_json TEXT;

CREATE INDEX IF NOT EXISTS idx_autopilot_work_orders_review_decisions
  ON autopilot_work_orders(state, updated_at DESC)
  WHERE review_decision_json IS NOT NULL
    AND archived_at IS NULL;
