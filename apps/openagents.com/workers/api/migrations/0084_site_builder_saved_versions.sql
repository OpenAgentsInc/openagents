CREATE TABLE IF NOT EXISTS site_builder_saved_versions (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  session_id TEXT NOT NULL,
  site_id TEXT NOT NULL,
  site_version_id TEXT NOT NULL,
  preview_id TEXT,
  artifact_ref TEXT,
  build_receipt_ref TEXT,
  source_hash TEXT,
  notes TEXT,
  site_metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (session_id) REFERENCES site_builder_sessions(id),
  FOREIGN KEY (site_id) REFERENCES site_projects(id),
  FOREIGN KEY (site_version_id) REFERENCES site_versions(id),
  FOREIGN KEY (preview_id) REFERENCES site_builder_previews(id)
);

CREATE INDEX IF NOT EXISTS idx_site_builder_saved_versions_session_created
  ON site_builder_saved_versions(session_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_site_builder_saved_versions_site_created
  ON site_builder_saved_versions(site_id, created_at DESC);
