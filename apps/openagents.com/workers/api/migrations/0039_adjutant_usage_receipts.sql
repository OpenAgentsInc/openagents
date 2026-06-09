CREATE TABLE IF NOT EXISTS adjutant_usage_receipts (
  id TEXT PRIMARY KEY NOT NULL,
  assignment_id TEXT NOT NULL REFERENCES adjutant_assignments(id) ON DELETE CASCADE,
  software_order_id TEXT REFERENCES software_orders(id) ON DELETE SET NULL,
  site_id TEXT REFERENCES site_projects(id) ON DELETE SET NULL,
  adjustment_id TEXT REFERENCES adjutant_adjustment_requests(id) ON DELETE SET NULL,
  run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
  category TEXT NOT NULL CHECK (
    category IN ('generation', 'build', 'hosting', 'storage', 'adjustment')
  ),
  visibility TEXT NOT NULL CHECK (visibility IN ('private', 'team', 'public')),
  billing_mode TEXT NOT NULL CHECK (
    billing_mode IN ('public_beta_free', 'paid_credits')
  ),
  summary TEXT NOT NULL CHECK (length(summary) > 0),
  quantity INTEGER NOT NULL DEFAULT 0 CHECK (quantity >= 0),
  unit TEXT NOT NULL CHECK (length(unit) > 0),
  credits_charged_cents INTEGER NOT NULL DEFAULT 0 CHECK (credits_charged_cents >= 0),
  currency TEXT NOT NULL DEFAULT 'USD',
  billing_ledger_entry_id TEXT REFERENCES billing_ledger_entries(id) ON DELETE SET NULL,
  public_receipt_json TEXT NOT NULL DEFAULT '{}',
  team_receipt_json TEXT NOT NULL DEFAULT '{}',
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS adjutant_usage_receipts_assignment_created_idx
  ON adjutant_usage_receipts(assignment_id, created_at DESC);

CREATE INDEX IF NOT EXISTS adjutant_usage_receipts_order_created_idx
  ON adjutant_usage_receipts(software_order_id, created_at DESC)
  WHERE software_order_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS adjutant_usage_receipts_site_created_idx
  ON adjutant_usage_receipts(site_id, created_at DESC)
  WHERE site_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS adjutant_usage_receipts_run_created_idx
  ON adjutant_usage_receipts(run_id, created_at DESC)
  WHERE run_id IS NOT NULL;
