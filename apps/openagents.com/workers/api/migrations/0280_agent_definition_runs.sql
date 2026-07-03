CREATE TABLE IF NOT EXISTS agent_definition_runs (
  run_id TEXT PRIMARY KEY,
  owner_agent_user_id TEXT NOT NULL,
  definition_id TEXT NOT NULL,
  definition_ref TEXT NOT NULL,
  trigger_ref TEXT NOT NULL,
  lane TEXT NOT NULL,
  status TEXT NOT NULL,
  pylon_ref TEXT,
  assignment_ref TEXT,
  durable_request_id TEXT NOT NULL,
  durable_stream_url TEXT,
  forge_tenant_ref TEXT NOT NULL,
  forge_work_ref TEXT NOT NULL,
  refusal_error TEXT,
  refusal_reason TEXT,
  evidence_refs_json TEXT NOT NULL,
  trigger_payload_json TEXT NOT NULL,
  runtime_run_json TEXT NOT NULL,
  initial_events_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_definition_runs_owner_definition_updated
  ON agent_definition_runs(owner_agent_user_id, definition_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_agent_definition_runs_assignment
  ON agent_definition_runs(assignment_ref);

CREATE INDEX IF NOT EXISTS idx_agent_definition_runs_forge_work
  ON agent_definition_runs(forge_tenant_ref, forge_work_ref);
