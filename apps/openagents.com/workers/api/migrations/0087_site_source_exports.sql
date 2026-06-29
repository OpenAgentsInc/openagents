CREATE TABLE IF NOT EXISTS site_source_exports (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  site_id TEXT NOT NULL,
  version_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN ('requested', 'approved', 'exported', 'failed', 'expired', 'revoked')
  ),
  export_kind TEXT NOT NULL CHECK (
    export_kind IN ('download_token', 'github_branch', 'github_pull_request')
  ),
  actor_user_id TEXT,
  approved_by_user_id TEXT,
  destination_provider TEXT NOT NULL,
  destination_owner TEXT,
  destination_repository TEXT,
  destination_branch TEXT,
  destination_pull_request_url TEXT,
  destination_url TEXT,
  source_archive_r2_key TEXT,
  artifact_manifest_r2_key TEXT,
  worker_module_r2_key TEXT,
  source_artifact_ref TEXT,
  token_ref TEXT,
  token_hash TEXT,
  token_expires_at TEXT,
  secret_scan_status TEXT NOT NULL CHECK (
    secret_scan_status IN ('passed', 'failed')
  ),
  secret_scan_ref TEXT,
  receipt_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (site_id) REFERENCES site_projects(id),
  FOREIGN KEY (version_id) REFERENCES site_versions(id)
);

CREATE INDEX IF NOT EXISTS idx_site_source_exports_site_created
  ON site_source_exports(site_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_site_source_exports_version_created
  ON site_source_exports(site_id, version_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_site_source_exports_token_ref
  ON site_source_exports(token_ref, token_expires_at)
  WHERE token_ref IS NOT NULL AND archived_at IS NULL;
