-- #5052 (epic #5051): self-serve worker->validator executor-trace completion.
-- A contributor that claimed a training window lease can now submit its worker
-- trace commitment (POST /api/training/leases/{leaseRef}/trace-submission,
-- requireAgent) and a DISTINCT-device validator can later replay-verify it
-- (POST /api/training/leases/{leaseRef}/replay-verdict, requireAgent). The
-- worker half is recorded here as a pending contribution awaiting a validator;
-- it is paired with the validator's replay digest in the verdict route, which
-- builds the existing exact_trace_replay verification challenge.
--
-- Replay is the trust anchor: this table records the SUBMITTED worker commitment
-- only and grants no acceptance, payout, settlement, or public-claim authority.
-- A row's `verification_challenge_ref` is populated only once a distinct-device
-- validator pairs with it; the Verified/Rejected verdict lives on the existing
-- training_verification_challenges row. All fields are public-safe refs or
-- bounded step integers (no raw prompts, host paths, wallet material, preimages).
--
-- Idempotency: one pending worker contribution per (lease_ref, workload_family).

CREATE TABLE IF NOT EXISTS training_trace_contributions (
  id TEXT PRIMARY KEY,
  contribution_ref TEXT NOT NULL UNIQUE,
  lease_ref TEXT NOT NULL,
  window_ref TEXT NOT NULL,
  training_run_ref TEXT NOT NULL,
  pylon_ref TEXT NOT NULL,
  workload_family TEXT NOT NULL,
  assignment_ref TEXT NOT NULL,
  pylon_device_ref TEXT NOT NULL,
  trace_commitment_digest_ref TEXT NOT NULL,
  sampled_window_ref TEXT NOT NULL,
  sampled_window_start_step INTEGER NOT NULL,
  sampled_window_end_step INTEGER NOT NULL,
  worker_receipt_ref TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('pending', 'paired')),
  validator_device_ref TEXT,
  replay_digest_ref TEXT,
  verification_challenge_ref TEXT,
  public_projection_json TEXT NOT NULL,
  submitted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (lease_ref, workload_family),
  FOREIGN KEY (lease_ref) REFERENCES training_window_leases(lease_ref),
  FOREIGN KEY (window_ref) REFERENCES training_windows(window_ref),
  FOREIGN KEY (training_run_ref) REFERENCES training_runs(training_run_ref)
);

CREATE INDEX IF NOT EXISTS idx_training_trace_contributions_lease
  ON training_trace_contributions(lease_ref, workload_family);

CREATE INDEX IF NOT EXISTS idx_training_trace_contributions_run_state
  ON training_trace_contributions(training_run_ref, state, submitted_at DESC);
