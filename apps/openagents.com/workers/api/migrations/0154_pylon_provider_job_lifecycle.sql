CREATE TABLE IF NOT EXISTS pylon_provider_job_lifecycle (
  id TEXT PRIMARY KEY,
  pylon_ref TEXT NOT NULL,
  assignment_ref TEXT NOT NULL UNIQUE,
  owner_agent_user_id TEXT NOT NULL,
  job_kind TEXT NOT NULL,
  stage TEXT NOT NULL CHECK (
    stage IN (
      'offered',
      'accepted',
      'running',
      'artifact_submitted',
      'closeout_submitted',
      'accepted_work'
    )
  ),
  task_refs_json TEXT NOT NULL,
  artifact_refs_json TEXT NOT NULL,
  proof_refs_json TEXT NOT NULL,
  closeout_refs_json TEXT NOT NULL,
  accepted_work_refs_json TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (pylon_ref) REFERENCES pylon_api_registrations(pylon_ref),
  FOREIGN KEY (assignment_ref) REFERENCES pylon_api_assignments(assignment_ref)
);

CREATE INDEX IF NOT EXISTS idx_pylon_provider_job_lifecycle_pylon_updated
  ON pylon_provider_job_lifecycle(pylon_ref, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_pylon_provider_job_lifecycle_stage_updated
  ON pylon_provider_job_lifecycle(stage, updated_at DESC);
