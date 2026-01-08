-- RLM experiment/run mapping
CREATE TABLE IF NOT EXISTS rlm_experiment_runs (
    experiment_id TEXT NOT NULL REFERENCES rlm_experiments(id) ON DELETE CASCADE,
    run_id TEXT NOT NULL REFERENCES rlm_runs(id) ON DELETE CASCADE,
    label TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (experiment_id, run_id)
);

CREATE INDEX IF NOT EXISTS idx_rlm_experiment_runs_experiment ON rlm_experiment_runs(experiment_id);
CREATE INDEX IF NOT EXISTS idx_rlm_experiment_runs_run ON rlm_experiment_runs(run_id);
