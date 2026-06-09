CREATE TABLE IF NOT EXISTS site_revision_feedback (
  id TEXT PRIMARY KEY NOT NULL,
  software_order_id TEXT NOT NULL REFERENCES software_orders(id) ON DELETE CASCADE,
  site_id TEXT REFERENCES site_projects(id) ON DELETE SET NULL,
  site_version_id TEXT REFERENCES site_versions(id) ON DELETE SET NULL,
  site_deployment_id TEXT REFERENCES site_deployments(id) ON DELETE SET NULL,
  author_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  body TEXT NOT NULL CHECK (length(body) > 0 AND length(body) <= 4000),
  status TEXT NOT NULL CHECK (
    status IN (
      'submitted',
      'queued',
      'running',
      'addressed',
      'closed',
      'rejected'
    )
  ),
  source TEXT NOT NULL CHECK (
    source IN (
      'customer_order_ui',
      'operator',
      'agent'
    )
  ),
  visibility TEXT NOT NULL CHECK (visibility IN ('private', 'team', 'public')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS site_revision_feedback_order_created_idx
  ON site_revision_feedback(software_order_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS site_revision_feedback_site_created_idx
  ON site_revision_feedback(site_id, created_at DESC)
  WHERE site_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS site_revision_feedback_version_created_idx
  ON site_revision_feedback(site_version_id, created_at DESC)
  WHERE site_version_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS site_revision_feedback_status_updated_idx
  ON site_revision_feedback(status, updated_at DESC)
  WHERE archived_at IS NULL;
