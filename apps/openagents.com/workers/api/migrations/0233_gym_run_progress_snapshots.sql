-- Live Gym / Harbor run-progress snapshots (#6271, epic #6253).
--
-- One row per Harbor run, keyed by the public-safe `runRef`. Each row stores the
-- already-public-safe `openagents.gym.run_progress.v1` object (built via
-- buildGymRunProgress + asserted by checkGymRunProgressPublicSafety BEFORE
-- storage) as a JSON blob, plus ingest/update freshness. There is NO raw
-- benchmark content here: prompts, responses, logs, trajectories, keys, and
-- private endpoints are rejected at the ingest boundary and never reach this
-- table. The pusher upserts by runRef as a job streams task completions.
CREATE TABLE IF NOT EXISTS gym_run_progress_snapshots (
  run_ref TEXT PRIMARY KEY,
  progress_json TEXT NOT NULL,
  last_updated_at TEXT NOT NULL,
  ingested_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_gym_run_progress_snapshots_last_updated_at
  ON gym_run_progress_snapshots (last_updated_at DESC, run_ref ASC);
