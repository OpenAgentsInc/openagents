-- Funnel analytics events
CREATE TABLE IF NOT EXISTS funnel_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event TEXT NOT NULL,
    user_id TEXT,
    repo TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_funnel_event ON funnel_events(event);
CREATE INDEX IF NOT EXISTS idx_funnel_user ON funnel_events(user_id);
CREATE INDEX IF NOT EXISTS idx_funnel_repo ON funnel_events(repo);
