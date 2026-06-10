CREATE TABLE IF NOT EXISTS training_runs (
  id TEXT PRIMARY KEY,
  training_run_ref TEXT NOT NULL UNIQUE,
  promise_ref TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('planned', 'active', 'sealed', 'reconciled')),
  source_refs_json TEXT NOT NULL,
  receipt_refs_json TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_training_runs_promise_ref
  ON training_runs(promise_ref, updated_at DESC);

CREATE TABLE IF NOT EXISTS training_windows (
  id TEXT PRIMARY KEY,
  window_ref TEXT NOT NULL UNIQUE,
  training_run_ref TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('planned', 'active', 'sealed', 'reconciled')),
  homework_kind TEXT NOT NULL CHECK (homework_kind IN ('admin_dispatched_homework', 'operator_planned_homework', 'auto_starter')),
  priority INTEGER NOT NULL,
  dataset_refs_json TEXT NOT NULL,
  source_refs_json TEXT NOT NULL,
  receipt_refs_json TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  planned_at TEXT NOT NULL,
  activated_at TEXT,
  sealed_at TEXT,
  reconciled_at TEXT,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (training_run_ref) REFERENCES training_runs(training_run_ref)
);

CREATE INDEX IF NOT EXISTS idx_training_windows_run_ref
  ON training_windows(training_run_ref, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_training_windows_claimable
  ON training_windows(state, homework_kind, priority DESC, planned_at ASC);

CREATE TABLE IF NOT EXISTS training_window_events (
  id TEXT PRIMARY KEY,
  window_ref TEXT NOT NULL,
  transition_kind TEXT NOT NULL,
  state_from TEXT,
  state_to TEXT NOT NULL,
  actor_ref TEXT NOT NULL,
  receipt_ref TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (window_ref) REFERENCES training_windows(window_ref)
);

CREATE INDEX IF NOT EXISTS idx_training_window_events_window_ref
  ON training_window_events(window_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS training_window_leases (
  id TEXT PRIMARY KEY,
  lease_ref TEXT NOT NULL UNIQUE,
  window_ref TEXT NOT NULL,
  training_run_ref TEXT NOT NULL,
  pylon_ref TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('active', 'released')),
  receipt_refs_json TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  claimed_at TEXT NOT NULL,
  lease_expires_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (window_ref) REFERENCES training_windows(window_ref),
  FOREIGN KEY (training_run_ref) REFERENCES training_runs(training_run_ref)
);

CREATE INDEX IF NOT EXISTS idx_training_window_leases_pylon_ref
  ON training_window_leases(pylon_ref, claimed_at DESC);

CREATE INDEX IF NOT EXISTS idx_training_window_leases_active
  ON training_window_leases(state, lease_expires_at);
