CREATE TABLE IF NOT EXISTS pylon_api_assignments (
  id TEXT PRIMARY KEY,
  assignment_ref TEXT NOT NULL UNIQUE,
  pylon_ref TEXT NOT NULL,
  owner_agent_user_id TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  job_kind TEXT NOT NULL,
  state TEXT NOT NULL,
  lease_expires_at TEXT NOT NULL,
  task_refs_json TEXT NOT NULL,
  acceptance_criteria_refs_json TEXT NOT NULL,
  result_expectation_refs_json TEXT NOT NULL,
  artifact_refs_json TEXT NOT NULL,
  proof_refs_json TEXT NOT NULL,
  accepted_work_refs_json TEXT NOT NULL,
  rejection_refs_json TEXT NOT NULL,
  closeout_refs_json TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (pylon_ref) REFERENCES pylon_api_registrations(pylon_ref)
);

CREATE INDEX IF NOT EXISTS idx_pylon_api_assignments_pylon_updated
  ON pylon_api_assignments(pylon_ref, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_pylon_api_assignments_state_updated
  ON pylon_api_assignments(state, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_pylon_api_assignments_lease_expires
  ON pylon_api_assignments(lease_expires_at);
