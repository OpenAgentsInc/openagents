CREATE TABLE IF NOT EXISTS sarah_managed_fleet_unit_receipts (
  receipt_ref TEXT PRIMARY KEY,
  run_ref TEXT NOT NULL,
  work_unit_ref TEXT NOT NULL,
  claim_ref TEXT NOT NULL,
  assignment_ref TEXT NOT NULL,
  pylon_ref TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  account_ref_hash TEXT NOT NULL,
  session_ref TEXT NOT NULL,
  placement_ref TEXT NOT NULL,
  agent_computer_ref TEXT NOT NULL,
  artifact_ref TEXT NOT NULL UNIQUE,
  closeout_ref TEXT NOT NULL UNIQUE,
  no_measurement_caveat_ref TEXT NOT NULL UNIQUE,
  lifecycle_receipt_refs_json TEXT NOT NULL,
  resource_usage_receipt_refs_json TEXT NOT NULL,
  agent_computer_state TEXT NOT NULL,
  created_at TEXT NOT NULL,
  CONSTRAINT sarah_managed_fleet_unit_receipts_run_fk
    FOREIGN KEY (run_ref) REFERENCES sarah_fleet_run_requests(run_ref),
  CONSTRAINT sarah_managed_fleet_unit_receipts_terminal_state
    CHECK (agent_computer_state = 'reclaimed')
);

CREATE UNIQUE INDEX IF NOT EXISTS sarah_managed_fleet_unit_receipts_assignment_idx
  ON sarah_managed_fleet_unit_receipts (run_ref, work_unit_ref, assignment_ref);
