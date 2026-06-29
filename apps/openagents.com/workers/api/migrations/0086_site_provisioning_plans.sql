CREATE TABLE IF NOT EXISTS site_provisioning_plans (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  site_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('review_required', 'approved')),
  requested_by_user_id TEXT,
  reviewed_by_user_id TEXT,
  resource_manifest_json TEXT NOT NULL,
  receipt_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  reviewed_at TEXT,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (site_id) REFERENCES site_projects(id)
);

CREATE INDEX IF NOT EXISTS idx_site_provisioning_plans_site_created
  ON site_provisioning_plans(site_id, created_at DESC);
