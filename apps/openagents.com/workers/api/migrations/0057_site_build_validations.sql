CREATE TABLE IF NOT EXISTS site_build_validations (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT NOT NULL REFERENCES site_projects(id) ON DELETE CASCADE,
  compatibility_check_id TEXT,
  source_kind TEXT NOT NULL CHECK (
    source_kind IN ('autopilot_generated', 'github_import', 'operator_static')
  ),
  source_repository_json TEXT,
  source_commit_sha TEXT,
  source_hash TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('passed', 'warning', 'failed', 'blocked', 'unknown')),
  package_manager TEXT,
  requested_build_command TEXT,
  build_command TEXT,
  output_kind TEXT NOT NULL CHECK (output_kind IN ('static', 'worker_module', 'ssr', 'unknown')),
  output_path TEXT,
  worker_module_path TEXT,
  manifest_json TEXT NOT NULL DEFAULT '{}',
  bounded_logs_json TEXT NOT NULL DEFAULT '[]',
  log_line_count INTEGER NOT NULL DEFAULT 0,
  log_truncated INTEGER NOT NULL DEFAULT 0 CHECK (log_truncated IN (0, 1)),
  findings_json TEXT NOT NULL DEFAULT '[]',
  blockers_json TEXT NOT NULL DEFAULT '[]',
  warnings_json TEXT NOT NULL DEFAULT '[]',
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  customer_safe_status TEXT NOT NULL,
  customer_safe_next_action TEXT NOT NULL,
  validated_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS site_build_validations_site_created_idx
  ON site_build_validations(site_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS site_build_validations_compatibility_idx
  ON site_build_validations(compatibility_check_id, created_at DESC)
  WHERE archived_at IS NULL;
