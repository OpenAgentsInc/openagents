CREATE TABLE IF NOT EXISTS blueprint_probe_contributions (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  contribution_kind TEXT NOT NULL,
  status TEXT NOT NULL,
  review_status TEXT NOT NULL,
  release_gate_ready INTEGER NOT NULL DEFAULT 0,
  candidate_runtime_allowed INTEGER NOT NULL DEFAULT 0,
  production_runtime_allowed INTEGER NOT NULL DEFAULT 0,
  blocker_refs_json TEXT NOT NULL DEFAULT '[]',
  release_gate_refs_json TEXT NOT NULL DEFAULT '[]',
  fixture_refs_json TEXT NOT NULL DEFAULT '[]',
  retained_failure_refs_json TEXT NOT NULL DEFAULT '[]',
  target_refs_json TEXT NOT NULL DEFAULT '[]',
  signature_contribution_json TEXT,
  developer_package_contribution_json TEXT,
  projection_json TEXT NOT NULL DEFAULT '{}',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_blueprint_probe_contributions_kind
  ON blueprint_probe_contributions(contribution_kind, archived_at);

CREATE INDEX IF NOT EXISTS idx_blueprint_probe_contributions_release_gate
  ON blueprint_probe_contributions(release_gate_ready, archived_at);

CREATE INDEX IF NOT EXISTS idx_blueprint_probe_contributions_status
  ON blueprint_probe_contributions(status, review_status, archived_at);
