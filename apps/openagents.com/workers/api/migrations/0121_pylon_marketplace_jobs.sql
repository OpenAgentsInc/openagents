CREATE TABLE IF NOT EXISTS pylon_marketplace_job_intakes (
  id TEXT PRIMARY KEY,
  intake_ref TEXT NOT NULL UNIQUE,
  job_ref TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  request_hash TEXT NOT NULL,
  state TEXT NOT NULL,
  source TEXT NOT NULL,
  job_kind TEXT NOT NULL,
  privacy_class TEXT NOT NULL,
  record_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pylon_marketplace_job_intakes_updated
  ON pylon_marketplace_job_intakes(updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_pylon_marketplace_job_intakes_state_updated
  ON pylon_marketplace_job_intakes(state, updated_at DESC);

CREATE TABLE IF NOT EXISTS pylon_marketplace_assignments (
  id TEXT PRIMARY KEY,
  assignment_ref TEXT NOT NULL UNIQUE,
  intake_ref TEXT NOT NULL,
  job_ref TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  request_hash TEXT NOT NULL,
  state TEXT NOT NULL,
  payout_state TEXT NOT NULL,
  record_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (intake_ref) REFERENCES pylon_marketplace_job_intakes(intake_ref)
);

CREATE INDEX IF NOT EXISTS idx_pylon_marketplace_assignments_intake_updated
  ON pylon_marketplace_assignments(intake_ref, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_pylon_marketplace_assignments_state_updated
  ON pylon_marketplace_assignments(state, updated_at DESC);

CREATE TABLE IF NOT EXISTS pylon_marketplace_triage_actions (
  id TEXT PRIMARY KEY,
  target_intake_ref TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  request_hash TEXT NOT NULL,
  outcome TEXT NOT NULL,
  response_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (target_intake_ref) REFERENCES pylon_marketplace_job_intakes(intake_ref)
);

CREATE INDEX IF NOT EXISTS idx_pylon_marketplace_triage_actions_target
  ON pylon_marketplace_triage_actions(target_intake_ref, created_at DESC);
