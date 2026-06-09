CREATE TABLE IF NOT EXISTS site_compatibility_checks (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT NOT NULL REFERENCES site_projects(id) ON DELETE CASCADE,
  source_kind TEXT NOT NULL CHECK (
    source_kind IN ('github_import', 'operator_static')
  ),
  source_repository_json TEXT,
  status TEXT NOT NULL CHECK (status IN ('ready', 'warning', 'blocked', 'unknown')),
  confidence TEXT NOT NULL CHECK (confidence IN ('high', 'medium', 'low')),
  package_manager TEXT,
  build_command TEXT,
  output_kind TEXT NOT NULL CHECK (output_kind IN ('static', 'worker_module', 'ssr', 'unknown')),
  output_path TEXT,
  worker_module_path TEXT,
  needs_d1 INTEGER NOT NULL DEFAULT 0 CHECK (needs_d1 IN (0, 1)),
  needs_r2 INTEGER NOT NULL DEFAULT 0 CHECK (needs_r2 IN (0, 1)),
  needs_workspace_auth INTEGER NOT NULL DEFAULT 0 CHECK (needs_workspace_auth IN (0, 1)),
  needs_public_auth INTEGER NOT NULL DEFAULT 0 CHECK (needs_public_auth IN (0, 1)),
  env_keys_json TEXT NOT NULL DEFAULT '[]',
  findings_json TEXT NOT NULL DEFAULT '[]',
  blockers_json TEXT NOT NULL DEFAULT '[]',
  warnings_json TEXT NOT NULL DEFAULT '[]',
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  customer_safe_status TEXT NOT NULL,
  customer_safe_next_action TEXT NOT NULL,
  checked_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS site_compatibility_checks_site_created_idx
  ON site_compatibility_checks(site_id, created_at DESC)
  WHERE archived_at IS NULL;
