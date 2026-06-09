CREATE TABLE IF NOT EXISTS first_batch_payment_policies (
  id TEXT PRIMARY KEY NOT NULL,
  software_order_id TEXT NOT NULL REFERENCES software_orders(id) ON DELETE CASCADE,
  assignment_id TEXT REFERENCES adjutant_assignments(id) ON DELETE SET NULL,
  site_id TEXT REFERENCES site_projects(id) ON DELETE SET NULL,
  policy_mode TEXT NOT NULL CHECK (
    policy_mode IN ('public_beta_free', 'operator_grant')
  ),
  applied_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  reason TEXT NOT NULL CHECK (length(reason) > 0),
  customer_safe_summary TEXT NOT NULL CHECK (length(customer_safe_summary) > 0),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS first_batch_payment_policies_order_active_idx
  ON first_batch_payment_policies(software_order_id)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS first_batch_payment_policies_assignment_idx
  ON first_batch_payment_policies(assignment_id, updated_at DESC)
  WHERE assignment_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS first_batch_payment_policies_site_idx
  ON first_batch_payment_policies(site_id, updated_at DESC)
  WHERE site_id IS NOT NULL AND archived_at IS NULL;
