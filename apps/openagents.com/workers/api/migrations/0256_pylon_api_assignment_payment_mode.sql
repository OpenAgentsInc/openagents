ALTER TABLE pylon_api_assignments
  ADD COLUMN payment_mode TEXT NOT NULL DEFAULT 'unpaid_smoke'
  CHECK (payment_mode IN (
    'unpaid_smoke',
    'operator_credit',
    'payable_pending_settlement',
    'settled_bitcoin',
    'rejected_no_pay'
  ));
