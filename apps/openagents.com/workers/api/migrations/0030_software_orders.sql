CREATE TABLE IF NOT EXISTS software_orders (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  status TEXT NOT NULL DEFAULT 'submitted' CHECK (
    status IN (
      'submitted',
      'scoping',
      'free_slice_ready',
      'quote_ready',
      'agent_queued',
      'agent_running',
      'delivered',
      'needs_customer_input',
      'declined',
      'unavailable'
    )
  ),
  visibility TEXT NOT NULL DEFAULT 'public' CHECK (visibility IN ('public')),
  request TEXT NOT NULL,
  repository_provider TEXT CHECK (
    repository_provider IS NULL OR repository_provider IN ('github')
  ),
  repository_owner TEXT,
  repository_name TEXT,
  repository_full_name TEXT,
  repository_private INTEGER CHECK (
    repository_private IS NULL OR repository_private IN (0, 1)
  ),
  repository_default_branch TEXT,
  repository_html_url TEXT,
  public_work_acknowledged_at TEXT NOT NULL,
  data_use_acknowledged_at TEXT NOT NULL,
  compute_payment_acknowledged_at TEXT NOT NULL,
  provider_account_required INTEGER NOT NULL DEFAULT 0 CHECK (
    provider_account_required IN (0, 1)
  ),
  free_slice_cents INTEGER NOT NULL DEFAULT 5000,
  quote_cents INTEGER,
  current_run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
  agent_started_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS software_orders_user_active_idx
  ON software_orders(user_id, archived_at, created_at DESC);

CREATE INDEX IF NOT EXISTS software_orders_status_idx
  ON software_orders(status, updated_at DESC);
