CREATE TABLE IF NOT EXISTS blueprint_program_runs (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  actor_ref TEXT NOT NULL,
  purpose_ref TEXT NOT NULL,
  program_type_id TEXT NOT NULL,
  program_signature_id TEXT NOT NULL,
  module_version_id TEXT NOT NULL,
  input_snapshot_hash TEXT NOT NULL,
  typed_output_json TEXT NOT NULL DEFAULT '{}',
  confidence REAL NOT NULL,
  route_ref TEXT NOT NULL,
  cost_ref TEXT NOT NULL,
  latency_ms INTEGER NOT NULL,
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  receipt_refs_json TEXT NOT NULL DEFAULT '[]',
  authority_boundary TEXT NOT NULL DEFAULT 'evidence_only',
  direct_mutation_disabled INTEGER NOT NULL DEFAULT 1,
  no_deploy INTEGER NOT NULL DEFAULT 1,
  no_email INTEGER NOT NULL DEFAULT 1,
  no_spend INTEGER NOT NULL DEFAULT 1,
  no_source_mutation INTEGER NOT NULL DEFAULT 1,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_blueprint_program_runs_program_signature
  ON blueprint_program_runs(program_signature_id, archived_at);

CREATE INDEX IF NOT EXISTS idx_blueprint_program_runs_module_version
  ON blueprint_program_runs(module_version_id, archived_at);
