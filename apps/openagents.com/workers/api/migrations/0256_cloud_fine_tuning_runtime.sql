CREATE TABLE IF NOT EXISTS cloud_fine_tuning_jobs (
  job_id TEXT PRIMARY KEY,
  account_ref TEXT NOT NULL,
  base_model TEXT NOT NULL,
  dataset_ref TEXT NOT NULL,
  suffix TEXT,
  status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled')),
  fine_tuned_model TEXT,
  usage_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_cloud_fine_tuning_jobs_account
  ON cloud_fine_tuning_jobs (account_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS cloud_fine_tuned_models (
  model_id TEXT PRIMARY KEY,
  account_ref TEXT NOT NULL,
  job_id TEXT NOT NULL REFERENCES cloud_fine_tuning_jobs(job_id) ON DELETE CASCADE,
  base_model TEXT NOT NULL,
  dataset_ref TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'servable', 'retired')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cloud_fine_tuned_models_account
  ON cloud_fine_tuned_models (account_ref, status, created_at DESC);
