CREATE TABLE IF NOT EXISTS training_verification_challenges (
  id TEXT PRIMARY KEY,
  challenge_ref TEXT NOT NULL UNIQUE,
  training_run_ref TEXT NOT NULL,
  window_ref TEXT,
  contribution_ref TEXT,
  homework_kind TEXT NOT NULL,
  verification_class TEXT NOT NULL CHECK (verification_class IN (
    'deterministic_recompute',
    'exact_trace_replay',
    'freivalds_merkle',
    'seeded_replication',
    'statistical_cross_check'
  )),
  sampling_policy TEXT NOT NULL CHECK (sampling_policy IN ('aggregate', 'per_contribution')),
  state TEXT NOT NULL CHECK (state IN ('Queued', 'Leased', 'Retrying', 'Verified', 'Rejected', 'TimedOut')),
  attempt_count INTEGER NOT NULL,
  max_attempts INTEGER NOT NULL,
  lease_ref TEXT,
  leased_to_ref TEXT,
  lease_expires_at TEXT,
  payload_json TEXT NOT NULL,
  commitment_refs_json TEXT NOT NULL,
  failure_codes_json TEXT NOT NULL,
  verdict_refs_json TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  verified_at TEXT,
  rejected_at TEXT,
  timed_out_at TEXT,
  archived_at TEXT,
  FOREIGN KEY (training_run_ref) REFERENCES training_runs(training_run_ref)
);

CREATE INDEX IF NOT EXISTS idx_training_verification_challenges_run
  ON training_verification_challenges(training_run_ref, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_training_verification_challenges_window
  ON training_verification_challenges(window_ref, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_training_verification_challenges_lease
  ON training_verification_challenges(state, verification_class, created_at ASC);

CREATE TABLE IF NOT EXISTS training_verification_events (
  id TEXT PRIMARY KEY,
  challenge_ref TEXT NOT NULL,
  transition_kind TEXT NOT NULL,
  state_from TEXT,
  state_to TEXT NOT NULL,
  validator_ref TEXT,
  failure_codes_json TEXT NOT NULL,
  receipt_refs_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (challenge_ref) REFERENCES training_verification_challenges(challenge_ref)
);

CREATE INDEX IF NOT EXISTS idx_training_verification_events_challenge
  ON training_verification_events(challenge_ref, created_at DESC);
