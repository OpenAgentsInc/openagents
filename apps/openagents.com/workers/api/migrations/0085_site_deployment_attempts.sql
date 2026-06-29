CREATE TABLE IF NOT EXISTS site_deployment_attempts (
  id TEXT PRIMARY KEY,
  site_id TEXT NOT NULL,
  version_id TEXT NOT NULL,
  deployment_id TEXT,
  runtime_kind TEXT NOT NULL,
  runtime_script_name TEXT,
  dispatch_namespace TEXT,
  external_deployment_id TEXT,
  status TEXT NOT NULL,
  upload_receipt_ref TEXT,
  health_status TEXT NOT NULL,
  health_url TEXT,
  health_ref TEXT,
  rollback_ref TEXT,
  observability_ref TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (site_id) REFERENCES site_projects(id),
  FOREIGN KEY (version_id) REFERENCES site_versions(id),
  FOREIGN KEY (deployment_id) REFERENCES site_deployments(id)
);

CREATE INDEX IF NOT EXISTS idx_site_deployment_attempts_site_created
  ON site_deployment_attempts(site_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_site_deployment_attempts_version_created
  ON site_deployment_attempts(version_id, created_at DESC);
