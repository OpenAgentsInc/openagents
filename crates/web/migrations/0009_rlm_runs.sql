-- RLM runs (experiment tracking)
CREATE TABLE IF NOT EXISTS rlm_runs (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    query TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('running', 'completed', 'failed')),
    fragment_count INTEGER NOT NULL DEFAULT 0,
    budget_sats INTEGER NOT NULL DEFAULT 0,
    total_cost_sats INTEGER NOT NULL DEFAULT 0,
    total_duration_ms INTEGER NOT NULL DEFAULT 0,
    output TEXT,
    error_message TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    completed_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_rlm_runs_user ON rlm_runs(user_id);
CREATE INDEX IF NOT EXISTS idx_rlm_runs_user_created ON rlm_runs(user_id, created_at);

-- RLM trace events
CREATE TABLE IF NOT EXISTS rlm_trace_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL REFERENCES rlm_runs(id) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    timestamp_ms INTEGER NOT NULL,
    event_json TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_rlm_trace_run_seq ON rlm_trace_events(run_id, seq);

-- RLM experiments
CREATE TABLE IF NOT EXISTS rlm_experiments (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_rlm_experiments_user ON rlm_experiments(user_id);
