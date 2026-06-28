-- AgentCL eval v0 durable schema (#6763).
--
-- These tables store public-safe AgentCL evaluation receipts only. Raw prompts,
-- provider payloads, private trajectories, command output, local paths, and
-- secrets stay outside D1. The schema keeps Plasticity, Stability, and
-- Generalization separate so a caller cannot collapse them into one
-- "memory improved" number.

CREATE TABLE IF NOT EXISTS gym_agentcl_eval_runs (
  eval_ref TEXT PRIMARY KEY,
  schema_version TEXT NOT NULL,
  environment_ref TEXT NOT NULL,
  experiment_id TEXT NOT NULL,
  stream_kind TEXT NOT NULL CHECK (stream_kind IN ('naive', 'compositional')),
  run_ref TEXT NOT NULL,
  task_set_ref TEXT,
  verifier_ref TEXT,
  runner_config_id TEXT,
  seam_id TEXT,
  seam_can_spend INTEGER NOT NULL DEFAULT 0 CHECK (seam_can_spend IN (0, 1)),
  state TEXT NOT NULL CHECK (
    state IN ('planned', 'running', 'completed', 'aborted', 'rejected')
  ),
  decision_grade INTEGER NOT NULL DEFAULT 0 CHECK (decision_grade IN (0, 1)),
  public_claim_eligible INTEGER NOT NULL DEFAULT 0 CHECK (
    public_claim_eligible IN (0, 1)
  ),
  collapse_gains_into_one_number INTEGER NOT NULL DEFAULT 0 CHECK (
    collapse_gains_into_one_number = 0
  ),
  run_metadata_json TEXT NOT NULL,
  proof_refs_json TEXT NOT NULL,
  caveat_refs_json TEXT NOT NULL,
  blocker_refs_json TEXT NOT NULL,
  started_at TEXT,
  completed_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_gym_agentcl_eval_runs_updated_at
  ON gym_agentcl_eval_runs (updated_at DESC, eval_ref ASC);

CREATE INDEX IF NOT EXISTS idx_gym_agentcl_eval_runs_experiment
  ON gym_agentcl_eval_runs (experiment_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS gym_agentcl_eval_phase_metrics (
  eval_ref TEXT NOT NULL,
  phase TEXT NOT NULL CHECK (
    phase IN (
      'baseline',
      'first_pass',
      'frozen_second_pass',
      'held_out_baseline',
      'held_out_pass'
    )
  ),
  task_role TEXT NOT NULL CHECK (
    task_role IN ('source', 'complex', 'held_out')
  ),
  task_count INTEGER NOT NULL CHECK (task_count > 0),
  accepted_outcome_rate REAL NOT NULL CHECK (
    accepted_outcome_rate >= 0 AND accepted_outcome_rate <= 1
  ),
  score_bps INTEGER NOT NULL CHECK (score_bps >= 0 AND score_bps <= 10000),
  report_ref TEXT,
  receipt_ref TEXT,
  metric_metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  PRIMARY KEY (eval_ref, phase),
  FOREIGN KEY (eval_ref)
    REFERENCES gym_agentcl_eval_runs (eval_ref)
    ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_gym_agentcl_eval_phase_metrics_role
  ON gym_agentcl_eval_phase_metrics (task_role, phase);

CREATE TABLE IF NOT EXISTS gym_agentcl_eval_gain_metrics (
  eval_ref TEXT NOT NULL,
  gain_kind TEXT NOT NULL CHECK (
    gain_kind IN ('plasticity', 'stability', 'generalization')
  ),
  gain_value REAL NOT NULL,
  gain_bps INTEGER NOT NULL,
  baseline_phase TEXT NOT NULL,
  comparison_phase TEXT NOT NULL,
  evidence_refs_json TEXT NOT NULL,
  metric_metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  PRIMARY KEY (eval_ref, gain_kind),
  FOREIGN KEY (eval_ref)
    REFERENCES gym_agentcl_eval_runs (eval_ref)
    ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS gym_agentcl_eval_run_state_events (
  event_ref TEXT PRIMARY KEY,
  eval_ref TEXT NOT NULL,
  event_index INTEGER NOT NULL CHECK (event_index >= 0),
  state TEXT NOT NULL CHECK (
    state IN ('planned', 'running', 'completed', 'aborted', 'rejected')
  ),
  observed_at TEXT NOT NULL,
  state_metadata_json TEXT NOT NULL DEFAULT '{}',
  FOREIGN KEY (eval_ref)
    REFERENCES gym_agentcl_eval_runs (eval_ref)
    ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_gym_agentcl_eval_run_state_events_order
  ON gym_agentcl_eval_run_state_events (eval_ref, event_index);

CREATE TABLE IF NOT EXISTS gym_agentcl_eval_prompt_mutations (
  mutation_ref TEXT PRIMARY KEY,
  eval_ref TEXT NOT NULL,
  run_ref TEXT NOT NULL,
  pass TEXT NOT NULL CHECK (
    pass IN ('baseline', 'first_pass', 'frozen_second_pass', 'held_out_pass')
  ),
  task_ref TEXT NOT NULL,
  step_index INTEGER NOT NULL CHECK (step_index >= 0),
  template_ref TEXT NOT NULL,
  memory_before_refs_json TEXT NOT NULL,
  memory_after_refs_json TEXT NOT NULL,
  feedback_ref TEXT NOT NULL,
  mutation_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (eval_ref)
    REFERENCES gym_agentcl_eval_runs (eval_ref)
    ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_gym_agentcl_eval_prompt_mutations_eval_step
  ON gym_agentcl_eval_prompt_mutations (eval_ref, step_index ASC, mutation_ref ASC);

CREATE INDEX IF NOT EXISTS idx_gym_agentcl_eval_prompt_mutations_task
  ON gym_agentcl_eval_prompt_mutations (task_ref, pass, step_index ASC);
