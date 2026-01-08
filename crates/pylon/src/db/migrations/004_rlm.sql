-- RLM runs table
CREATE TABLE IF NOT EXISTS runs (
    id TEXT PRIMARY KEY,
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

CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);
CREATE INDEX IF NOT EXISTS idx_runs_created ON runs(created_at);

-- RLM trace events table
CREATE TABLE IF NOT EXISTS trace_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    seq INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    timestamp_ms INTEGER NOT NULL,
    event_json TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_trace_events_run ON trace_events(run_id);
CREATE INDEX IF NOT EXISTS idx_trace_events_run_seq ON trace_events(run_id, seq);

-- RLM experiments table
CREATE TABLE IF NOT EXISTS experiments (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);
