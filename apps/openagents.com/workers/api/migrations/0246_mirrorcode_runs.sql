-- MirrorCode-as-a-service demo run rows (#6378, epic #6376).
--
-- One row per run, keyed by the public-safe `run_id`. Each row stores the
-- already-public-safe `MirrorCodeRun` object (built via buildMirrorCodeRun +
-- asserted by its no-task-contents / no-canary public-safety boundary BEFORE
-- storage) as a JSON blob, plus ordering/freshness columns. There is NO raw
-- benchmark content here: prompts, responses, logs, trajectories, keys, task
-- source, and test data are rejected at the ingest boundary and never reach
-- this table. The owner-gated POST upserts by run_id as a run is launched and
-- as its status/result lands. The public `/api/gym/mirrorcode/runs` route lists
-- these rows live at read; `/api/gym/mirrorcode/runs/{id}` reads one.
CREATE TABLE IF NOT EXISTS mirrorcode_runs (
  run_id TEXT PRIMARY KEY,
  run_json TEXT NOT NULL,
  bucket TEXT NOT NULL,
  grade TEXT NOT NULL,
  status TEXT NOT NULL,
  started_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_mirrorcode_runs_started_at
  ON mirrorcode_runs (started_at DESC, run_id ASC);
