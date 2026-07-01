CREATE TABLE IF NOT EXISTS gym_mutalisk_khala_delegation_jobs (
  run_ref TEXT PRIMARY KEY,
  job_ref TEXT NOT NULL UNIQUE,
  job_json TEXT NOT NULL,
  projection_json TEXT NOT NULL,
  latest_stage TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_gym_mutalisk_khala_delegation_jobs_updated
  ON gym_mutalisk_khala_delegation_jobs(updated_at DESC, run_ref ASC);

CREATE TABLE IF NOT EXISTS gym_mutalisk_khala_delegation_progress (
  run_ref TEXT NOT NULL,
  stage TEXT NOT NULL,
  progress_json TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (run_ref, stage),
  FOREIGN KEY (run_ref)
    REFERENCES gym_mutalisk_khala_delegation_jobs(run_ref)
    ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_gym_mutalisk_khala_delegation_progress_updated
  ON gym_mutalisk_khala_delegation_progress(updated_at DESC, run_ref ASC);

CREATE TABLE IF NOT EXISTS gym_mutalisk_khala_delegation_summaries (
  run_ref TEXT PRIMARY KEY,
  candidate_manifest_ref TEXT NOT NULL,
  candidate_ref TEXT NOT NULL,
  summary_json TEXT NOT NULL,
  admission_json TEXT NOT NULL,
  bridge_output_json TEXT NOT NULL,
  metric_value_bps INTEGER NOT NULL,
  admission_decision TEXT NOT NULL,
  ingested_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (run_ref)
    REFERENCES gym_mutalisk_khala_delegation_jobs(run_ref)
    ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_gym_mutalisk_khala_delegation_summaries_candidate
  ON gym_mutalisk_khala_delegation_summaries(candidate_manifest_ref, candidate_ref);
