CREATE TABLE IF NOT EXISTS mullet_scenarios (
  id TEXT PRIMARY KEY,
  owner_user_id TEXT NOT NULL,
  owner_email TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  name TEXT NOT NULL,
  kind TEXT NOT NULL,
  scenario_json TEXT NOT NULL,
  source_refs_json TEXT NOT NULL,
  provenance_summary_json TEXT NOT NULL,
  visibility TEXT NOT NULL DEFAULT 'private' CHECK (visibility = 'private'),
  export_redaction_state TEXT NOT NULL DEFAULT 'not_checked' CHECK (
    export_redaction_state IN ('not_checked', 'passed', 'failed')
  ),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_mullet_scenarios_owner_updated
  ON mullet_scenarios (owner_user_id, updated_at DESC)
  WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS mullet_simulation_runs (
  id TEXT PRIMARY KEY,
  scenario_id TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  owner_email TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'succeeded', 'failed')),
  run_json TEXT NOT NULL,
  source_refs_json TEXT NOT NULL,
  provenance_summary_json TEXT NOT NULL,
  provider_settlement_state TEXT NOT NULL,
  power_data_state TEXT NOT NULL,
  visibility TEXT NOT NULL DEFAULT 'private' CHECK (visibility = 'private'),
  export_redaction_state TEXT NOT NULL DEFAULT 'not_checked' CHECK (
    export_redaction_state IN ('not_checked', 'passed', 'failed')
  ),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT,
  deleted_at TEXT,
  FOREIGN KEY (scenario_id) REFERENCES mullet_scenarios (id)
);

CREATE INDEX IF NOT EXISTS idx_mullet_simulation_runs_owner_updated
  ON mullet_simulation_runs (owner_user_id, updated_at DESC)
  WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_mullet_simulation_runs_scenario
  ON mullet_simulation_runs (scenario_id, updated_at DESC)
  WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS mullet_run_hourly_results (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  scenario_id TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  hour_index INTEGER NOT NULL,
  timestamp TEXT NOT NULL,
  selected_mode TEXT NOT NULL,
  reason_code TEXT NOT NULL,
  energy_mwh REAL NOT NULL,
  result_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (run_id) REFERENCES mullet_simulation_runs (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_mullet_run_hourly_results_run_hour
  ON mullet_run_hourly_results (run_id, hour_index);

CREATE TABLE IF NOT EXISTS mullet_run_candidate_modes (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  hourly_result_id TEXT NOT NULL,
  scenario_id TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  hour_index INTEGER NOT NULL,
  candidate_index INTEGER NOT NULL,
  timestamp TEXT NOT NULL,
  mode TEXT NOT NULL,
  reason_code TEXT NOT NULL,
  risk_adjusted_net_usd_per_mwh REAL NOT NULL,
  clears_readiness INTEGER NOT NULL CHECK (clears_readiness IN (0, 1)),
  clears_demand INTEGER NOT NULL CHECK (clears_demand IN (0, 1)),
  clears_provider_floor INTEGER NOT NULL CHECK (clears_provider_floor IN (0, 1)),
  candidate_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (run_id) REFERENCES mullet_simulation_runs (id),
  FOREIGN KEY (hourly_result_id) REFERENCES mullet_run_hourly_results (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_mullet_run_candidate_modes_run_hour_candidate
  ON mullet_run_candidate_modes (run_id, hour_index, candidate_index);

CREATE INDEX IF NOT EXISTS idx_mullet_run_candidate_modes_run
  ON mullet_run_candidate_modes (run_id, hour_index, candidate_index);

CREATE TABLE IF NOT EXISTS mullet_run_exports (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  scenario_id TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  owner_email TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  format TEXT NOT NULL CHECK (format IN ('markdown', 'json')),
  export_json TEXT NOT NULL,
  private_visibility INTEGER NOT NULL DEFAULT 1 CHECK (private_visibility = 1),
  redaction_status TEXT NOT NULL CHECK (
    redaction_status IN ('not_checked', 'passed', 'failed')
  ),
  content_ref TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (run_id) REFERENCES mullet_simulation_runs (id)
);

CREATE INDEX IF NOT EXISTS idx_mullet_run_exports_run
  ON mullet_run_exports (run_id, created_at DESC);
