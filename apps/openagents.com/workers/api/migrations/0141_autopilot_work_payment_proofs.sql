ALTER TABLE autopilot_work_orders
  ADD COLUMN buyer_payment_proof_ref TEXT;

CREATE INDEX IF NOT EXISTS idx_autopilot_work_orders_payment_proof
  ON autopilot_work_orders(buyer_payment_proof_ref)
  WHERE buyer_payment_proof_ref IS NOT NULL
    AND archived_at IS NULL;
